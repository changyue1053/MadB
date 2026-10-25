@file:OptIn(kotlin.native.runtime.NativeRuntimeApi::class)
package madb.skie

import platform.Foundation.NSCondition

/** Conformance-only hooks; none is used to implement business close or stop guarantees. */
object TestSupport {
    fun collectGarbage() { kotlin.native.runtime.GC.collect() }

    /** Arm before starting operations. A single engine owns at most one reusable release latch. */
    @Throws(BoundaryFailure::class)
    fun armCompletionCleanup(engine: Engine) { arm(engine, false) }
    @Throws(BoundaryFailure::class)
    fun armCompletionAcknowledgement(engine: Engine) { arm(engine, true) }
    private fun arm(engine: Engine, afterCleanup: Boolean) {
        engine.lock.lock()
        try {
            if (engine.completionProbe != null) throw BoundaryFailure("AlreadyArmed")
            engine.completionProbe = CompletionProbe(afterCleanup)
        } finally { engine.lock.unlock() }
    }
    fun completionCleanupEntered(engine: Engine): Boolean = probe(engine)?.hasEntered() ?: false
    fun releaseCompletionCleanup(engine: Engine) { probe(engine)?.release() }
    private fun probe(engine: Engine): CompletionProbe? {
        engine.lock.lock()
        try { return engine.completionProbe } finally { engine.lock.unlock() }
    }
}

internal class CompletionProbe(private val afterCleanup: Boolean) {
    private val condition = NSCondition()
    private var entered = false
    private var released = false
    fun pauseBeforeCleanup() { if (!afterCleanup) pause() }
    fun pauseBeforeAcknowledgement() { if (afterCleanup) pause() }
    private fun pause() {
        condition.lock()
        try {
            entered = true
            while (!released) condition.wait()
        } finally { condition.unlock() }
    }
    fun hasEntered(): Boolean {
        condition.lock()
        try { return entered } finally { condition.unlock() }
    }
    fun release() {
        condition.lock()
        try { released = true; condition.broadcast() } finally { condition.unlock() }
    }
}
