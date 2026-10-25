@file:OptIn(kotlinx.cinterop.ExperimentalForeignApi::class)
package com.madb.c07

import org.cpf.kotlin.akinterop.annotation.*

@Target(AnnotationTarget.CLASS)
@Retention(AnnotationRetention.SOURCE)
annotation class MadbValueRecord

@MadbValueRecord
@KNExportClass(name = "")
// The real KSP declaration remains the sole record schema/codec source.
data class Parcel(@property:KNExportField(name = "") val value: Int, @property:KNExportField(name = "") val text: String)

// Existing owned Core references resolve to the renamed actual Kotlin class.
typealias Snapshot = Parcel

@KNExportInterface(exportAll = true)
interface RuntimeDispatcher {
    fun onReady()
    fun onValue(handle: String, snapshot: Parcel)
    fun onClosed(handle: String, status: Int)
}
