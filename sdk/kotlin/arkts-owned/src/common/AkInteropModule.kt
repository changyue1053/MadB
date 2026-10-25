@file:OptIn(kotlinx.cinterop.ExperimentalForeignApi::class, kotlin.experimental.ExperimentalNativeApi::class)
package com.madb.c07

import kotlinx.cinterop.*
import cnames.structs.madb_kn_executor
import cnames.structs.madb_kn_lifecycle
import com.madb.c07.executor.*
import org.cpf.kotlin.akinterop.OhosFFIContext
import org.cpf.kotlin.akinterop.OhosFFIConfig
import org.cpf.kotlin.akinterop.generated.registerKNExports
import platform.ArkTS.ArkTS_Napi_NativeModule.napi_env
import platform.ArkTS.ArkTS_Napi_NativeModule.napi_value
import platform.cpf.kotlin.akinterop.managedShutdownMetric

/** Called only by the single native bootstrap AFTER native main + identity
 * checks. Every CName catches Kotlin failures before its C ABI return. */
@CName("madb_kn_owned_initialize")
fun initializeOwned(env: napi_env, exports: napi_value, epoch: ULong, failAfterStage: Int): Int = try {
    ExecutionProbe.bindNapi(env)
    check(OwnedRuntime.lifecycle == null)
    OhosFFIContext.installManagedSchema {
        OhosFFIContext.configure(OhosFFIConfig(managedShutdownGlobalsOnly = true))
        installGeneratedRecordCodec()
        registerKNExports(env, exports)
    }
    OwnedRuntime.beginEpoch(epoch)
    OhosFFIContext.init(env, exports, "com.madb.owned", "entry", epoch = epoch, failAfterStage = ExecutionProbe.admissionStage(failAfterStage))
    0
} catch (_: Throwable) { -1 } // Historical admission failure is not a live owner fault.

@CName("madb_kn_owned_init_metric")
fun initializationMetric(index: Int): ULong = try {
    when(index) { 0 -> OhosFFIContext.epoch; 1 -> OhosFFIContext.initializationStage.toULong(); else -> ULong.MAX_VALUE }
} catch (_: Throwable) { ULong.MAX_VALUE }

private fun callbackEpoch(context: COpaquePointer?): Boolean =
    OwnedRuntime.epochMatches(context?.rawValue?.toLong()?.toULong() ?: 0uL)

/** Native async hook is installed AFTER the three managed cleanup hooks so
 * its actual entry can gate/stop first and remove the subordinate hooks. */
@CName("madb_kn_owned_bind_lifecycle")
fun bindLifecycle(owner: CPointer<madb_kn_lifecycle>?, epoch: ULong): Int = try {
    check(OwnedRuntime.epochMatches(epoch) && owner != null && OwnedRuntime.lifecycle == null)
    OwnedRuntime.lifecycle = owner; 0
} catch (_: Throwable) { OwnedRuntime.fault(); -1 }

@CName("madb_kn_owned_init_rollback")
fun rollbackInitialization(env: napi_env, epoch: ULong): Int = try {
    if (OhosFFIContext.epoch != epoch) 0 else {
        OwnedRuntime.rollbackAdmission()
        val status = OhosFFIContext.requestManagedShutdown(env)
        if (status == 0) OwnedRuntime.retireLifecycle() else status
    }
}
catch (_: Throwable) { OwnedRuntime.fault(); -1 }

@CName("madb_kn_business_step")
fun businessStep(generation: ULong): Int = try { OwnedRuntime.step(generation) }
catch (_: Throwable) { OwnedRuntime.fault(); -2 }

@CName("madb_kn_owned_env_closing")
fun actualEnvClosing(context: COpaquePointer?): Int = try {
    check(callbackEpoch(context)) { "OldLifecycleEpoch" }
    OhosFFIContext.beginActualEnvCleanup(OhosFFIContext.mainEnv) { OwnedRuntime.beginActualEnvCleanup() }
} catch (_: Throwable) { if (callbackEpoch(context)) OwnedRuntime.fault(); -1 }

@CName("madb_kn_owned_after_join")
fun afterJoin(context: COpaquePointer?, generation: ULong, joinResult: Int, envClosing: Int): Int = try {
    check(callbackEpoch(context)) { "OldLifecycleEpoch" }
    OwnedRuntime.afterJoin(generation, joinResult, envClosing != 0)
} catch (_: Throwable) { if (callbackEpoch(context)) OwnedRuntime.fault(); -1 }

@CName("madb_kn_owned_managed_ready")
fun managedReady(context: COpaquePointer?, envClosing: Int): Int = try {
    check(callbackEpoch(context)) { "OldLifecycleEpoch" }
    if (managedShutdownMetric(15) != 0uL) -1 else OwnedRuntime.managedReady(envClosing != 0)
} catch (_: Throwable) { if (callbackEpoch(context)) OwnedRuntime.fault(); -1 }

@CName("madb_kn_owned_poll")
fun pollOwned(env: napi_env): Int = try {
    val phase = managedShutdownMetric(0)
    if (phase == 1uL) {
        val shutdown = OhosFFIContext.withManagedControl(env) {
            OwnedRuntime.progress()
            OwnedRuntime.prepareManagedShutdown()
        }
        if (shutdown) {
            val accepted = OhosFFIContext.requestManagedShutdown(env)
            if (accepted == 0) {
                val retired = OwnedRuntime.retireLifecycle()
                if (retired != 0) { OwnedRuntime.fault(); -1 } else 0
            } else if (accepted == 1) 0 else { OwnedRuntime.fault(); -1 }
        } else 0
    } else if (phase == 3uL || phase == 4uL) 0 else -1
} catch (_: Throwable) { OwnedRuntime.fault(); -1 }

@CName("madb_kn_owned_metric")
fun ownedMetric(index: Int): ULong = try { OwnedRuntime.metric(index) }
catch (_: Throwable) { ULong.MAX_VALUE }
