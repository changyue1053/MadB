@file:OptIn(kotlinx.cinterop.ExperimentalForeignApi::class, kotlin.experimental.ExperimentalNativeApi::class)
package com.madb.c07

import kotlinx.atomicfu.atomic
import platform.posix.usleep
import org.cpf.kotlin.akinterop.annotation.KNExportFunction
import kotlinx.cinterop.*
import kotlinx.atomicfu.locks.synchronized
import com.madb.c07.executor.*

/** Mutable controls exist only in the test compilation. Owner callbacks below
 * run under OwnedRuntime's lock, except the explicitly atomic worker gates. */
internal object ExecutionProbe {
    // Diagnostics only. No pointer/resource retained after executor destruction.
    private val completedThread = ULongArray(21)
    private var completedThreadValid = false
    fun beforeExecutorDestroy(executor: CPointer<cnames.structs.madb_kn_executor>) = memScoped {
        val out = allocArray<ULongVar>(21)
        check(madb_kn_executor_test_command(executor, 0, out, 21) == 0)
        for (i in 0 until 21) completedThread[i] = out[i]
        completedThreadValid = true
    }
    fun afterExecutorDestroy(result: Int) {
        if (completedThreadValid) {
            if (result == 0) completedThread[19] = 1uL else completedThread[20]++
        }
    }
    fun threadProbe(action: Int, out: CPointer<ULongVar>, capacity: Int): Int {
        if (capacity != 21 || action !in 0..2) return 22
        val owner = OwnedRuntime.lifecycle
        if (owner != null) {
            val result = madb_kn_lifecycle_test_command(owner, action, out, capacity)
            if (result != 2) return result // ENOENT: executor already detached.
        }
        if (action != 0 || !completedThreadValid) return 2
        for (i in 0 until 21) out[i] = completedThread[i]
        return 0
    }
    // Read only while holding the existing OwnedRuntime lock. These counters
    // never own a model, pointer, transaction, or callback and never reset.
    private var ledgerCurrent = 0uL; private var ledgerPeak = 0uL
    private var ledgerCharged = 0uL; private var ledgerRetired = 0uL
    private var nativeCurrent = 0uL; private var nativePeak = 0uL
    private var nativeAllocatedBytes = 0uL; private var nativeFreedBytes = 0uL
    private var nativeAllocs = 0uL; private var nativeFrees = 0uL
    private var stagingCurrent = 0uL; private var stagingPeak = 0uL
    private var stagingAdded = 0uL; private var stagingCommitted = 0uL; private var stagingAbandoned = 0uL
    fun nativeAllocated(size: Int) { val n = size.toULong(); nativeCurrent += n; nativePeak = maxOf(nativePeak, nativeCurrent); nativeAllocatedBytes += n; nativeAllocs++ }
    fun nativeFreed(size: Int) { val n = size.toULong(); nativeCurrent -= n; nativeFreedBytes += n; nativeFrees++ }
    fun ledgerCharge(size: Int) { val n = size.toULong(); ledgerCurrent += n; ledgerPeak = maxOf(ledgerPeak, ledgerCurrent); ledgerCharged += n }
    fun ledgerRetire(size: Int) { val n = size.toULong(); ledgerCurrent -= n; ledgerRetired += n }
    fun stagingAdd(size: Int) { val n = size.toULong(); stagingCurrent += n; stagingPeak = maxOf(stagingPeak, stagingCurrent); stagingAdded += n }
    fun stagingEnd(size: Int, committed: Boolean) { val n = size.toULong(); stagingCurrent -= n; if (committed) stagingCommitted += n else stagingAbandoned += n }
    fun memoryMetric(index: Int): ULong {
        val utf = org.cpf.kotlin.akinterop.generate.ManagedRecordMemoryProbe
        return when(index) {
            0 -> 70603uL; 1 -> ledgerCurrent; 2 -> ledgerPeak; 3 -> ledgerCharged; 4 -> ledgerRetired
            5 -> nativeCurrent; 6 -> nativePeak; 7 -> nativeAllocatedBytes; 8 -> nativeFreedBytes
            9 -> nativeAllocs; 10 -> nativeFrees; 11 -> stagingCurrent; 12 -> stagingPeak
            13 -> stagingAdded; 14 -> stagingCommitted; 15 -> stagingAbandoned
            16 -> utf.current; 17 -> utf.peak; 18 -> utf.allocatedBytes; 19 -> utf.freedBytes
            20 -> utf.allocations; 21 -> utf.releases; else -> ULong.MAX_VALUE
        }
    }

