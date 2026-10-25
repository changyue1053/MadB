@file:OptIn(kotlinx.cinterop.ExperimentalForeignApi::class, kotlin.experimental.ExperimentalNativeApi::class)
package com.madb.c07

import kotlinx.atomicfu.locks.SynchronizedObject
import kotlinx.atomicfu.locks.synchronized
import kotlinx.cinterop.*
import cnames.structs.madb_kn_executor
import cnames.structs.madb_kn_lifecycle
import com.madb.c07.executor.*
import org.cpf.kotlin.akinterop.OhosFFIContext
import org.cpf.kotlin.akinterop.generate.ManagedBoundaryFailure
import org.cpf.kotlin.akinterop.generate.ManagedOwnedCall
import org.cpf.kotlin.akinterop.generate.ManagedReturnOwner
import org.cpf.kotlin.akinterop.types.ArkProxy
import org.cpf.kotlin.akinterop.types.tsfnRegister

internal fun fail(code: String): Nothing = throw ManagedBoundaryFailure(code)
internal const val OK = 0
internal const val CANCELLED = 5
internal const val NOT_READY = 7
internal const val DOMAIN = 8
internal const val CALLBACK_FAILURE = 9
internal const val INTERNAL = 10

internal class Op(val id: ULong, var model: OwnedModel?) {
    var queued = true; var running = false; var terminal = -1
    var claimed = false; var consumed = false; var closing = false; var acked = false
    var waiters = 0
    fun businessClosed() = !queued && !running && model == null && closing && (!claimed || consumed)
}
internal class Ticket(val sub: Sub, var model: OwnedModel) {
    var source = true; var held = false; var entered = false
}
internal class Sub(val id: ULong, val state: Boolean) {
    val pending = ArrayList<Ticket>(8)
    var entered: Ticket? = null; var staging = 0; var stopping = false; var outcome = OK; var acked = false
    var waiters = 0
    fun businessClosed() = stopping && pending.isEmpty() && entered == null && staging == 0
}
internal class Publication(val receivers: List<Sub>, val models: List<OwnedModel>, val tickets: List<Ticket?>, val event: Boolean) {
    val bytes: Int = models.sumOf { it.size }
}
internal class HeldDelivery(val key: String, val ticket: Ticket) { var released = false }
internal class Wait(val id: String, val op: Op?, val sub: Sub?, val result: Boolean) {
    var entered = false
}
internal class EngineState(val generation: ULong) {
    val ops = ArrayList<Op>(8); val subs = ArrayList<Sub>(8); val waiters = ArrayList<Wait>(32)
    var executor: CPointer<madb_kn_executor>? = null
    var nextId = 0uL; var closing = false; var joined = false; var published = false; var acked = false
    var bytes = 0; var eventTickets = 0; var stateTickets = 0; var sources = 0; var entered = 0
    var scratch = false; var returnGuards = 0
    var publication: Publication? = null; var heldDelivery: HeldDelivery? = null
    fun next(): ULong { if (nextId == ULong.MAX_VALUE) fail("ResourceExhausted"); return ++nextId }
    fun businessClosed() = closing && joined && ops.all { it.businessClosed() } &&
        subs.all { it.businessClosed() } && sources == 0 && entered == 0 && !scratch && returnGuards == 0 && publication == null && heldDelivery == null
}

/** The sole business owner. Native support has no access to these collections,
 * terminal values, byte buffers, capacity decisions or logical handle kinds. */
internal object OwnedRuntime : SynchronizedObject() {
    var lifecycle: CPointer<madb_kn_lifecycle>? = null
    private var observer: RuntimeDispatcher? = null
    private var opened = false; private var closing = false; private var envExiting = false
    private var generation = 0uL; private var engine: EngineState? = null
    private var nextObservation = 0uL
    private var dispatchDepth = 0; private var wakeLive = 0; private var wakePending = false
    private var failures = 0uL; private var allocations = 0uL; private var releases = 0uL
    private var workCalls = 0uL; private var workCompleted = 0uL; private var cancelled = 0uL
    private var discarded = 0uL; private var callbacks = 0uL; private var callbackFailures = 0uL
    private var wakes = 0uL; private var wakeDrops = 0uL; private var wakeRejected = 0uL
    private var joins = 0uL; private var workerStarts = 0uL; private var scratchAcquired = 0uL; private var scratchReleased = 0uL
    private var epoch = 0uL
    private var runtimeKey = ""
    fun epochMatches(value: ULong): Boolean = synchronized(this) { value != 0uL && value == epoch }
    fun beginEpoch(value: ULong) = synchronized(this) {
        check(value > epoch && lifecycle == null && engine == null && observer == null &&
            wakeLive == 0 && dispatchDepth == 0 && failures == 0uL)
        val key = "kn:$value" // allocate before resetting/publication
        epoch = value; runtimeKey = key; nextObservation = 0uL; opened = false; closing = false; envExiting = false
        ExecutionProbe.beginEpoch()
    }
    fun rollbackAdmission() {
        val proxy = synchronized(this) {
            check(engine == null && wakeLive == 0 && dispatchDepth == 0)
            closing = true
            val value = observer; observer = null; value
        }
        if (proxy != null) try { (proxy as ArkProxy).ref.close() }
        catch (failure: Throwable) { synchronized(this) { observer = proxy; failures++ }; throw failure }
    }

