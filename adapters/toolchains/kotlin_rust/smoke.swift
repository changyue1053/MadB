import Foundation
@main struct ProducerSmoke {
    static func main() async throws {
        let engine = try openEngine(expectedProfile: CommandLine.arguments[1])
        let value = Value(id: "producer", sequence: nil, text: "你好", bytes: [], phase: .ready)
        let output = try engine.roundtrip(value: value)
        precondition(output.id == "producer" && output.sequence == nil && output.text == "你好")
        engine.requestClose()
        try await engine.awaitClosed()
        precondition(engine.statistics().workerExits == 1)
        print("MADB_C06_SMOKE_OK")
    }
}