    fun bindNapi(env: kotlinx.cinterop.COpaquePointer?) { com.madb.c07.executor.madb_kn_provider_napi_bind(env) }
    private val work = atomic(false); private val source = atomic(false); private val terminal = atomic(false)
    private val workEntered = atomic(0); private val sourceEntered = atomic(0)
    private val terminalEntered = atomic(0); private val heldPhase = atomic(0)
    private var workGeneration = 0uL; private var workId = 0uL; private var workTerminal = -1
    var deliveryKey: String? = null; private set
    var deliveryState = 0; private set
    private var deliveryId = 0uL; private var takes = 0uL; private var invokes = 0uL; private var discards = 0uL
    private var publicationState = 0; private var publicationSelected = 0; private var stagingBytes = 0
    private var reservations = 0uL; private var commits = 0uL; private var abandons = 0uL
    private var cloneNth = 0; private var cloneMode = -1
    private var cloneCalls = 0uL; private var cloneHits = 0uL; private var allocated = 0uL; private var released = 0uL
    private var transferPhase = 0; private val transferHits = Array(3) { 0uL }
    private var guards = 0uL; private var guardReleases = 0uL; private var rollbacks = 0uL
    fun admissionStage(requested: Int): Int = requested
    fun beginEpoch() {
        completedThread.fill(0uL); completedThreadValid = false
        work.value = false; source.value = false; terminal.value = false; heldPhase.value = 0
        workEntered.value = 0; sourceEntered.value = 0; terminalEntered.value = 0
        workGeneration = 0uL; workId = 0uL; workTerminal = -1
        deliveryKey = null; deliveryState = 0; deliveryId = 0uL; takes = 0uL; invokes = 0uL; discards = 0uL
        publicationState = 0; publicationSelected = 0; stagingBytes = 0; reservations = 0uL; commits = 0uL; abandons = 0uL
        cloneNth = 0; cloneMode = -1; cloneCalls = 0uL; cloneHits = 0uL; allocated = 0uL; released = 0uL
        transferPhase = 0; transferHits.fill(0uL); guards = 0uL; guardReleases = 0uL; rollbacks = 0uL
    }
    fun selectedWork(generation: ULong, id: ULong) { workGeneration = generation; workId = id; workTerminal = -1 }
    fun committedWork(status: Int) { workTerminal = status }
    fun work(allowed: () -> Boolean) {
        if (work.value) { workEntered.incrementAndGet(); heldPhase.value = 1 }
        try { while (work.value && allowed()) usleep(1000u) } finally { heldPhase.value = 0 }
    }
    fun beforeWorkExit(allowed: () -> Boolean) {
        if (terminal.value) { terminalEntered.incrementAndGet(); heldPhase.value = 2 }
        try { while (terminal.value && allowed()) usleep(1000u) } finally { heldPhase.value = 0 }
    }
    fun source(allowed: () -> Boolean) { if (source.value) sourceEntered.incrementAndGet(); while (source.value && allowed()) usleep(1000u) }
    fun set(kind: Int, enabled: Int) {
        if (kind !in 0..2 || enabled !in 0..1) fail("InvalidArgument")
        when (kind) { 0 -> work.value = enabled != 0; 1 -> source.value = enabled != 0; 2 -> terminal.value = enabled != 0 }
    }
    fun endEngine() {
        work.value = false; source.value = false; terminal.value = false
        cloneNth = 0; cloneMode = -1; transferPhase = 0
        if (deliveryState == 1) deliveryState = 0
        deliveryKey = null
    }
    fun stoppedSubscription(id: ULong) {
        if (deliveryState == 1 && deliveryId == id) { deliveryState = 0; deliveryKey = null }
    }
    fun armDelivery(key: String, id: ULong) {
        if (deliveryState == 1 || deliveryState == 2) fail("ResourceExhausted")
        deliveryKey = key; deliveryId = id; deliveryState = 1
    }
    fun takeDelivery(key: String, id: ULong): Boolean {
        takes++
        if (deliveryState != 1 || deliveryKey != key) return false
        deliveryState = 2; deliveryId = id; return true
    }
    fun releaseDelivery() { if (deliveryState != 2) fail("InvalidArgument"); deliveryState = 3; deliveryKey = null }
    fun finishDelivery(invoked: Boolean) { if (invoked) invokes++ else discards++ }
    fun publicationHeld(selected: Int, bytes: Int) { publicationState = 1; publicationSelected = selected; stagingBytes = bytes; reservations++ }
    fun publicationDone(committed: Boolean) { publicationState = if (committed) 2 else 3; stagingBytes = 0; if (committed) commits++ else abandons++ }
    fun armClone(nth: Int, mode: Int) {
        if (nth !in 1..8 || mode !in 0..1) fail("InvalidArgument")
        if (cloneNth != 0) fail("ResourceExhausted")
        cloneNth = nth; cloneMode = mode
    }
    fun cloneFault(): Int {
        cloneCalls++
        if (cloneNth == 0 || --cloneNth != 0) return -1
        cloneHits++; return cloneMode.also { cloneMode = -1 }
    }
    fun allocated() { allocated++ }
    fun released() { released++ }
    fun armTransfer(phase: Int) {
        if (phase !in 1..3) fail("InvalidArgument")
        if (transferPhase != 0) fail("ResourceExhausted")
        transferPhase = phase
    }
    fun transfer(phase: Int) {
        if (transferPhase != phase) return
        transferPhase = 0; transferHits[phase - 1]++; fail("BridgeFailure")
    }
    fun guardEntered() { guards++ }
    fun guardReleased() { guardReleases++ }
    fun claimRolledBack() { rollbacks++ }
    fun frame(): String = listOf(
        1, workEntered.value, terminalEntered.value, heldPhase.value, workGeneration, workId, workTerminal,
        deliveryState, deliveryId, takes, invokes, discards,
        publicationState, publicationSelected, stagingBytes, reservations, commits, abandons,
        cloneCalls, cloneHits, allocated, released, allocated - released, cloneNth,
        transferPhase, transferHits[0], transferHits[1], transferHits[2], guards, guardReleases, rollbacks
    ).joinToString(",")
    fun metric(index: Int): ULong = when(index) { 0 -> workEntered.value.toULong(); 1 -> sourceEntered.value.toULong(); 2 -> if(work.value) 1uL else 0uL; else -> if(source.value) 1uL else 0uL }
}