    private fun decimal(text: String): ULong {
        if (text.isEmpty() || text.length > 20 || text[0] == '0') fail("InvalidHandle")
        var value = 0uL
        for (ch in text) {
            if (ch !in '0'..'9') fail("InvalidHandle")
            val digit = (ch.code - 48).toULong()
            if (value > (ULong.MAX_VALUE - digit) / 10u) fail("InvalidHandle")
            value = value * 10u + digit
        }
        return value
    }
    fun validateRuntime(handle: String) = synchronized(this) { requireRuntime(handle, true) }
    private fun requireRuntime(handle: String, open: Boolean = false) {
        if (handle != runtimeKey) fail("InvalidHandle")
        if (!opened) fail("Closed")
        if (open && closing) fail("Closing")
        if (envExiting) fail("Closed")
    }
    private data class Target(val engine: EngineState, val kind: Char, val id: ULong)
    private fun target(handle: String, open: Boolean = false): Target {
        if (handle.length > 90) fail("InvalidHandle")
        val parts = handle.split(':')
        if (parts.size != 5 || parts[0] != "kn" || decimal(parts[1]) != epoch || parts[3].length != 1) fail("InvalidHandle")
        val gen = decimal(parts[2]); val kind = parts[3][0]
        val id = if (parts[4] == "0" && kind == 'e') 0uL else decimal(parts[4])
        val e = engine ?: fail("InvalidHandleRetired")
        if (gen != e.generation) fail("InvalidHandleRetired")
        if (kind != 'e' && kind != 'o' && kind != 's') fail("InvalidHandle")
        if (kind == 'e' && id != 0uL) fail("InvalidHandle")
        if (open && (closing || e.closing)) fail("Closing")
        if (envExiting) fail("Closed")
        return Target(e, kind, id)
    }
    private fun handle(e: EngineState, kind: Char, id: ULong = 0uL) = "kn:$epoch:${e.generation}:$kind:$id"
    private fun operation(t: Target): Op = if (t.kind != 'o') fail("InvalidHandle") else t.engine.ops.find { it.id == t.id } ?: fail("InvalidHandleRetired")
    private fun subscription(t: Target): Sub = if (t.kind != 's') fail("InvalidHandle") else t.engine.subs.find { it.id == t.id } ?: fail("InvalidHandleRetired")
    private fun clone(e: EngineState, value: Snapshot, scratch: Boolean = false): OwnedModel {
        val size = value.text.encodeToByteArray().size
        if (!scratch && e.bytes + size > 4096) fail("ResourceExhausted")
        val result = try { OwnedModel.copy(value, ExecutionProbe.cloneFault()) } catch (_: OutOfMemoryError) { fail("ResourceExhausted") }
        // Even an error carrying an allocated model first gets a paired owner.
        val model = result.model
        if (model != null) { allocations++; ExecutionProbe.allocated(); if (!scratch) { e.bytes += model.size; ExecutionProbe.ledgerCharge(model.size) } }
        if (result.error != null) {
            if (model != null) release(e, model, scratch)
            fail(result.error)
        }
        return model ?: fail("InternalFailure")
    }
    private fun release(e: EngineState, model: OwnedModel, scratch: Boolean = false) {
        model.release(); releases++; ExecutionProbe.released(); if (!scratch) { e.bytes -= model.size; ExecutionProbe.ledgerRetire(model.size) }
        if (e.bytes < 0) { failures++; fail("InternalFailure") }
    }
    private fun dropOp(e: EngineState, op: Op) {
        if (!op.running) { op.model?.let { release(e, it); discarded++ }; op.model = null }
    }
    private fun releaseTicket(e: EngineState, ticket: Ticket) {
        if (ticket.source) e.sources--
        if (ticket.sub.state) e.stateTickets-- else e.eventTickets--
        release(e, ticket.model); discarded++
    }
    private fun stopSub(e: EngineState, sub: Sub) {
        sub.stopping = true; ExecutionProbe.stoppedSubscription(sub.id)
        var index = 0
        while (index < sub.pending.size) {
            val t = sub.pending[index]
            if (t.held) { index++; continue }
            sub.pending.removeAt(index); releaseTicket(e, t)
        }
        // A taken pre-entry state lease can acquire a new source obligation
        // when replaced. Only an unselected source can be cancelled here.
        e.heldDelivery?.ticket?.let { ticket ->
            if (ticket.sub === sub && ticket.source && !ticket.held) {
                ticket.source = false; e.sources--
            }
        }
    }
    private fun closeOp(e: EngineState, op: Op) {
        op.closing = true
        if (op.terminal < 0) { op.terminal = CANCELLED; cancelled++ }
        if (op.queued) op.queued = false
        if (!op.claimed || op.consumed || op.terminal != OK) dropOp(e, op)
    }
    private fun closeEngine(e: EngineState) {
        e.closing = true
        e.ops.forEach { closeOp(e, it) }; e.subs.forEach { stopSub(e, it) }
        e.executor?.let { if (madb_kn_executor_request_stop(it) != 0) failures++ }
    }

    private class AdmissionReturn(val failure: () -> Unit) : ManagedReturnOwner {
        var admitted = false
        override fun releaseAfterEncoding(encoded: Boolean) { if (admitted && !encoded) failure() }
    }
    private fun admissionReturn(failure: () -> Unit): AdmissionReturn {
        val owner = AdmissionReturn(failure)
        ManagedOwnedCall.hold(owner)
        return owner
    }

