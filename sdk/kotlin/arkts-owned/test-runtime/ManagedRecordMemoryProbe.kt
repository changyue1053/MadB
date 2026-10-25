@file:OptIn(kotlinx.cinterop.ExperimentalForeignApi::class)
package org.cpf.kotlin.akinterop.generate

/** Test-only owner-thread counters of actual UTF16 array requested bytes.
 * memScoped exits before released() runs. No NAPI or object-heap estimate. */
object ManagedRecordMemoryProbe {
    var current = 0uL; private set
    var peak = 0uL; private set
    var allocatedBytes = 0uL; private set
    var freedBytes = 0uL; private set
    var allocations = 0uL; private set
    var releases = 0uL; private set
    fun allocated(bytes: Int) { val n = bytes.toULong(); current += n; peak = maxOf(peak, current); allocatedBytes += n; allocations++ }
    fun released(bytes: Int) { val n = bytes.toULong(); current -= n; freedBytes += n; releases++ }
}
