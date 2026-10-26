import Foundation
import MadBObservation
import MadBSkie

private enum ConsumerFailure: Error { case check(String) }

@MainActor
private func require(_ condition: Bool, _ message: String) throws {
    if !condition { throw ConsumerFailure.check(message) }
}

@MainActor
private func sample(_ id: String) -> MadBSkie.Value {
    let bytes = KotlinByteArray(size: 2)
    bytes.set(index: 0, value: 0)
    bytes.set(index: 1, value: -1)
    return MadBSkie.Value(id: id, sequence: nil, text: "state-" + id, bytes: bytes, phase: .open)
}

@MainActor
private func awaitValue(_ model: MadBStateModel<MadBValueSnapshot>, _ id: String) async throws {
    let deadline = ContinuousClock.now + .seconds(10)
    while model.value?.id != id && ContinuousClock.now < deadline { await Task.yield() }
    try require(model.value?.id == id, "missing native state: " + id)
    try require(model.value?.text == "state-" + id && model.value?.bytes == [0, 255], "snapshot contents differ")
    try require(model.collectionFailure == nil && model.stopFailure == nil, "unexpected source failure")
}

@MainActor
private func requireDrained(_ engine: MadBSkie.Engine, closed: Bool) throws {
    let stats = engine.stats()
    try require(stats.activeSubscriptions == 0 && stats.activeOperations == 0, "active native resources remain")
    try require(stats.queuedItems == 0 && stats.inflightItems == 0 && stats.retainedBytes == 0, "native payloads remain")
    try require(stats.activeWaiters == 0 && stats.closed == closed, "native close/waiter state differs")
}

@MainActor
private func checkIndependentObservers(_ owner: MadBEngineHandle) async throws {
    let engine = owner.native
    let first = MadBStateModel<MadBValueSnapshot>()
    let second = MadBStateModel<MadBValueSnapshot>()
    try first.start { try makeMadBSkieStateSource(from: owner) }
    try second.start { try makeMadBSkieStateSource(from: owner) }
    try require(engine.stats().activeSubscriptions == 2, "shared owner did not acquire independent subscriptions")
    try require(first.value == nil && second.value == nil, "new observers invented replay")
    try engine.publish(value: sample("both-pages"))
    try await awaitValue(first, "both-pages")
    try await awaitValue(second, "both-pages")

    try await first.stopAndWait()
    try require(engine.stats().activeSubscriptions == 1 && !engine.stats().closed, "one page stop changed the other subscription or owner")
    try engine.publish(value: sample("second-page-only"))
    try await awaitValue(second, "second-page-only")
    try require(first.status == .stopped && first.value?.id == "both-pages", "stopped first page received a later update")

    try first.start { try makeMadBSkieStateSource(from: owner) }
    try require(first.value == nil && second.value?.id == "second-page-only", "page reentry changed another page or replayed old state")
    try require(engine.stats().activeSubscriptions == 2, "page reentry did not restore one subscription")
    try engine.publish(value: sample("both-pages-again"))
    try await awaitValue(first, "both-pages-again")
    try await awaitValue(second, "both-pages-again")

    try await second.stopAndWait()
    try require(engine.stats().activeSubscriptions == 1 && !engine.stats().closed, "second page stop closed the shared owner")
    try engine.publish(value: sample("first-page-only"))
    try await awaitValue(first, "first-page-only")
    try require(second.status == .stopped && second.value?.id == "both-pages-again", "stopped second page received a later update")
    try await first.stopAndWait()
    try requireDrained(engine, closed: false)
}

@main
private struct SkieConsumer {
    @MainActor static func main() async throws {
        guard CommandLine.arguments.count == 3 else { throw ConsumerFailure.check("expected trusted profile and interface") }
        let profile = CommandLine.arguments[1]
        let interface = CommandLine.arguments[2]
        try require(profileIdentity() == profile && interfaceIdentity() == interface, "loaded native identity differs")
        try require(bootstrapVersion() == 1 && nativeAbiVersion() == 1, "native ABI differs")
        let owner = try MadBEngineHandle(expectedProfile: profile)
        let engine = owner.native
        let model = MadBStateModel<MadBValueSnapshot>()
        try require(model.value == nil && model.status == .idle, "construction changed state")
        try requireDrained(engine, closed: false)
        try engine.publish(value: sample("before-subscription"))
        var factories = 0
        try model.start { factories += 1; return try makeMadBSkieStateSource(from: owner) }
        try require(model.value == nil, "state adapter invented initial replay")
        try require(engine.stats().activeSubscriptions == 1 && engine.stats().queuedItems == 0, "factory did not acquire one empty subscription")
        do {
            try model.start { factories += 1; return try makeMadBSkieStateSource(from: owner) }
            throw ConsumerFailure.check("duplicate start accepted")
        } catch MadBObservationError.alreadyActive {}
        try require(factories == 1 && engine.stats().activeSubscriptions == 1, "duplicate start acquired native resource")
        // The collector cannot enter MainActor between these synchronous publishes.
        let discardedBefore = engine.stats().discardedValues
        try engine.publish(value: sample("coalesced"))
        try engine.publish(value: sample("first"))
        try require(engine.stats().queuedItems == 1 && engine.stats().discardedValues == discardedBefore + 1, "native state did not coalesce pending values")
        try await awaitValue(model, "first")
        model.requestStop()
        try await model.stopAndWait()
        try await model.stopAndWait()
        try require(model.status == .stopped && model.value?.id == "first", "stop lost last presentation value")
        try requireDrained(engine, closed: false)
        try require(try engine.roundtrip(value: sample("engine-alive")).id == "engine-alive", "UI stop closed shared engine")
        try engine.publish(value: sample("while-stopped"))
        try model.start { factories += 1; return try makeMadBSkieStateSource(from: owner) }
        try require(model.value == nil, "restart replayed stale state")
        try engine.publish(value: sample("second"))
        try await awaitValue(model, "second")
        let firstStop = Task { try await model.stopAndWait() }
        let secondStop = Task { try await model.stopAndWait() }
        try await firstStop.value
        try await secondStop.value
        try require(factories == 2 && model.status == .stopped, "restart/duplicate stop failed")
        try requireDrained(engine, closed: false)
        try await checkIndependentObservers(owner)
        try await owner.awaitClosed()
        try requireDrained(engine, closed: true)
        print("MADB_OBSERVATION_SKIE_CONSUMER_OK")
    }
}
