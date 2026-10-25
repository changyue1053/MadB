import Foundation
import MadBSkie

private final class SmokeHost: NSObject, MadBSkie.Host {
    func transform(value: MadBSkie.Value) -> MadBSkie.Value { value }
}

@main private struct Smoke {
    @MainActor static func main() async throws {
        guard CommandLine.arguments.count == 2 else { throw SmokeError.arguments }
        let owner = try MadBEngineHandle(expectedProfile: CommandLine.arguments[1])
        let engine = owner.native
        guard nativeAbiVersion() == 1, bootstrapVersion() == 1 else { throw SmokeError.identity }
        let bytes = KotlinByteArray(size: 2)
        bytes.set(index: 0, value: 0); bytes.set(index: 1, value: -1)
        let input = MadBSkie.Value(id: "m1", sequence: nil, text: "你好", bytes: bytes, phase: .open)
        let echoed = try engine.roundtrip(value: input)
        guard echoed.text == "你好", echoed.sequence == nil else { throw SmokeError.value }
        switch engine.phase() { case .open: break; case .closing, .closed: throw SmokeError.value }
        let operation = MadBOperationHandle(try engine.startOperation(value: input, host: SmokeHost()))
        let result = try await operation.result()
        guard result.bytes == [0, 255] else { throw SmokeError.value }
        let subscription = try owner.subscribeEvents()
        let collector = Task { @MainActor in
            try await subscription.collect { event in
                switch event {
                case .data(let value):
                    precondition(value.id == "m1")
                    subscription.requestStop()
                case .failure: preconditionFailure("unexpected stream failure")
                }
            }
        }
        try engine.publish(value: input)
        try await collector.value
        try await subscription.awaitStopped()
        switch onEnum(of: failureEvent(code: "fixture")) {
        case .failure(let failure): guard failure.code == "fixture" else { throw SmokeError.value }
        case .data: throw SmokeError.value
        }
        try await owner.awaitClosed()
        let stats = engine.stats()
        guard stats.closed, stats.activeOperations == 0, stats.activeSubscriptions == 0, stats.retainedBytes == 0 else { throw SmokeError.closed }
        print("MADB_C06_SKIE_SMOKE_OK")
    }
    private enum SmokeError: Error { case arguments, identity, value, closed }
}
