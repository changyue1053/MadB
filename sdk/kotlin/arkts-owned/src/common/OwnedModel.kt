@file:OptIn(kotlinx.cinterop.ExperimentalForeignApi::class)
package com.madb.c07

import kotlinx.cinterop.*
import org.cpf.kotlin.akinterop.generate.ManagedBoundaryFailure
import org.cpf.kotlin.akinterop.generate.ManagedRecordValues

/** Kotlin owns and explicitly frees this native UTF-8 allocation. The C/C++
 * executor never sees its pointer, text or business value. Managed object/string
 * heap sizes are excluded from requested-byte observation; 4096 is the canonical retained budget. */
internal data class ModelClone(val model: OwnedModel?, val error: String? = null)
internal class OwnedModel private constructor(var value: Int, val size: Int, private var text: CPointer<ByteVar>?) {
    companion object {
        fun copy(snapshot: Snapshot, failure: Int = -1): ModelClone {
            ManagedRecordValues.validateString(snapshot.text, "text")
            if (failure == 0) return ModelClone(null, "ResourceExhausted")
            val bytes = snapshot.text.encodeToByteArray()
            val memory = nativeHeap.allocArray<ByteVar>(maxOf(1, bytes.size))
            ExecutionProbe.nativeAllocated(maxOf(1, bytes.size))
            try {
                for (i in bytes.indices) memory[i] = bytes[i]
                val model = OwnedModel(snapshot.value, bytes.size, memory)
                return ModelClone(model, if (failure == 1) "ResourceExhausted" else null)
            } catch (failure: Throwable) { nativeHeap.free(memory); ExecutionProbe.nativeFreed(maxOf(1, bytes.size)); throw failure }
        }
    }
    fun snapshot(): Snapshot {
        val ptr = text ?: throw ManagedBoundaryFailure("InternalFailure")
        return Snapshot(value, ptr.readBytes(size).decodeToString())
    }
    fun increment(): Int {
        if (text == null) return 10
        if (value == Int.MAX_VALUE) return 8
        value += 1
        return 0
    }
    fun release() {
        val ptr = text ?: throw ManagedBoundaryFailure("InternalFailure")
        nativeHeap.free(ptr)
        ExecutionProbe.nativeFreed(maxOf(1, size))
        text = null
    }
}