    fun open(value: RuntimeDispatcher): String = synchronized(this) {
        if (opened || closing) fail("AlreadyInitialized")
        if (lifecycle == null) fail("InternalFailure")
        val receipt = admissionReturn { requestRuntimeClose(runtimeKey) }
        ManagedOwnedCall.claimArgument(value)
        observer = value; opened = true; receipt.admitted = true
        runtimeKey
    }
    fun connect(runtime: String): String {
        var admittedHandle: String? = null
        val receipt = admissionReturn { admittedHandle?.let { requestClose(it) } }
        val e = synchronized(this) {
            requireRuntime(runtime, true)
            if (engine != null || wakeLive != 0 || dispatchDepth != 0) fail("Closing")
            if (generation == ULong.MAX_VALUE) fail("ResourceExhausted")
            val created = EngineState(++generation)
            admittedHandle = handle(created, 'e')
            engine = created
            created
        }
        val executor = madb_kn_executor_create(e.generation) ?: run { synchronized(this) { engine = null }; fail("ResourceExhausted") }
        synchronized(this) { e.executor = executor }
        val owner = lifecycle ?: fail("InternalFailure")
        if (madb_kn_lifecycle_attach(owner, executor, e.generation) != 0) {
            val deleted = madb_kn_executor_destroy(executor)
            synchronized(this) { if (deleted == 0) { e.executor = null; engine = null } else failures++ }
            fail("InternalFailure")
        }
        if (madb_kn_executor_start(executor) != 0) {
            synchronized(this) { e.closing = true }
            madb_kn_lifecycle_request_join(owner)
            fail("ResourceExhausted")
        }
        return synchronized(this) { workerStarts++; e.published = true; receipt.admitted = true; admittedHandle!! }
    }
    fun roundtrip(snapshot: Snapshot, key: String): Snapshot = synchronized(this) {
        val t = target(key, true); if (t.kind != 'e') fail("InvalidHandle")
        val e = t.engine
        if (e.scratch) fail("ResourceExhausted")
        var model: OwnedModel? = null
        val owner = object : ManagedReturnOwner {
            override fun releaseAfterEncoding(encoded: Boolean): Unit = synchronized(this@OwnedRuntime) {
                model?.let { release(e, it, true) }; model = null
                e.scratch = false; scratchReleased++
            }
        }
        ManagedOwnedCall.hold(owner)
        e.scratch = true; scratchAcquired++
        model = clone(e, snapshot, true)
        model!!.snapshot()
    }
    fun start(snapshot: Snapshot, key: String): String {
        var admittedHandle: String? = null
        val receipt = admissionReturn { admittedHandle?.let { requestClose(it) } }
        val pair = synchronized(this) {
            val t = target(key, true); if (t.kind != 'e') fail("InvalidHandle")
            val e = t.engine; if (e.ops.size >= 8) fail("ResourceExhausted")
            val id = e.next()
            val outHandle = handle(e, 'o', id)
            val executor = e.executor ?: fail("InternalFailure")
            val op = Op(id, null)
            val pair = Pair(outHandle, executor)
            op.model = clone(e, snapshot)
            e.ops.add(op) // fixed capacity 8 was allocated at Engine admission
            admittedHandle = outHandle; receipt.admitted = true
            pair
        }
        if (madb_kn_executor_wake(pair.second) != 0) synchronized(this) { failures++ }
        return pair.first
    }
    fun cancel(key: String) {
        synchronized(this) {
            val t = target(key); val op = operation(t)
            if (op.terminal < 0) { op.terminal = CANCELLED; cancelled++ }
            if (op.queued) op.queued = false
            if (!op.running && op.terminal != OK) dropOp(t.engine, op)
        }
        ready()
    }
    fun observe(key: String, stream: Int): String = synchronized(this) {
        val t = target(key, true); if (t.kind != 'e' || stream !in 0..1) fail("InvalidArgument")
        val e = t.engine; if (e.subs.size >= 8) fail("ResourceExhausted")
        val sub = Sub(e.next(), stream == 0)
        val outHandle = handle(e, 's', sub.id)
        val receipt = admissionReturn { requestClose(outHandle) }
        e.subs.add(sub); receipt.admitted = true
        // No current/history slot exists before this subscription's first publish.
        outHandle
    }
    private fun preparePublication(e: EngineState, snapshot: Snapshot, stream: Int): Publication {
        if (stream !in 0..1) fail("InvalidArgument")
        val receivers = e.subs.filter { !it.stopping && it.state == (stream == 0) }
        val size = snapshot.text.encodeToByteArray().size
        val reservedEvents = e.publication?.let { if (it.event) it.receivers.size else 0 } ?: 0
        if (stream == 1 && e.eventTickets + reservedEvents + receivers.size > 8) fail("ResourceExhausted")
        // All clone bytes are staging: old state remains charged until commit.
        if (e.bytes + size * receivers.size > 4096) fail("ResourceExhausted")
        val models = ArrayList<OwnedModel>(receivers.size)
        try {
            receivers.forEach { val model = clone(e, snapshot); models.add(model); ExecutionProbe.stagingAdd(model.size) }
            // Allocate every possible ticket before mutating any recipient.
            val tickets = receivers.mapIndexed { index, sub -> Ticket(sub, models[index]) }
            return Publication(receivers, models, tickets, stream == 1)
        } catch (failure: Throwable) { models.forEach { release(e, it); ExecutionProbe.stagingEnd(it.size, false) }; throw failure }
    }
    private fun abandonPublication(e: EngineState, publication: Publication) {
        publication.models.forEach { release(e, it); ExecutionProbe.stagingEnd(it.size, false) }
    }
    private fun commitPublication(e: EngineState, publication: Publication): Boolean {
        // A held transaction cannot partially commit when any selected source stopped.
        if (closing || e.closing || envExiting || publication.receivers.any { it.stopping }) {
            abandonPublication(e, publication); return false
        }
        publication.receivers.forEachIndexed { index, sub ->
            val model = publication.models[index]
            val replacement = if (sub.state) sub.pending.firstOrNull()
                ?: e.heldDelivery?.ticket?.takeIf { it.sub === sub && !it.entered } else null
            if (replacement != null) {
                // A pre-entry lease is still the latest-state slot. Replace its
                // model without manufacturing a second ticket/entered callback.
                release(e, replacement.model); discarded++; replacement.model = model
                if (!replacement.source) { replacement.source = true; e.sources++ }
            } else {
                sub.pending.add(publication.tickets[index]!!); e.sources++
                if (sub.state) e.stateTickets++ else e.eventTickets++
            }
        }
        publication.models.forEach { ExecutionProbe.stagingEnd(it.size, true) }
        return true
    }
    private fun wakeExecutor(executor: CPointer<madb_kn_executor>?) {
        executor?.let { if (madb_kn_executor_wake(it) != 0) synchronized(this) { failures++ } }
    }
    fun publish(snapshot: Snapshot, key: String, stream: Int) {
        val executor = synchronized(this) {
            val t = target(key, true); if (t.kind != 'e') fail("InvalidArgument")
            val transaction = preparePublication(t.engine, snapshot, stream)
            check(commitPublication(t.engine, transaction))
            t.engine.executor
        }
        wakeExecutor(executor)
    }

