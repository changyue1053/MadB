import Foundation
import MadBSkie

public enum MadBPhase: Sendable, Equatable {
    case open, closing, closed
}

/// An owned value copy for transfer between Swift actors. Kotlin DTOs themselves gain no Sendable claim.
public struct MadBValueSnapshot: Sendable, Equatable {
    public let id: String
    public let sequence: Int64?
    public let text: String
    public let bytes: [UInt8]
    public let phase: MadBPhase

    public init(copying value: MadBSkie.Value) {
        id = value.id
        sequence = value.sequence?.int64Value
        text = value.text
        bytes = (0..<Int(value.bytes.size)).map { UInt8(bitPattern: value.bytes.get(index: Int32($0))) }
        switch value.phase {
        case .open: phase = .open
        case .closing: phase = .closing
        case .closed: phase = .closed
        }
    }
}

public enum MadBEventSnapshot: Sendable, Equatable {
    case data(MadBValueSnapshot)
    case failure(String)

    public init(copying event: MadBSkie.BoundaryEvent) {
        switch onEnum(of: event) {
        case .data(let data): self = .data(MadBValueSnapshot(copying: data.value))
        case .failure(let failure): self = .failure(failure.code)
        }
    }
}

public enum MadBSkieRuntimeError: Error { case wouldDeadlock, closing, resourceExhausted, alreadyCollected }

private enum DeliveryContext {
    @TaskLocal static var owner: ObjectIdentifier?
    @TaskLocal static var gate: ObjectIdentifier?
}

/// Protects only Swift callback admission/drain. It owns no producer, task, payload queue or native state.
private final class DeliveryGate: @unchecked Sendable {
    private let lock = NSLock()
    private var closing = false
    private var active = 0
    private var collectorClaimed = false
    // Token fields are only read/written under this gate's lock.
    private final class Waiter: @unchecked Sendable { var cancelled = false }
    private var waiters: [ObjectIdentifier: CheckedContinuation<Void, any Error>] = [:]
    func claimCollector() throws {
        try lock.withLock {
            if collectorClaimed { throw MadBSkieRuntimeError.alreadyCollected }
            collectorClaimed = true
        }
    }
    func enter() -> Bool {
        lock.withLock { if closing { return false }; active += 1; return true }
    }
    func leave() {
        let ready = lock.withLock {
            active -= 1
            if closing && active == 0 { let result = Array(waiters.values); waiters.removeAll(); return result }
            return []
        }
        ready.forEach { $0.resume() }
    }
    func closeAdmission() {
        let ready = lock.withLock {
            closing = true
            if active == 0 { let result = Array(waiters.values); waiters.removeAll(); return result }
            return []
        }
        ready.forEach { $0.resume() }
    }
    func awaitDrained() async throws {
        let token = Waiter()
        try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, any Error>) in
                let outcome: Result<Void, any Error>? = lock.withLock {
                    if token.cancelled || Task.isCancelled { return .failure(CancellationError()) }
                    if active == 0 { return .success(()) }
                    if waiters.count == 32 { return .failure(MadBSkieRuntimeError.resourceExhausted) }
                    waiters[ObjectIdentifier(token)] = continuation
                    return nil
                }
                if let outcome { continuation.resume(with: outcome) }
            }
        } onCancel: {
            let continuation = self.lock.withLock {
                token.cancelled = true
                return self.waiters.removeValue(forKey: ObjectIdentifier(token))
            }
            continuation?.resume(throwing: CancellationError())
        }
        try Task.checkCancellation()
    }
}

