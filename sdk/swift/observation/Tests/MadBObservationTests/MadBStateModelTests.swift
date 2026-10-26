import MadBObservation
import Observation
import Testing

@Suite(.timeLimit(.minutes(1)))
@MainActor
struct MadBStateModelTests {
    @Test func constructionAndFailedFactoryHaveNoSubscription() throws {
        let model = MadBStateModel<Int>()
        #expect(model.value == nil)
        #expect(model.status == .idle)
        #expect(throws: TestFailure.factory) {
            try model.start { throw TestFailure.factory }
        }
        #expect(model.status == .idle)
        #expect(model.collectionFailure == nil)
    }

    @Test func duplicateStartDoesNotCallFactory() async throws {
        let model = MadBStateModel<Int>()
        let source = ControlledSource()
        var calls = 0
        try model.start { calls += 1; return source.source }
        #expect(throws: MadBObservationError.alreadyActive) {
            try model.start { calls += 1; return source.source }
        }
        #expect(calls == 1)
        try await model.stopAndWait()
        #expect(source.counts.drains == 1)
    }

    @Test func reentrantFactoryCannotAcquireAnotherSubscription() async throws {
        let model = MadBStateModel<Int>()
        let source = ControlledSource()
        var secondFactoryCalled = false
        try model.start {
            #expect(throws: MadBObservationError.alreadyActive) {
                try model.start { secondFactoryCalled = true; return source.source }
            }
            return source.source
        }
        #expect(!secondFactoryCalled)
        try await model.stopAndWait()
    }

    @Test func drainDoesNotInheritBackendCallbackTaskLocal() async throws {
        let model = MadBStateModel<Int>()
        let source = ControlledSource()
        try TestCallbackContext.$active.withValue(true) {
            try model.start { source.source }
        }
        try await model.stopAndWait()
        #expect(!source.counts.inheritedCallbackContext)
    }

    @Test func stopRequestedInsideFactorySkipsCollectionAndStillDrains() async throws {
        let model = MadBStateModel<Int>()
        let source = ControlledSource()
        try model.start {
            model.requestStop()
            return source.source
        }
        #expect(model.status == .stopping)
        try await model.stopAndWait()
        #expect(source.counts.collects == 0)
        #expect(source.counts.drains == 1)
        #expect(model.status == .stopped)
    }

    @Test func observationCanRequestStopDuringStartPublication() async throws {
        let model = MadBStateModel<Int>()
        let source = ControlledSource()
        withObservationTracking { _ = model.status } onChange: {
            MainActor.assumeIsolated { model.requestStop() }
        }
        try model.start { source.source }
        #expect(model.status == .stopping)
        try await model.stopAndWait()
        #expect(source.counts.collects == 0)
        #expect(source.counts.drains == 1)
    }

    @Test func stopPublicationKeepsAdmissionClosedUntilItFinishes() async throws {
        let model = MadBStateModel<Int>()
        let first = ControlledSource(holdDrain: true)
        let second = ControlledSource()
        try model.start { first.source }
        model.requestStop()
        await first.waitForDrain()
        let rejected = ChangeCounter()
        let acquired = ChangeCounter()
        withObservationTracking { _ = model.status } onChange: {
            MainActor.assumeIsolated {
                do { try model.start { acquired.increment(); return second.source } }
                catch MadBObservationError.alreadyActive { rejected.increment() }
                catch { Issue.record("unexpected reentrant admission error: \(error)") }
            }
        }
        first.releaseDrain()
        try await model.stopAndWait()
        #expect(acquired.value == 0)
        #expect(rejected.value == 1)
        try model.start { second.source }
        #expect(model.status == .observing)
        try await model.stopAndWait()
    }

    @Test func realValueTriggersObservationAndStopSuppressesLateDelivery() async throws {
        let model = MadBStateModel<Int>()
        let source = ControlledSource()
        try model.start { source.source }
        await source.waitForCollection()
        #expect(model.value == nil)
        let changes = ChangeCounter()
        withObservationTracking { _ = model.value } onChange: { changes.increment() }
        await source.emit(42)
        #expect(model.value == 42)
        #expect(changes.value == 1)
        model.requestStop()
        await source.emit(99) // A deliberately misbehaving source cannot update stopped UI.
        try await model.stopAndWait()
        #expect(model.value == 42)
        #expect(model.status == .stopped)
    }

    @Test func cancelledOwnerStillWaitsForRealDrain() async throws {
        let model = MadBStateModel<Int>()
        let source = ControlledSource(holdDrain: true)
        try model.start { source.source }
        await source.waitForCollection()
        var returned = false
        let stop = Task { try await model.stopAndWait(); returned = true }
        await source.waitForDrain()
        stop.cancel()
        #expect(model.status == .stopping)
        #expect(!returned)
        #expect(!source.counts.cancelledDrain)
        source.releaseDrain()
        try await stop.value
        #expect(returned)
        #expect(model.status == .stopped)
    }

    @Test func concurrentStopsShareOneDrainAndBlockRestart() async throws {
        let model = MadBStateModel<Int>()
        let source = ControlledSource(holdDrain: true)
        try model.start { source.source }
        let first = Task { try await model.stopAndWait() }
        await source.waitForDrain()
        let second = Task { try await model.stopAndWait() }
        #expect(throws: MadBObservationError.alreadyActive) {
            try model.start { source.source }
        }
        source.releaseDrain()
        try await first.value
        try await second.value
        #expect(source.counts.drains == 1)
    }

    @Test func restartClearsOldSnapshotAndHasNoInventedReplay() async throws {
        let model = MadBStateModel<Int>()
        let first = ControlledSource()
        try model.start { first.source }
        await first.waitForCollection()
        await first.emit(1)
        try await model.stopAndWait()
        #expect(model.value == 1)
        let second = ControlledSource()
        try model.start { second.source }
        #expect(model.value == nil)
        await first.emit(90)
        #expect(model.value == nil)
        await second.waitForCollection()
        await second.emit(2)
        #expect(model.value == 2)
        try await model.stopAndWait()
    }

    @Test func collectionErrorDoesNotHideDrainErrorAndDrainCanRetry() async throws {
        let model = MadBStateModel<Int>()
        let source = ControlledSource(drainFailure: .drain)
        try model.start { source.source }
        await source.waitForCollection()
        source.finish(.failure(TestFailure.collection))
        // Join this attempt before it has completed, so it does not mean a retry.
        do { try await model.stopAndWait(); Issue.record("drain failure was swallowed") }
        catch { #expect(error as? TestFailure == .drain) }
        #expect(model.collectionFailure as? TestFailure == .collection)
        #expect(model.stopFailure as? TestFailure == .drain)
        #expect(model.status == .stopFailed)
        #expect(throws: MadBObservationError.alreadyActive) {
            try model.start { source.source }
        }
        try await model.stopAndWait()
        #expect(source.counts.drains == 2)
        #expect(model.status == .stopped)
        #expect(model.stopFailure == nil)
        #expect(model.collectionFailure as? TestFailure == .collection)
    }

    @Test func naturalCompletionAutomaticallyRequestsAndConfirmsStop() async throws {
        let model = MadBStateModel<Int>()
        let source = ControlledSource(holdDrain: true)
        try model.start { source.source }
        await source.waitForCollection()
        source.finish()
        await source.waitForDrain()
        #expect(model.status == .stopping)
        #expect(source.counts.stops > 0)
        source.releaseDrain()
        try await model.stopAndWait()
        #expect(model.collectionFailure == nil)
        #expect(model.status == .stopped)
    }

    @Test func noTaskModelRetainCycleAndDeinitDoesNotPretendToConfirmDrain() async throws {
        let source = ControlledSource(holdDrain: true)
        var model: MadBStateModel<Int>? = MadBStateModel<Int>()
        weak var weakModel = model
        try model!.start { source.source }
        await source.waitForCollection()
        model = nil
        #expect(weakModel == nil)
        weakModel = nil
        await source.waitForDrain()
        #expect(!source.counts.cancelledDrain)
        source.releaseDrain()
    }
}