    /** Coalesced notification only. Values and terminal state remain in this
     * ledger; a dropped native payload pays the same logical wake permit. */
    private fun ready() {
        val admittedEpoch = synchronized(this) {
            if (envExiting || observer == null || wakePending) 0uL
            else { wakePending = true; wakeLive++; wakes++; epoch }
        }
        if (admittedEpoch == 0uL) return
        try {
            tsfnRegister.enqueueOwned(OhosFFIContext.mainTid, dispose = {
                synchronized(this) { check(admittedEpoch == epoch); wakeLive--; wakePending = false; wakeDrops++; if (wakeLive < 0) failures++ }
            }) {
                val destination = synchronized(this) {
                    if (admittedEpoch != epoch || envExiting) null else observer?.also { dispatchDepth++ }
                }
                if (destination != null) try { destination.onReady() }
                catch (_: Throwable) { synchronized(this) { callbackFailures++ } }
                finally { synchronized(this) { dispatchDepth-- } }
            }
        } catch (_: Throwable) { synchronized(this) { wakeRejected++ } }
    }

    /** Called only from the executor's foreign pthread through a CName stub.
     * No NAPI or ArkValue access occurs here. Native wake/stop has no business
     * state, and the same lock linearizes cancellation and terminal assignment. */
    fun step(gen: ULong): Int {
        var op: Op? = null
        var ticket: Ticket? = null
        val e = synchronized(this) {
            val current = engine ?: return -1
            if (current.generation != gen || current.closing || envExiting) return -1
            op = current.ops.firstOrNull { it.queued }
            if (op != null) { op!!.queued = false; op!!.running = true; workCalls++; ExecutionProbe.selectedWork(current.generation, op!!.id) }
            else {
                ticket = current.subs.asSequence().flatMap { it.pending.asSequence() }
                    .firstOrNull { it.source && !it.held }
                    ?: current.heldDelivery?.ticket?.takeIf { it.source && !it.held }
                ticket?.held = true
            }
            current
        }
        val selected = op
        if (selected != null) {
            ExecutionProbe.work { synchronized(this) { !e.closing && !envExiting } }
            val outcome = try {
                // Cancellation may win while the business worker is running;
                // its model remains owned until this call actually returns.
                selected.model?.increment() ?: INTERNAL
            } catch (_: Throwable) { INTERNAL }
            synchronized(this) {
                if (selected.terminal < 0) selected.terminal = outcome
                ExecutionProbe.committedWork(selected.terminal)
            }
            ready()
            // Terminal is immutable now; Work and the physical worker still own
            // their leases, including across an explicit close request.
            ExecutionProbe.beforeWorkExit { synchronized(this) { !envExiting } }
            synchronized(this) {
                selected.running = false; workCompleted++
                if (selected.terminal != OK || selected.consumed || (selected.closing && !selected.claimed)) dropOp(e, selected)
            }
            ready(); return 1
        }
        val source = ticket ?: return 0
        ExecutionProbe.source { synchronized(this) { !e.closing && !source.sub.stopping && !envExiting } }
        synchronized(this) {
            source.held = false
            if (e.heldDelivery?.ticket === source) {
                // The Delivery lease owns this same ticket until its release;
                // this step returns only the actual source obligation.
                source.source = false; e.sources--
            } else if (source.sub.stopping || e.closing || envExiting) {
                source.sub.pending.remove(source); releaseTicket(e, source)
            } else { source.source = false; e.sources-- }
        }
        ready(); return 1
    }

