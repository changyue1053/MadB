import MadBObservation

// Consumer-authored value. This compile probe does not generate an SDK or claim
// a foreign object is Sendable. The producer of the source owns its stop protocol.
struct Reading: Sendable {
    let value: Int32
    let label: String
}

@MainActor
func observeOrdinarySource(
    makeSource: @MainActor () throws -> MadBStateSource<Reading>
) async throws {
    let model = MadBStateModel<Reading>()
    let initial: Reading? = model.value
    _ = initial
    try model.start(using: makeSource)
    model.requestStop()
    try await model.stopAndWait()
}
