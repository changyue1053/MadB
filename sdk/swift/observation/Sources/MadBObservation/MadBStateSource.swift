/// The ordinary subscription operations required by the optional UI projection.
///
/// `collect` must serialize and await deliveries, and terminate after stop/cancellation.
/// `requestStop` must be thread-safe and idempotent. `awaitStopped` must confirm the
/// underlying subscription and admitted deliveries have drained. Implementations
/// keep their existing ownership, buffering, cancellation and thread contracts.
/// No source method may synchronously wait for the MainActor.
public struct MadBStateSource<Value: Sendable>: Sendable {
    public typealias Receiver = @Sendable (Value) async -> Void

    let collect: @Sendable (@escaping Receiver) async throws -> Void
    let requestStop: @Sendable () -> Void
    let awaitStopped: @Sendable () async throws -> Void

    public init(
        collect: @escaping @Sendable (@escaping Receiver) async throws -> Void,
        requestStop: @escaping @Sendable () -> Void,
        awaitStopped: @escaping @Sendable () async throws -> Void
    ) {
        self.collect = collect
        self.requestStop = requestStop
        self.awaitStopped = awaitStopped
    }
}