    private fun validateWaitId(e: EngineState, id: String) {
        if (decimal(id) <= nextObservation) fail("InvalidArgument")
        if (e.waiters.any { it.id == id }) fail("InvalidArgument")
        if (e.waiters.size >= 32) fail("ResourceExhausted")
    }
    fun claimResult(key: String, id: String) = synchronized(this) {
        val t = target(key, true); val op = operation(t)
        if (op.closing) fail("Closing")
        if (op.claimed) fail("AlreadyConsumed")
        validateWaitId(t.engine, id)
        val wait = Wait(id, op, null, true)
        val receipt = admissionReturn {
            synchronized(this) {
                if (!t.engine.waiters.remove(wait)) { failures++; fail("InternalFailure") }
                op.waiters--; op.claimed = false; ExecutionProbe.claimRolledBack()
            }
        }
        t.engine.waiters.add(wait); op.waiters++; op.claimed = true
        nextObservation = decimal(id); receipt.admitted = true
        ExecutionProbe.transfer(1)
    }
    fun requestClose(key: String) {
        val join = synchronized(this) {
            val t = target(key)
            when (t.kind) {
                'e' -> closeEngine(t.engine)
                'o' -> closeOp(t.engine, operation(t))
                's' -> stopSub(t.engine, subscription(t))
            }
            if (t.kind == 'e' && !t.engine.joined) lifecycle else null
        }
        if (join != null && madb_kn_lifecycle_request_join(join) != 0) synchronized(this) { failures++ }
        ready()
    }
    fun watchClose(key: String, id: String): Int {
        // The cleanup request is independent of Promise/waiter admission.
        requestClose(key)
        return synchronized(this) {
            val t = target(key); val e = t.engine
            val op = if (t.kind == 'o') operation(t) else null
            val sub = if (t.kind == 's') subscription(t) else null
            if (decimal(id) <= nextObservation || e.waiters.any { it.id == id }) fail("InvalidArgument")
            val done = op?.businessClosed() ?: sub?.businessClosed() ?: e.businessClosed()
            if (done) return@synchronized sub?.outcome ?: OK
            validateWaitId(e, id)
            val wait = Wait(id, op, sub, false)
            val receipt = admissionReturn {
                synchronized(this) {
                    if (!e.waiters.remove(wait)) { failures++; fail("InternalFailure") }
                    op?.let { it.waiters-- }; sub?.let { it.waiters-- }
                    ExecutionProbe.claimRolledBack()
                }
            }
            e.waiters.add(wait); op?.let { it.waiters++ }; sub?.let { it.waiters++ }
            nextObservation = decimal(id); receipt.admitted = true
            ExecutionProbe.transfer(1)
            -1
        }
    }
    private fun waitStatus(e: EngineState, wait: Wait): Int {
        if (wait.entered) return NOT_READY
        if (wait.result) return wait.op!!.let {
            if (it.terminal < 0 || it.running || it.queued) NOT_READY else it.terminal
        }
        return if (wait.op?.businessClosed() ?: wait.sub?.businessClosed() ?: e.businessClosed())
            wait.sub?.outcome ?: OK else NOT_READY
    }
    fun peek(runtime: String, id: String): Int = synchronized(this) {
        requireRuntime(runtime)
        val e = engine ?: fail("InvalidHandleRetired")
        val wait = e.waiters.find { it.id == id } ?: fail("InvalidHandle")
        val status = waitStatus(e, wait)
        // Scalar private tag distinguishes a successful close receipt from a
        // successful Snapshot without adding another native value record.
        if (!wait.result && status == OK) 11 else status
    }
    private fun takeGuard(e: EngineState, wait: Wait) {
        if (wait.entered) fail("AlreadyConsumed")
        ExecutionProbe.transfer(2)
        val owner = object : ManagedReturnOwner {
            override val consumesDelivery: Boolean = true
            override fun releaseAfterEncoding(encoded: Boolean): Unit = synchronized(this@OwnedRuntime) {
                if (wait.result) {
                    val op = wait.op!!
                    op.consumed = true; dropOp(e, op)
                }
                if (!e.waiters.remove(wait)) { failures++; fail("InternalFailure") }
                wait.op?.let { it.waiters-- }; wait.sub?.let { it.waiters-- }
                e.returnGuards--; ExecutionProbe.guardReleased()
            }
        }
        ManagedOwnedCall.hold(owner)
        wait.entered = true; e.returnGuards++; ExecutionProbe.guardEntered()
        ExecutionProbe.transfer(3)
    }
    fun takeSnapshot(runtime: String, id: String): Snapshot = synchronized(this) {
        requireRuntime(runtime)
        val e = engine ?: fail("InvalidHandleRetired")
        val wait = e.waiters.find { it.id == id } ?: fail("InvalidHandle")
        if (!wait.result || waitStatus(e, wait) != OK) fail("InvalidArgument")
        val model = wait.op!!.model ?: fail("InternalFailure")
        takeGuard(e, wait)
        model.snapshot()
    }
    fun takeStatus(runtime: String, id: String): Int = synchronized(this) {
        requireRuntime(runtime)
        val e = engine ?: fail("InvalidHandleRetired")
        val wait = e.waiters.find { it.id == id } ?: fail("InvalidHandle")
        val status = waitStatus(e, wait)
        if (status == NOT_READY || (wait.result && status == OK)) fail("InvalidArgument")
        takeGuard(e, wait); status
    }

