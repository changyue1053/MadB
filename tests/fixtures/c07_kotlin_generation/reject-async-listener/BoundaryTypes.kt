@file:OptIn(kotlinx.cinterop.ExperimentalForeignApi::class)
package com.madb.c07

import org.cpf.kotlin.akinterop.annotation.*

@Target(AnnotationTarget.CLASS)
@Retention(AnnotationRetention.SOURCE)
annotation class MadbValueRecord

@MadbValueRecord
@KNExportClass(name = "")
// The real KSP declaration remains the sole record schema/codec source.
data class Snapshot(@property:KNExportField(name = "") val value: Int, @property:KNExportField(name = "") val text: String)

@KNExportInterface(exportAll = true)
interface RuntimeDispatcher {
    fun onReady()
    suspend fun onValue(handle: String, snapshot: Snapshot)
    fun onClosed(handle: String, status: Int)
}
