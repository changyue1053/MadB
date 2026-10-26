import MadBObservation

/// A terminal error delivered by the public SKIE state subscription.
public struct MadBSkieStateFailure: Error, Equatable, Sendable {
    public let code: String
    public init(code: String) { self.code = code }
}

private actor StateFailure {
    var failure: MadBSkieStateFailure?
    func record(_ code: String) { if failure == nil { failure = MadBSkieStateFailure(code: code) } }
}

/// Compile this optional adapter with the installed C06 concurrency facade.
/// Call it inside `model.start { ... }` so duplicate starts cannot acquire a subscription.
/// It uses the public state lane, preserving its pending-value coalescing and no-replay
/// semantics. The shared engine remains owned and closed by its original caller.
public func makeMadBSkieStateSource(from owner: MadBEngineHandle) throws -> MadBStateSource<MadBValueSnapshot> {
    let subscription = try owner.subscribeState()
    return MadBStateSource(
        collect: { receive in
            let state = StateFailure()
            try await subscription.collect { event in
                switch event {
                case .data(let value): await receive(value)
                case .failure(let code):
                    await state.record(code)
                    subscription.requestStop()
                }
            }
            if let failure = await state.failure { throw failure }
        },
        requestStop: { subscription.requestStop() },
        awaitStopped: { try await subscription.awaitStopped() }
    )
}
