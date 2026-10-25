import Foundation

private actor P03Signal {
    private var signalled = false
    private var waiter: CheckedContinuation<Void, Never>?
    func wait() async { if !signalled { await withCheckedContinuation { waiter = $0 } } }
    func signal() { signalled = true; waiter?.resume(); waiter = nil }
}
private final class EchoAsyncHost: AsyncHost {
    func transform(value: Value) async throws -> Value { await Task.yield(); return value }
}
private final class ErrorAsyncHost: AsyncHost {
    func transform(value: Value) async throws -> Value { throw BoundaryError.CallbackFailure(message: "declared host failure") }
}
private final class LateObjectHost: AsyncObjectHost {
    let entered = P03Signal()
    let release = P03Signal()
    func produce(resource: AsyncResource) async -> AsyncResource {
        await entered.signal()
        // Deliberately ignores cancellation: the transferred late object must be reclaimed.
        await release.wait()
        return resource
    }
}
private func cancelledObject(_ ledger: TestLedger) async throws {
    let resource = ledger.createResource()
    let host = LateObjectHost()
    let call = Task { try await asyncHostObject(resource: resource, host: host) }
    await host.entered.wait()
    call.cancel()
    do { _ = try await call.value; preconditionFailure("cancelled waiter resumed") }
    catch is CancellationError { }
    await host.release.signal()
    // On return ARC releases this scope's resource and task references. The parent
    // observes the Rust Drop counter, not merely callback-body completion.
}
@main
private struct P03Swift {
    static func main() async throws {
        let value = Value(id: "m1", sequence: Int64.min, text: "你好\0é", bytes: [0,127,128,255], phase: .done)
        let echoed = try await asyncHostTransform(value: value, host: EchoAsyncHost())
        precondition(echoed == value)
        do { _ = try await asyncHostTransform(value: value, host: ErrorAsyncHost()); preconditionFailure("error lost") }
        catch BoundaryError.CallbackFailure { }
        let ledger = TestLedger()
        try await cancelledObject(ledger)
        let deadline = ContinuousClock.now + .seconds(5)
        while ledger.released() != 1 {
            precondition(ContinuousClock.now < deadline, "late object leaked or callback did not drain")
            try await Task.sleep(for: .milliseconds(1))
        }
        precondition(ledger.created() == 1)
        precondition(uniffiForeignFutureHandleCountMadbBoundary() == 0)
        print("{\"check\":\"uniffi-swift-p03\",\"actual\":\"passed\",\"late_object_releases\":1}")
    }
}
