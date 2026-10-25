package com.madb.c07

import org.cpf.kotlin.akinterop.annotation.KNExportFunction
import org.cpf.kotlin.akinterop.generate.ManagedBoundaryFailure

// Every native callable declaration and the Snapshot/dispatcher metadata is
// consumed by the actual stock KSP plus the bounded record exporter.
@KNExportFunction(name = "") fun ownedOpen(observer: RuntimeDispatcher): String = ownedBoundary { OwnedRuntime.open(observer) }
@KNExportFunction(name = "") fun ownedConnect(runtime: String): String = ownedBoundary { OwnedRuntime.connect(runtime) }
@KNExportFunction(name = "") fun ownedRoundtrip(snapshot: Snapshot, engine: String): Snapshot = ownedBoundary { OwnedRuntime.roundtrip(snapshot, engine) }
@KNExportFunction(name = "") fun ownedSubmit(snapshot: Snapshot, engine: String): String = ownedBoundary { OwnedRuntime.start(snapshot, engine) }
@KNExportFunction(name = "") fun ownedPublish(snapshot: Snapshot, engine: String, stream: Int): Unit = ownedBoundary { OwnedRuntime.publish(snapshot, engine, stream) }
@KNExportFunction(name = "") fun ownedObserve(engine: String, stream: Int): String = ownedBoundary { OwnedRuntime.observe(engine, stream) }
@KNExportFunction(name = "") fun ownedCancel(operation: String): Unit = ownedBoundary { OwnedRuntime.cancel(operation) }
@KNExportFunction(name = "") fun ownedRequestClose(handle: String): Unit = ownedBoundary { OwnedRuntime.requestClose(handle) }
@KNExportFunction(name = "") fun ownedClaimResult(operation: String, id: String): Unit = ownedBoundary { OwnedRuntime.claimResult(operation, id) }
@KNExportFunction(name = "") fun ownedWatchClose(handle: String, id: String): Int = ownedBoundary { OwnedRuntime.watchClose(handle, id) }
@KNExportFunction(name = "") fun ownedPeek(runtime: String, id: String): Int = ownedBoundary { OwnedRuntime.peek(runtime, id) }
@KNExportFunction(name = "") fun ownedTakeSnapshot(runtime: String, id: String): Snapshot = ownedBoundary { OwnedRuntime.takeSnapshot(runtime, id) }
@KNExportFunction(name = "") fun ownedTakeStatus(runtime: String, id: String): Int = ownedBoundary { OwnedRuntime.takeStatus(runtime, id) }
@KNExportFunction(name = "") fun ownedDeliver(subscription: String): Int = ownedBoundary { OwnedRuntime.deliver(subscription) }
@KNExportFunction(name = "") fun ownedRuntimeClose(runtime: String): Unit = ownedBoundary { OwnedRuntime.requestRuntimeClose(runtime) }

internal inline fun <T> ownedBoundary(block: () -> T): T = try { block() }
catch (failure: ManagedBoundaryFailure) { throw failure }
catch (_: OutOfMemoryError) { fail("ResourceExhausted") }
catch (_: Throwable) { OwnedRuntime.fault(); fail("InternalFailure") }
