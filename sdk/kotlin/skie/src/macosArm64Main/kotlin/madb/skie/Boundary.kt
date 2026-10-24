@file:OptIn(kotlinx.coroutines.ExperimentalCoroutinesApi::class, kotlinx.coroutines.DelicateCoroutinesApi::class)
package madb.skie

import kotlinx.coroutines.*
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flow
import platform.Foundation.NSLock

/** Phase is a native Swift enum in the SKIE consumer. */
enum class Phase { OPEN, CLOSING, CLOSED }
data class Value(val id: String, val sequence: Long?, val text: String, val bytes: ByteArray, val phase: Phase)
sealed class BoundaryEvent {
    data class Data(val value: Value) : BoundaryEvent()
    data class Failure(val code: String) : BoundaryEvent()
}
class BoundaryFailure(val code: String) : Exception(code)
/** Called synchronously on this engine's owned serial worker. Implementations must be thread safe. */
interface Host { fun transform(value: Value): Value }
data class Stats(val activeOperations: Int, val activeSubscriptions: Int, val queuedItems: Int,
    val retainedBytes: Long, val completedOperations: Int, val cancelledOperations: Int,
    val discardedValues: Int, val closed: Boolean, val activeWaiters: Int, val inflightItems: Int)

private const val MAX_PAYLOAD = 1024
private const val MAX_ITEMS = 8
private const val MAX_BYTES = 4096L
private fun Value.weight(): Long = id.encodeToByteArray().size.toLong() + text.encodeToByteArray().size +
    bytes.size
private fun Value.snapshot(): Value = copy(bytes = bytes.copyOf())
private fun Value.validate() { if (weight() > MAX_PAYLOAD) throw BoundaryFailure("ResourceExhausted") }
private inline fun <T> NSLock.guarded(action: () -> T): T { lock(); try { return action() } finally { unlock() } }

/** Reject the wrong resolved profile before allocating its worker or accepting business work. */
@Throws(BoundaryFailure::class)
fun openEngine(expectedProfile: String): Engine {
    if (expectedProfile != BuildIdentity.PROFILE) throw BoundaryFailure("ProfileMismatch")
    return Engine()
}
fun profileIdentity(): String = BuildIdentity.PROFILE
fun interfaceIdentity(): String = BuildIdentity.INTERFACE
fun nativeAbiVersion(): Int = 1
fun bootstrapVersion(): Int = 1
fun failureEvent(code: String): BoundaryEvent = BoundaryEvent.Failure(code)

/** Each instance owns exactly one serial worker. Control/cleanup never waits for its business queue. */
class Engine internal constructor() {
    internal val lock = NSLock()
    private val worker = newSingleThreadContext("MadBSkieOwner")
    private val ownerJob = SupervisorJob()
    internal val scope = CoroutineScope(worker + ownerJob)
    private var phase = Phase.OPEN
    private val operations = mutableSetOf<Operation>()
    private val subscriptions = mutableSetOf<Subscription>()
    private var completed = 0
    private var cancelled = 0
    internal var discarded = 0
    private val shutdownScope = CoroutineScope(Dispatchers.Default + SupervisorJob())
    private val shutdownComplete = CompletableDeferred<Unit>()
    private var shutdownStarted = false
    private var waiters = 0
    internal var completionProbe: CompletionProbe? = null

    private fun admit() { if (phase != Phase.OPEN) throw BoundaryFailure(phase.name.lowercase().replaceFirstChar { it.uppercase() }) }
    fun phase(): Phase = lock.guarded { phase }
    @Throws(BoundaryFailure::class)
    fun roundtrip(value: Value): Value = lock.guarded { admit(); value.validate(); value.snapshot() }