@KNExportFunction(name = "") fun ownedTestGate(runtime: String, kind: Int, enabled: Int): Unit = ownedBoundary {
    OwnedRuntime.testGate(runtime, kind, enabled)
}
@KNExportFunction(name = "") fun ownedTestDelivery(subscription: String, action: Int): String = ownedBoundary {
    OwnedRuntime.testDelivery(subscription, action)
}
@KNExportFunction(name = "") fun ownedTestPublication(snapshot: Snapshot, engine: String, stream: Int): Unit = ownedBoundary {
    OwnedRuntime.testPublication(snapshot, engine, stream)
}
@KNExportFunction(name = "") fun ownedTestReleasePublication(engine: String, commit: Int): Unit = ownedBoundary {
    OwnedRuntime.testReleasePublication(engine, commit)
}
@KNExportFunction(name = "") fun ownedTestClone(engine: String, nth: Int, mode: Int): Unit = ownedBoundary {
    OwnedRuntime.testClone(engine, nth, mode)
}
@KNExportFunction(name = "") fun ownedTestTransfer(runtime: String, phase: Int): Unit = ownedBoundary {
    OwnedRuntime.testTransfer(runtime, phase)
}
@KNExportFunction(name = "") fun ownedTestInspect(runtime: String): String = ownedBoundary { OwnedRuntime.testInspect(runtime) }
@KNExportFunction(name = "") fun ownedTestHandle(handle: String, kind: Int, probeCase: Int): Unit = ownedBoundary {
    OwnedRuntime.testHandle(handle, kind, probeCase)
}

@kotlin.native.CName("madb_kn_owned_memory_metric")
fun ownedMemoryMetric(index: Int): ULong = try { OwnedRuntime.memoryMetric(index) } catch (_: Throwable) { ULong.MAX_VALUE }

@kotlin.native.CName("madb_kn_owned_thread_probe")
fun ownedThreadProbe(epoch: ULong, action: Int, out: CPointer<ULongVar>?, capacity: Int): Int = try {
    if (madb_kn_is_main_thread() != 1) 1
    else if (out == null || capacity != 21 || !OwnedRuntime.epochMatches(epoch)) 22
    else synchronized(OwnedRuntime) {
        val result = ExecutionProbe.threadProbe(action, out, capacity)
        if (result == 0) { out[1] = epoch; out[3] = 1uL }
        result
    }
} catch (_: Throwable) { 5 }
