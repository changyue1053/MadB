import Foundation
import Observation

/// Presentation-session progress, not a replacement for the native resource state.
public enum MadBObservationStatus: Sendable, Equatable {
    case idle, observing, stopping, stopped, stopFailed
}

public enum MadBObservationError: Error, Equatable {
    case alreadyActive
}

/// A local, MainActor-isolated projection of owned state snapshots.
///
/// Construction and property access never subscribe. Keep a stable model owner and
/// call `start` explicitly. A model owns one subscription, not its business engine.
/// This is latest presentation state, not a lossless business-event stream.
@MainActor
@Observable
public final class MadBStateModel<Value: Sendable> {
    /// Nil until this session receives a real value. No initial replay is invented.
    public private(set) var value: Value?
    public private(set) var status: MadBObservationStatus = .idle
    public private(set) var collectionFailure: (any Error)?
    public private(set) var stopFailure: (any Error)?

    private struct Session: Sendable {
        let id: UUID
        let source: MadBStateSource<Value>
        let task: Task<Result<Void, any Error>, Never>
    }

    @ObservationIgnored private var session: Session?
    @ObservationIgnored private var isStarting = false
    @ObservationIgnored private var stopRequestedDuringStart = false

    public init() {}

    /// Checks admission before invoking the factory exactly once. The factory must
    /// release anything it acquired if it throws. It is the explicit subscription point.
    /// A successful restart clears the old snapshot and errors while awaiting new state.
    public func start(using factory: @MainActor () throws -> MadBStateSource<Value>) throws {
        guard session == nil, !isStarting else { throw MadBObservationError.alreadyActive }
        isStarting = true
        stopRequestedDuringStart = false
        defer { isStarting = false; stopRequestedDuringStart = false }
        let source = try factory()
        let id = UUID()
        value = nil
        collectionFailure = nil
        stopFailure = nil
        status = .observing
        // Factories and Observation's synchronous willSet callbacks can request
        // stop before a task exists. Preserve that request and never collect it.
        let shouldCollect = !stopRequestedDuringStart
        if !shouldCollect { status = .stopping }
        let task = Task { [weak self, source] in
            var failure: (any Error)?
            do {
                if shouldCollect {
                    try await source.collect { [weak self] value in
                        await self?.receive(value, sessionID: id)
                    }
                }
            } catch {
                // Cancellation is normal only after this model requested a stop.
                if !(error is CancellationError && Task.isCancelled) { failure = error }
            }
            source.requestStop()
            self?.collectionEnded(sessionID: id, failure: failure)
            // Cleanup inherits neither collector cancellation nor callback TaskLocals
            // (including backend reentrancy guards). One bounded attempt per session.
            let result = await Task.detached { await Self.drain(source) }.value
            self?.stopEnded(sessionID: id, result: result)
            return result
        }
        session = Session(id: id, source: source, task: task)
        if !shouldCollect { source.requestStop() }
    }

    /// Requests stop immediately. It is not a drain confirmation.
    public func requestStop() {
        if isStarting { stopRequestedDuringStart = true }
        guard let session else { return }
        if status != .stopFailed { status = .stopping }
        session.source.requestStop()
        session.task.cancel()
    }

    /// Owner cleanup join: cancellation of the calling task does not shorten this wait.
    /// Returns only after the source confirms drain; it never closes the shared engine.
    /// A failed drain keeps the source owned and rejects restart. Calling this method
    /// again retries that drain, without collecting or subscribing again.
    public func stopAndWait() async throws {
        guard let current = session else { return }
        let task: Task<Result<Void, any Error>, Never>
        if status == .stopFailed {
            status = .stopping
            stopFailure = nil
            current.source.requestStop()
            task = Task { [weak self, source = current.source, id = current.id] in
                let result = await Task.detached { await Self.drain(source) }.value
                self?.stopEnded(sessionID: id, result: result)
                return result
            }
            session = Session(id: current.id, source: current.source, task: task)
        } else {
            requestStop()
            task = current.task
        }
        try await task.value.get()
    }

    private func receive(_ next: Value, sessionID: UUID) {
        guard session?.id == sessionID, status == .observing else { return }
        value = next
    }

    private func collectionEnded(sessionID: UUID, failure: (any Error)?) {
        guard session?.id == sessionID else { return }
        collectionFailure = failure
        status = .stopping
    }

    private func stopEnded(sessionID: UUID, result: Result<Void, any Error>) {
        guard session?.id == sessionID else { return }
        switch result {
        case .success:
            stopFailure = nil
            status = .stopped
            // Keep admission closed through synchronous Observation notifications;
            // a callback cannot start a session that this completion then overwrites.
            session = nil
        case .failure(let error):
            stopFailure = error
            status = .stopFailed
        }
    }

    private nonisolated static func drain(_ source: MadBStateSource<Value>) async -> Result<Void, any Error> {
        do { try await source.awaitStopped(); return .success(()) }
        catch { return .failure(error) }
    }

    deinit {
        // Best effort only: callers still owe explicit stopAndWait confirmation.
        // The running task owns its source and completes its already-required drain.
        session?.source.requestStop()
        session?.task.cancel()
    }
}