    @Throws(BoundaryFailure::class)
    fun startOperation(value: Value, host: Host): Operation = lock.guarded {
        admit(); value.validate()
        if (operations.size == MAX_ITEMS || retainedBytesLocked() + value.weight() > MAX_BYTES) throw BoundaryFailure("ResourceExhausted")
        Operation(this, value.snapshot(), host).also { operations.add(it) }
    }
    internal fun release(operation: Operation) { operations.remove(operation) }
    internal fun finished(wasCancelled: Boolean) { if (wasCancelled) cancelled++ else completed++ }
    internal fun checkResultCapacity(operation: Operation, resultWeight: Long) {
        if (retainedBytesLocked() - operation.retainedBytes + resultWeight > MAX_BYTES) throw BoundaryFailure("ResourceExhausted")
    }
    @Throws(BoundaryFailure::class)
    fun subscribeEvents(): Subscription = subscribe(false)
    @Throws(BoundaryFailure::class)
    fun subscribeState(): Subscription = subscribe(true)
    private fun subscribe(state: Boolean): Subscription = lock.guarded {
        admit()
        if (subscriptions.size == MAX_ITEMS) throw BoundaryFailure("ResourceExhausted")
        Subscription(this, state).also { subscriptions.add(it) }
    }
    @Throws(BoundaryFailure::class)
    fun publish(value: Value) {
        val observers = lock.guarded {
            admit(); value.validate()
            // Validate before fan-out: failure does not transfer any snapshot to observers.
            val eventPending = subscriptions.sumOf { it.pendingEventCount() }
            val eventReceivers = subscriptions.count { it.acceptsEvents() }
            if (eventPending + eventReceivers > MAX_ITEMS) throw BoundaryFailure("ResourceExhausted")
            val projected = retainedBytesLocked() + subscriptions.sumOf { it.additionalBytes(value.weight()) }
            if (projected > MAX_BYTES) throw BoundaryFailure("ResourceExhausted")
            subscriptions.toList().also { list -> list.forEach { it.enqueue(value.snapshot()) } }
        }
        observers.forEach { it.notifyChanged() }
    }
    internal fun detached(subscription: Subscription) { subscriptions.remove(subscription) }
    private fun retainedBytesLocked(): Long = operations.sumOf { it.retainedBytes } + subscriptions.sumOf { it.bytes }
    fun stats(): Stats = lock.guarded { Stats(operations.size, subscriptions.size,
        subscriptions.sumOf { it.pendingCount() }, retainedBytesLocked(),
        completed, cancelled, discarded, phase == Phase.CLOSED, waiters, subscriptions.sumOf { it.items - it.pendingCount() }) }

    fun requestClose() {
        val pending = lock.guarded {
            if (phase == Phase.CLOSED) return
            phase = Phase.CLOSING
            operations.toList() to subscriptions.toList().also { list -> list.forEach { it.stopLocked() } }
        }
        pending.second.forEach { it.notifyChanged() }
        pending.first.forEach { it.requestCancel() }
    }
    internal suspend fun <T> withWaitPermit(action: suspend () -> T): T {
        lock.guarded {
            if (waiters == 32) throw BoundaryFailure("ResourceExhausted")
            waiters++
        }
        try { return action() } finally { lock.guarded { waiters-- } }
    }
    @Throws(BoundaryFailure::class, CancellationException::class)
    suspend fun awaitClosed() = withWaitPermit {
        requestClose()
        val start = lock.guarded {
            if (shutdownStarted) false else { shutdownStarted = true; true }
        }
        if (start) shutdownScope.launch {
            try {
                val pending = lock.guarded { operations.toList() to subscriptions.toList() }
                pending.first.forEach { it.awaitFinished() }
                pending.second.forEach { it.awaitStoppedInternal() }
                ownerJob.cancelAndJoin()
                // close() joins its worker. It must run on the shared control executor,
                // outside the engine lock, including after a completion ACK resumes us.
                worker.close()
                lock.guarded { phase = Phase.CLOSED }
                shutdownComplete.complete(Unit)
            } catch (cause: Throwable) {
                shutdownComplete.completeExceptionally(cause)
            } finally {
                shutdownScope.cancel()
            }
        }
        // This caller owns only its wait; cancellation never abandons shutdown work.
        shutdownComplete.await()
    }
}

