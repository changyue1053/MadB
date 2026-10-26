import Foundation
import MadBObservation
import Observation

private actor Drain {
    private var ended = false
    private var waiters: [CheckedContinuation<Void, Never>] = []
    func finish() {
        ended = true
        let pending = waiters
        waiters.removeAll()
        for waiter in pending { waiter.resume() }
    }
    func wait() async {
        if ended { return }
        await withCheckedContinuation { waiters.append($0) }
    }
}

private final class ChangeFlag: @unchecked Sendable {
    private let lock = NSLock()
    private var changed = false
    func mark() { lock.withLock { changed = true } }
    var value: Bool { lock.withLock { changed } }
}

@main
private struct PackageConsumer {
    @MainActor static func main() async throws {
        let model = MadBStateModel<Int>()
        precondition(model.value == nil && model.status == .idle)
        let (stream, continuation) = AsyncStream<Int>.makeStream(bufferingPolicy: .bufferingNewest(1))
        let drain = Drain()
        var factories = 0
        try model.start {
            factories += 1
            return MadBStateSource(
                collect: { receive in
                    for await value in stream { await receive(value) }
                    await drain.finish()
                },
                requestStop: { continuation.finish() },
                awaitStopped: { await drain.wait() }
            )
        }
        let changes = ChangeFlag()
        withObservationTracking { _ = model.value } onChange: { changes.mark() }
        continuation.yield(42)
        let deadline = ContinuousClock.now + .seconds(5)
        while model.value != 42 && ContinuousClock.now < deadline { await Task.yield() }
        precondition(model.value == 42 && changes.value)
        do {
            try model.start { factories += 1; preconditionFailure("duplicate factory executed") }
            preconditionFailure("duplicate start accepted")
        } catch MadBObservationError.alreadyActive {}
        precondition(factories == 1)
        try await model.stopAndWait()
        try await model.stopAndWait()
        precondition(model.status == .stopped && model.value == 42)
        precondition(model.collectionFailure == nil && model.stopFailure == nil)
        print("MADB_OBSERVATION_PACKAGE_CONSUMER_OK")
    }
}
