import Foundation
import MadBObservation

enum TestFailure: Error, Equatable { case collection, drain, factory }

enum TestCallbackContext { @TaskLocal static var active = false }

/// Test-owned barriers make collection and native drain independently controllable.
/// Every field is protected by lock; continuations and callbacks run outside it.
final class ControlledSource: @unchecked Sendable {
    private let lock = NSLock()
    private var receiver: MadBStateSource<Int>.Receiver?
    private var collection: CheckedContinuation<Void, any Error>?
    private var terminal: Result<Void, any Error>?
    private var collectWaiters: [CheckedContinuation<Void, Never>] = []
    private var drainWaiters: [CheckedContinuation<Void, Never>] = []
    private var drainRelease: CheckedContinuation<Void, Never>?
    private var drainHeld: Bool
    private var drainFailure: TestFailure?
    private var drains = 0
    private var stops = 0
    private var cancelledDrain = false
    private var inheritedCallbackContext = false

    init(holdDrain: Bool = false, drainFailure: TestFailure? = nil) {
        drainHeld = holdDrain
        self.drainFailure = drainFailure
    }

    var source: MadBStateSource<Int> {
        MadBStateSource(collect: { try await self.collect($0) },
                        requestStop: { self.requestStop() },
                        awaitStopped: { try await self.awaitStopped() })
    }

    var counts: (collects: Int, stops: Int, drains: Int, cancelledDrain: Bool, inheritedCallbackContext: Bool) {
        lock.withLock { (receiver == nil ? 0 : 1, stops, drains, cancelledDrain, inheritedCallbackContext) }
    }

    private func collect(_ receive: @escaping MadBStateSource<Int>.Receiver) async throws {
        try await withCheckedThrowingContinuation { continuation in
            let result = lock.withLock { () -> (Result<Void, any Error>?, [CheckedContinuation<Void, Never>]) in
                receiver = receive
                let waiters = collectWaiters
                collectWaiters = []
                if let terminal { return (terminal, waiters) }
                collection = continuation
                return (nil, waiters)
            }
            result.1.forEach { $0.resume() }
            if let terminal = result.0 { continuation.resume(with: terminal) }
        }
    }

    func waitForCollection() async {
        await withCheckedContinuation { continuation in
            let ready = lock.withLock {
                if receiver != nil { return true }
                collectWaiters.append(continuation); return false
            }
            if ready { continuation.resume() }
        }
    }

    func emit(_ value: Int) async {
        let receive = lock.withLock { receiver }
        await receive?(value)
    }

    func finish(_ result: Result<Void, any Error> = .success(())) {
        let continuation = lock.withLock {
            if terminal == nil { terminal = result }
            let pending = collection; collection = nil
            return pending
        }
        continuation?.resume(with: result)
    }

    private func requestStop() {
        lock.withLock { stops += 1 }
        finish()
    }

    private func awaitStopped() async throws {
        let state = lock.withLock { () -> (TestFailure?, [CheckedContinuation<Void, Never>]) in
            drains += 1
            cancelledDrain = cancelledDrain || Task.isCancelled
            inheritedCallbackContext = inheritedCallbackContext || TestCallbackContext.active
            let failure = drainFailure; drainFailure = nil
            let waiters = drainWaiters; drainWaiters = []
            return (failure, waiters)
        }
        state.1.forEach { $0.resume() }
        if let failure = state.0 { throw failure }
        await withCheckedContinuation { continuation in
            let ready = lock.withLock {
                if !drainHeld { return true }
                drainRelease = continuation; return false
            }
            if ready { continuation.resume() }
        }
    }

    func waitForDrain() async {
        await withCheckedContinuation { continuation in
            let ready = lock.withLock {
                if drains > 0 { return true }
                drainWaiters.append(continuation); return false
            }
            if ready { continuation.resume() }
        }
    }

    func releaseDrain() {
        let continuation = lock.withLock {
            drainHeld = false
            let pending = drainRelease; drainRelease = nil
            return pending
        }
        continuation?.resume()
    }
}

final class ChangeCounter: @unchecked Sendable {
    private let lock = NSLock()
    private var count = 0
    func increment() { lock.withLock { count += 1 } }
    var value: Int { lock.withLock { count } }
}