private final class DeliveryOwner: @unchecked Sendable {
    private let lock = NSLock()
    private var closing = false
    private var gates: [ObjectIdentifier: DeliveryGate] = [:]
    private var waiters = 0
    func acquireWaiter() throws {
        try lock.withLock {
            if waiters == 32 { throw MadBSkieRuntimeError.resourceExhausted }
            waiters += 1
        }
    }
    func releaseWaiter() { lock.withLock { waiters -= 1 } }
    func register(_ gate: DeliveryGate) throws {
        try lock.withLock {
            if closing { throw MadBSkieRuntimeError.closing }
            if gates.count == 8 { throw MadBSkieRuntimeError.resourceExhausted }
            gates[ObjectIdentifier(gate)] = gate
        }
    }
    func remove(_ gate: DeliveryGate) { _ = lock.withLock { gates.removeValue(forKey: ObjectIdentifier(gate)) } }
    func closeAdmission() -> [DeliveryGate] {
        let current = lock.withLock { closing = true; return Array(gates.values) }
        current.forEach { $0.closeAdmission() }
        return current
    }
}

/// One facade owner per engine; copies share the same Swift delivery owner. Kotlin admission and
/// native close state are NSLock-protected. No mutable Kotlin DTO is published across actors.
/// No deinit blocks or replaces the explicit asynchronous close.
public struct MadBEngineHandle: @unchecked Sendable {
    /// Raw generated API for synchronous interop. Its stop/close does not include Swift facade deliveries.
    public let native: MadBSkie.Engine
    private let delivery = DeliveryOwner()
    public init(expectedProfile: String) throws { native = try openEngine(expectedProfile: expectedProfile) }
    public func subscribeEvents() throws -> MadBSubscriptionHandle {
        try MadBSubscriptionHandle(native.subscribeEvents(), owner: delivery)
    }
    public func subscribeState() throws -> MadBSubscriptionHandle {
        try MadBSubscriptionHandle(native.subscribeState(), owner: delivery)
    }
    public func requestClose() { _ = delivery.closeAdmission(); native.requestClose() }
    public func awaitClosed() async throws {
        if DeliveryContext.owner == ObjectIdentifier(delivery) { throw MadBSkieRuntimeError.wouldDeadlock }
        try delivery.acquireWaiter()
        defer { delivery.releaseWaiter() }
        let pending = delivery.closeAdmission()
        native.requestClose()
        try await native.awaitClosed()
        for gate in pending { try await gate.awaitDrained(); delivery.remove(gate) }
    }
}

/// Operation.run/requestCancel synchronize through the same Kotlin lock and Deferred instance.
/// Host callbacks execute on the declared serial worker; hosts must meet that concurrency contract.
public struct MadBOperationHandle: @unchecked Sendable {
    private let native: MadBSkie.Operation
    public init(_ native: MadBSkie.Operation) { self.native = native }
    public func requestCancel() { native.requestCancel() }
    public func result() async throws -> MadBValueSnapshot {
        MadBValueSnapshot(copying: try await native.run())
    }
}

/// One collector per observation, with no extra queue or producer task. A Swift delivery lease
/// covers the consumer body after SKIE next() hands off a value; stop waits for that lease too.
public struct MadBSubscriptionHandle: @unchecked Sendable {
    private let native: MadBSkie.Subscription
    private let gate = DeliveryGate()
    private let owner: DeliveryOwner
    fileprivate init(_ native: MadBSkie.Subscription, owner: DeliveryOwner) throws {
        self.native = native; self.owner = owner
        do { try owner.register(gate) }
        catch { native.requestStop(); throw error }
    }
    public func requestStop() { gate.closeAdmission(); native.requestStop() }
    public func awaitStopped() async throws {
        if DeliveryContext.gate == ObjectIdentifier(gate) { throw MadBSkieRuntimeError.wouldDeadlock }
        try owner.acquireWaiter()
        defer { owner.releaseWaiter() }
        requestStop()
        try await native.awaitStopped()
        try await gate.awaitDrained()
        owner.remove(gate)
    }
    public func collect(_ receive: @Sendable (MadBEventSnapshot) async -> Void) async throws {
        try gate.claimCollector()
        defer { requestStop(); owner.remove(gate) }
        for await event in native.events {
            guard gate.enter() else { break }
            let snapshot = MadBEventSnapshot(copying: event)
            await DeliveryContext.$owner.withValue(ObjectIdentifier(owner)) {
                await DeliveryContext.$gate.withValue(ObjectIdentifier(gate)) { await receive(snapshot) }
            }
            gate.leave()
        }
    }
}