    fun deliver(key: String): Int {
        val selected = synchronized(this) {
            val t = target(key); val sub = subscription(t)
            if (closing || t.engine.closing || sub.stopping) return 4
            if (sub.entered != null) return NOT_READY
            val ticket = sub.pending.firstOrNull() ?: return NOT_READY
            if (ticket.source || ticket.held) return NOT_READY
            val destination = observer ?: fail("InternalFailure")
            val hold = if (ExecutionProbe.deliveryKey == key) HeldDelivery(key, ticket) else null
            val delivery = Triple(t.engine, ticket, destination)
            sub.pending.removeAt(0); sub.entered = ticket
            if (ExecutionProbe.takeDelivery(key, sub.id)) {
                check(t.engine.heldDelivery == null && hold != null)
                t.engine.heldDelivery = hold
                return NOT_READY
            }
            ticket.entered = true; t.engine.entered++; dispatchDepth++
            delivery
        }
        return invokeDelivery(selected.first, selected.second, key, selected.third)
    }
    private fun invokeDelivery(e: EngineState, ticket: Ticket, key: String, destination: RuntimeDispatcher): Int {
        var outcome = OK
        try {
            val snapshot = ticket.model.snapshot()
            synchronized(this) { ExecutionProbe.finishDelivery(true) }
            destination.onValue(key, snapshot); synchronized(this) { callbacks++ }
        }
        catch (_: Throwable) {
            outcome = CALLBACK_FAILURE
            synchronized(this) { callbackFailures++; ticket.sub.outcome = CALLBACK_FAILURE; stopSub(e, ticket.sub) }
        } finally {
            synchronized(this) {
                ticket.sub.entered = null; ticket.entered = false; e.entered--; dispatchDepth--
                releaseTicket(e, ticket)
            }
        }
        return outcome
    }
    private fun progressReleasedDelivery() {
        val selected = synchronized(this) {
            val e = engine ?: return
            val held = e.heldDelivery ?: return
            if (!held.released || dispatchDepth != 0) return
            val ticket = held.ticket
            if (ticket.source || ticket.held) return
            // A stopped listener is never entered by test release. Replaced
            // state uses this same ticket; its source obligation finished above.
            if (envExiting || closing || e.closing || ticket.sub.stopping) {
                e.heldDelivery = null; ticket.sub.entered = null
                releaseTicket(e, ticket); ExecutionProbe.finishDelivery(false)
                return
            }
            val destination = observer ?: fail("InternalFailure")
            val delivery = Triple(e, held, destination)
            e.heldDelivery = null; ticket.entered = true; e.entered++; dispatchDepth++
            delivery
        }
        invokeDelivery(selected.first, selected.second.ticket, selected.second.key, selected.third)
    }

    /** Internal lifecycle receipts are delivered to the facade, never to the
     * business listener. Each slot is retained until the owner observes its ACK. */
    fun progress() {
        progressReleasedDelivery()
        while (true) {
            val receipt = synchronized(this) {
                if (envExiting || dispatchDepth != 0) return
                val e = engine ?: return
                val destination = observer ?: return
                val op = e.ops.firstOrNull { !it.acked && it.businessClosed() }
                val sub = if (op == null) e.subs.firstOrNull { !it.acked && it.businessClosed() } else null
                val isEngine = op == null && sub == null && !e.acked && e.businessClosed() &&
                    e.ops.all { it.acked } && e.subs.all { it.acked }
                if (op == null && sub == null && !isEngine) {
                    e.ops.removeAll { it.acked && it.waiters == 0 }
                    e.subs.removeAll { it.acked && it.waiters == 0 }
                    if (e.acked && e.waiters.isEmpty() && e.ops.isEmpty() && e.subs.isEmpty() && wakeLive == 0 &&
                        madb_kn_lifecycle_metric(13) == 0uL) { engine = null; ExecutionProbe.endEngine() } // frozen MADB_KN_LIFE_JOB_LIVE
                    return
                }
                val receipt = Receipt(e, op, sub, destination)
                dispatchDepth++
                receipt
            }
            try {
                receipt.observer.onClosed(handle(receipt.engine, if (receipt.op != null) 'o' else if (receipt.sub != null) 's' else 'e',
                    receipt.op?.id ?: receipt.sub?.id ?: 0uL), receipt.sub?.outcome ?: OK)
                synchronized(this) {
                    if (receipt.op != null) receipt.op.acked = true
                    else if (receipt.sub != null) receipt.sub.acked = true
                    else receipt.engine.acked = true
                }
            } catch (_: Throwable) { synchronized(this) { failures++ }; return }
            finally { synchronized(this) { dispatchDepth-- } }
        }
    }
    private class Receipt(val engine: EngineState, val op: Op?, val sub: Sub?, val observer: RuntimeDispatcher)