/** Cancellation is cooperative: a synchronous host callback must return before work is stopped. */
class Operation internal constructor(private val engine: Engine, private var input: Value?, private var host: Host?) {
    internal var retainedBytes = input!!.weight()
    private var started = false
    private var waiting = false
    private var cancelRequested = false
    private var result: Value? = null
    // Job completion precedes invokeOnCompletion cleanup; only this ACK proves cleanup.
    private val cleanupDone = CompletableDeferred<Unit>()
    private val task: Deferred<Unit> = engine.scope.async(start = CoroutineStart.LAZY) {
        val pair = engine.lock.guarded { input!! to host!! }
        ensureActive()
        val transformed = pair.second.transform(pair.first)
        ensureActive()
        transformed.validate()
        val snapshot = transformed.snapshot()
        engine.lock.guarded {
            engine.checkResultCapacity(this@Operation, snapshot.weight())
            input = null; host = null
            retainedBytes = snapshot.weight(); result = snapshot
        }
    }
    init {
        task.invokeOnCompletion { cause ->
            val probe = engine.lock.guarded { engine.completionProbe }
            probe?.pauseBeforeCleanup()
            engine.lock.guarded {
                input = null; host = null
                engine.finished(cause is CancellationException)
                if (cause != null || cancelRequested || (started && !waiting)) discardLocked()
            }
            // Resume outside the engine lock; a close waiter may immediately reenter it.
            probe?.pauseBeforeAcknowledgement()
            cleanupDone.complete(Unit)
            engine.lock.guarded { settleCleanedLocked() }
        }
    }
    private fun discardLocked() {
        if (result != null) { result = null; engine.discarded++ }
        retainedBytes = 0
        releaseIfCleanedLocked()
    }
    private fun releaseIfCleanedLocked() {
        if (cleanupDone.isCompleted && result == null) engine.release(this)
    }
    private fun settleCleanedLocked() {
        if (!cleanupDone.isCompleted) return
        // Cancellation may win between the first cleanup lock and the outside-lock ACK.
        if (cancelRequested || (started && !waiting)) discardLocked() else releaseIfCleanedLocked()
    }
    @Throws(BoundaryFailure::class, CancellationException::class)
    suspend fun run(): Value = engine.withWaitPermit {
        engine.lock.guarded {
            if (started) throw BoundaryFailure("AlreadyStarted")
            started = true; waiting = true
        }
        try {
            // Await cancellation stops this waiter; the owner keeps work until explicit requestCancel.
            task.await()
            cleanupDone.await()
            currentCoroutineContext().ensureActive()
            engine.lock.guarded {
                if (cancelRequested) throw CancellationException("Work cancelled")
                val delivered = result ?: throw BoundaryFailure("MissingResult")
                result = null; retainedBytes = 0; releaseIfCleanedLocked()
                delivered
            }
        } finally {
            engine.lock.guarded { waiting = false; settleCleanedLocked() }
        }
    }
    fun requestCancel() {
        engine.lock.guarded {
            cancelRequested = true
            settleCleanedLocked()
        }
        task.cancel()
    }
    internal suspend fun awaitFinished() {
        cleanupDone.await()
        engine.lock.guarded { settleCleanedLocked() }
    }
}

/** One collector per observation. All retained values, including an emitting value, count toward bytes. */
class Subscription internal constructor(private val engine: Engine, private val state: Boolean) {
    private val changed = Channel<Unit>(Channel.CONFLATED)
    private val pending = ArrayDeque<Value>()
    internal var items = 0
    internal var bytes = 0L
    private var stopping = false
    private var collecting = false
    private var everCollected = false
    private val stopped = CompletableDeferred<Unit>()
    val events: Flow<BoundaryEvent> = flow {
        val admitted = engine.lock.guarded {
            if (everCollected) false else { everCollected = true; collecting = !stopping; !stopping }
        }
        if (!admitted) return@flow
        try {
            while (true) {
                val next = engine.lock.guarded { if (stopping) null else pending.removeFirstOrNull() }
                if (next != null) {
                    try { emit(BoundaryEvent.Data(next)) }
                    finally { engine.lock.guarded { items--; bytes -= next.weight() } }
                } else {
                    if (engine.lock.guarded { stopping }) break
                    changed.receive()
                }
            }
        } finally {
            engine.lock.guarded { collecting = false; stopLocked() }
            notifyChanged()
        }
    }
    internal fun additionalBytes(weight: Long): Long = if (stopping) 0 else
        weight - (if (state) pending.firstOrNull()?.weight() ?: 0 else 0)
    internal fun pendingCount(): Int = pending.size
    internal fun acceptsEvents(): Boolean = !stopping && !state
    internal fun pendingEventCount(): Int = if (state) 0 else pending.size
    internal fun enqueue(value: Value) {
        if (stopping) return
        if (state) {
            val old = pending.removeFirstOrNull()
            if (old != null) { items--; bytes -= old.weight(); engine.discarded++ }
        }
        pending.addLast(value); items++; bytes += value.weight()
    }
    internal fun stopLocked() {
        if (!stopping) {
            stopping = true
            pending.forEach { items--; bytes -= it.weight(); engine.discarded++ }
            pending.clear()
        }
        if (!collecting) engine.detached(this)
    }
    internal fun notifyChanged() {
        changed.trySend(Unit)
        if (engine.lock.guarded { stopping && !collecting }) stopped.complete(Unit)
    }
    fun requestStop() { engine.lock.guarded { stopLocked() }; notifyChanged() }
    internal suspend fun awaitStoppedInternal() { requestStop(); stopped.await() }
    @Throws(BoundaryFailure::class, CancellationException::class)
    suspend fun awaitStopped() = engine.withWaitPermit { awaitStoppedInternal() }
}