    fun requestRuntimeClose(runtime: String) {
        val owner = synchronized(this) {
            requireRuntime(runtime); closing = true
            engine?.let { closeEngine(it) }
            if (engine?.joined == false) lifecycle else null
        }
        if (owner != null && madb_kn_lifecycle_request_join(owner) != 0) synchronized(this) { failures++ }
    }
    fun prepareManagedShutdown(): Boolean {
        val proxy = synchronized(this) {
            if (!closing || envExiting || engine != null || dispatchDepth != 0 || wakeLive != 0 || failures != 0uL) return false
            val result = observer
            observer = null
            result
        }
        if (proxy != null) try { (proxy as ArkProxy).ref.close() }
        catch (_: Throwable) { synchronized(this) { observer = proxy; failures++ }; return false }
        return true
    }
    fun afterJoin(gen: ULong, result: Int, actualEnvExit: Boolean): Int {
        val pair = synchronized(this) {
            val e = engine ?: return if (result == 0) 0 else -1
            if (e.generation != gen || result != 0) { failures++; return -1 }
            val executor = e.executor ?: run { failures++; return -1 }
            Pair(e, executor)
        }
        val e = pair.first; val executor = pair.second
        val started = madb_kn_executor_metric(executor, 4)
        val joined = madb_kn_executor_metric(executor, 2)
        if (started != joined || madb_kn_executor_metric(executor, 3) != 0uL) {
            synchronized(this) { failures++ }; return -1
        }
        if (madb_kn_executor_destroy(executor) != 0) { synchronized(this) { failures++ }; return -1 }
        synchronized(this) {
            e.executor = null; e.joined = true; joins += joined
            // A source ticket may still be held if executor stop won before the
            // worker next visited it. Physical join makes this owner release safe.
            e.heldDelivery?.ticket?.let { ticket ->
                ticket.held = false
                if (ticket.source) { ticket.source = false; e.sources-- }
            }
            e.subs.forEach { sub -> sub.pending.forEach { it.held = false }; stopSub(e, sub) }
            if (actualEnvExit) {
                e.heldDelivery?.let { held ->
                    held.ticket.sub.entered = null; releaseTicket(e, held.ticket); ExecutionProbe.finishDelivery(false)
                }; e.heldDelivery = null
                e.waiters.forEach { w -> w.op?.let { it.waiters-- }; w.sub?.let { it.waiters-- } }
                e.waiters.clear()
                e.ops.forEach { it.consumed = true; closeOp(e, it); it.acked = true }
                e.subs.forEach { it.acked = true }
                if (e.businessClosed()) { engine = null; ExecutionProbe.endEngine() }
            } else if (!e.published && e.businessClosed()) { engine = null; ExecutionProbe.endEngine() }
        }
        return 0
    }
    fun beginActualEnvCleanup(): Int {
        val proxy = synchronized(this) {
            envExiting = true; closing = true
            // The native actual hook owns retirement now; no late pure disposal
            // may dereference this context after automatic retire frees it.
            lifecycle = null
            engine?.let { e ->
                closeEngine(e)
                e.publication?.let { transaction ->
                    transaction.receivers.forEach { it.staging-- }; abandonPublication(e, transaction); ExecutionProbe.publicationDone(false)
                }; e.publication = null
                e.heldDelivery?.let { held ->
                    if (held.ticket.held) held.released = true // worker owns source; join path disposes later
                    else {
                        if (held.ticket.source) { held.ticket.source = false; e.sources-- }
                        held.ticket.sub.entered = null; releaseTicket(e, held.ticket); ExecutionProbe.finishDelivery(false)
                        e.heldDelivery = null
                    }
                }
            }
            val value = observer; observer = null; value
        }
        if (proxy != null) try { (proxy as ArkProxy).ref.close() }
        catch (_: Throwable) { synchronized(this) { observer = proxy; failures++ }; return -1 }
        return 0
    }
    fun managedReady(actualEnvExit: Boolean): Int = synchronized(this) {
        if (failures != 0uL) return@synchronized -1
        val drained = engine == null && observer == null && dispatchDepth == 0
        // Actual hook removal cannot wait on native TSFN disposal, which OHOS
        // may defer until asynchronous env hooks return. Final ACK checks it.
        if (drained && (actualEnvExit || wakeLive == 0)) 1 else 0
    }
    fun retireLifecycle(): Int {
        val owner = synchronized(this) { lifecycle } ?: return 0
        val result = madb_kn_lifecycle_retire(owner)
        if (result == 0) synchronized(this) { lifecycle = null }
        return result
    }
    // Test declarations are compiled only by testSupportMain. These operations
    // borrow this same owner and do not create a second scheduler or value queue.
    fun testGate(runtime: String, kind: Int, enabled: Int) = synchronized(this) {
        requireRuntime(runtime, enabled != 0)
        ExecutionProbe.set(kind, enabled)
    }
    fun memoryMetric(index: Int): ULong = synchronized(this) { ExecutionProbe.memoryMetric(index) }
    fun testInspect(runtime: String): String = synchronized(this) {
        requireRuntime(runtime); ExecutionProbe.frame()
    }
    fun testDelivery(key: String, action: Int): String {
        var released = false
        return try { synchronized(this) {
            if (action !in 0..2) fail("InvalidArgument")
            val t = target(key, action == 0); val sub = subscription(t)
            when (action) {
                0 -> {
                    if (sub.stopping) fail("Closing")
                    if (t.engine.heldDelivery != null) fail("ResourceExhausted")
                    ExecutionProbe.armDelivery(key, sub.id)
                }
                2 -> {
                    val held = t.engine.heldDelivery ?: fail("InvalidArgument")
                    if (held.ticket.sub !== sub || held.released) fail("InvalidArgument")
                    ExecutionProbe.releaseDelivery(); held.released = true; released = true
                }
            }
            ExecutionProbe.frame()
        } } finally {
            // The real release must progress even if frame creation fails.
            // Native progress only: no listener on this support call stack.
            if (released) ready()
        }
    }
    fun testPublication(snapshot: Snapshot, key: String, stream: Int) = synchronized(this) {
        val t = target(key, true); if (t.kind != 'e') fail("InvalidHandle")
        val e = t.engine
        if (e.publication != null) fail("ResourceExhausted")
        val transaction = preparePublication(e, snapshot, stream)
        e.publication = transaction
        transaction.receivers.forEach { it.staging++ }
        ExecutionProbe.publicationHeld(transaction.receivers.size, transaction.bytes)
    }
    fun testReleasePublication(key: String, commit: Int) {
        if (commit !in 0..1) fail("InvalidArgument")
        var committed = false
        val executor = synchronized(this) {
            val t = target(key); if (t.kind != 'e') fail("InvalidHandle")
            val e = t.engine; val transaction = e.publication ?: fail("InvalidArgument")
            e.publication = null
            transaction.receivers.forEach { it.staging-- }
            committed = if (commit == 1) commitPublication(e, transaction)
                else { abandonPublication(e, transaction); false }
            ExecutionProbe.publicationDone(committed)
            e.executor
        }
        wakeExecutor(executor); ready()
        if (commit == 1 && !committed) fail("Closing")
    }
    fun testClone(key: String, nth: Int, mode: Int) = synchronized(this) {
        val t = target(key, true); if (t.kind != 'e') fail("InvalidHandle")
        ExecutionProbe.armClone(nth, mode)
    }
    fun testTransfer(runtime: String, phase: Int) = synchronized(this) {
        requireRuntime(runtime, true); ExecutionProbe.armTransfer(phase)
    }
    fun testHandle(key: String, kind: Int, case: Int) = synchronized(this) {
        if (kind !in 0..2 || case !in 0..5) fail("InvalidArgument")
        val expected = "eos"[kind]
        // The facade supplies only a current typed object's private handle.
        val original = target(key, true)
        if (original.kind != expected) fail("InvalidHandle")
        if (expected == 'o') operation(original)
        if (expected == 's') subscription(original)
        val parts = key.split(':').toMutableList()
        when (case) {
            1 -> parts[3] = if (expected == 'e') "o" else "e"
            2 -> {
                if (original.engine.generation <= 1uL) fail("InvalidArgument")
                parts[2] = (original.engine.generation - 1uL).toString()
            }
            3 -> parts[1] = "18446744073709551616"
            4 -> parts[1] = "0" + parts[1]
            5 -> parts[4] += "\u0000"
        }
        val admitted = target(parts.joinToString(":"), true)
        if (admitted.kind != expected) fail("InvalidHandle")
        if (expected == 'o') operation(admitted)
        if (expected == 's') subscription(admitted)
    }
    fun fault() = synchronized(this) { failures++ }
    /** Locked per-read metrics; cross-field live snapshots are not atomic. */
    fun metric(index: Int): ULong = synchronized(this) {
        val e = engine
        when (index) {
            0 -> failures; 1 -> allocations; 2 -> releases; 3 -> (allocations - releases)
            4 -> workerStarts; 5 -> joins; 6 -> workCalls; 7 -> workCompleted
            8 -> cancelled; 9 -> discarded; 10 -> callbacks; 11 -> callbackFailures
            12 -> wakes; 13 -> wakeDrops; 14 -> wakeRejected; 15 -> wakeLive.toULong()
            16 -> (e?.ops?.size ?: 0).toULong(); 17 -> (e?.subs?.size ?: 0).toULong()
            18 -> (e?.waiters?.size ?: 0).toULong(); 19 -> (e?.bytes ?: 0).toULong()
            20 -> ((e?.eventTickets ?: 0) + (e?.publication?.let { if (it.event) it.receivers.size else 0 } ?: 0)).toULong(); 21 -> (e?.stateTickets ?: 0).toULong()
            22 -> ((e?.sources ?: 0) + (e?.publication?.receivers?.size ?: 0)).toULong(); 23 -> (e?.entered ?: 0).toULong()
            24 -> dispatchDepth.toULong(); 25 -> scratchAcquired; 26 -> scratchReleased
            27 -> (e?.returnGuards ?: 0).toULong(); 28 -> generation
            29 -> if (closing) 1uL else 0uL; 30 -> if (envExiting) 1uL else 0uL
            31 -> if (engine != null) 1uL else 0uL; 32 -> if (observer != null) 1uL else 0uL
            33 -> ExecutionProbe.metric(0); 34 -> ExecutionProbe.metric(1)
            35 -> ExecutionProbe.metric(2); 36 -> ExecutionProbe.metric(3)
            else -> ULong.MAX_VALUE
        }
    }
}
