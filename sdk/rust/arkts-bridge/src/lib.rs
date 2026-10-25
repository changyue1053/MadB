#![deny(unsafe_op_in_unsafe_fn)]
use napi_derive_ohos::napi;
use napi_ohos::{Env, JsValue, Unknown};
use napi_sys_ohos::{napi_env, napi_value};

// Genuine Rust Box/String model, allocator and checked increment implementation.
#[path = "../../arkts-provider/src/lib.rs"]
mod provider;

unsafe extern "C" {
    fn madb_c07_native_call(
        env: napi_env,
        method: u32,
        argc: usize,
        argv: *const napi_value,
    ) -> napi_value;
}

// Every value is borrowed from this current generated napi-ohos callback.
// Never pre-convert String/int32: strict original UTF-16/numeric checks belong
// to the preserved codec. No UnknownRef, JsDeferred, async/Tokio adapter or
// foreign callback_info is constructed. Raw returns preserve pending errors.
fn call(env: Env, method: u32, args: &[napi_value]) -> napi_value {
    // SAFETY: only generated synchronous exports below construct these bounded
    // arrays from their own same-env Unknown arguments; native never retains
    // these borrowed arrays and catches every C++ exception at its C seam.
    unsafe { madb_c07_native_call(env.raw(), method, args.len(), args.as_ptr()) }
}

#[napi(js_name = "runtimeOpen", ts_return_type = "unknown", catch_unwind)]
pub fn runtime_open(
    env: Env,
    expected_identity: Unknown<'_>,
    dispatcher: Unknown<'_>,
) -> napi_value {
    call(env, 0, &[expected_identity.raw(), dispatcher.raw()])
}

#[napi(js_name = "connect", ts_return_type = "unknown", catch_unwind)]
pub fn connect(env: Env, runtime: Unknown<'_>) -> napi_value {
    call(env, 1, &[runtime.raw()])
}

#[napi(js_name = "roundtrip", ts_return_type = "unknown", catch_unwind)]
pub fn roundtrip(
    env: Env,
    engine: Unknown<'_>,
    value: Unknown<'_>,
    text: Unknown<'_>,
) -> napi_value {
    call(env, 2, &[engine.raw(), value.raw(), text.raw()])
}

#[napi(js_name = "submit", ts_return_type = "unknown", catch_unwind)]
pub fn submit(env: Env, engine: Unknown<'_>, value: Unknown<'_>, text: Unknown<'_>) -> napi_value {
    call(env, 3, &[engine.raw(), value.raw(), text.raw()])
}

#[napi(js_name = "cancel", ts_return_type = "unknown", catch_unwind)]
pub fn cancel(env: Env, operation: Unknown<'_>) -> napi_value {
    call(env, 4, &[operation.raw()])
}

#[napi(js_name = "observe", ts_return_type = "unknown", catch_unwind)]
pub fn observe(env: Env, engine: Unknown<'_>, stream: Unknown<'_>) -> napi_value {
    call(env, 5, &[engine.raw(), stream.raw()])
}

#[napi(js_name = "publish", ts_return_type = "unknown", catch_unwind)]
pub fn publish(
    env: Env,
    engine: Unknown<'_>,
    stream: Unknown<'_>,
    value: Unknown<'_>,
    text: Unknown<'_>,
) -> napi_value {
    call(
        env,
        6,
        &[engine.raw(), stream.raw(), value.raw(), text.raw()],
    )
}

#[napi(js_name = "requestClose", ts_return_type = "unknown", catch_unwind)]
pub fn request_close(env: Env, target: Unknown<'_>) -> napi_value {
    call(env, 7, &[target.raw()])
}

#[napi(js_name = "claimResult", ts_return_type = "unknown", catch_unwind)]
pub fn claim_result(env: Env, operation: Unknown<'_>, observation_id: Unknown<'_>) -> napi_value {
    call(env, 8, &[operation.raw(), observation_id.raw()])
}

#[napi(js_name = "watchClose", ts_return_type = "unknown", catch_unwind)]
pub fn watch_close(env: Env, target: Unknown<'_>, observation_id: Unknown<'_>) -> napi_value {
    call(env, 9, &[target.raw(), observation_id.raw()])
}

#[napi(js_name = "tryTake", ts_return_type = "unknown", catch_unwind)]
pub fn try_take(env: Env, runtime: Unknown<'_>, observation_id: Unknown<'_>) -> napi_value {
    call(env, 10, &[runtime.raw(), observation_id.raw()])
}

#[napi(js_name = "deliver", ts_return_type = "unknown", catch_unwind)]
pub fn deliver(env: Env, subscription: Unknown<'_>) -> napi_value {
    call(env, 11, &[subscription.raw()])
}

#[napi(
    js_name = "requestRuntimeClose",
    ts_return_type = "unknown",
    catch_unwind
)]
pub fn request_runtime_close(env: Env, runtime: Unknown<'_>) -> napi_value {
    call(env, 12, &[runtime.raw()])
}

#[napi(js_name = "pollRuntime", ts_return_type = "unknown", catch_unwind)]
pub fn poll_runtime(env: Env, runtime: Unknown<'_>) -> napi_value {
    call(env, 13, &[runtime.raw()])
}

#[napi(
    js_name = "consumeRuntimeReceipt",
    ts_return_type = "unknown",
    catch_unwind
)]
pub fn consume_runtime_receipt(env: Env, runtime: Unknown<'_>) -> napi_value {
    call(env, 14, &[runtime.raw()])
}

#[napi(js_name = "audit", ts_return_type = "unknown", catch_unwind)]
pub fn audit(env: Env, runtime: Unknown<'_>) -> napi_value {
    call(env, 15, &[runtime.raw()])
}

#[cfg(feature = "test-support")]
mod test_exports {
    use super::*;

    #[napi(js_name = "testArmWork", ts_return_type = "unknown", catch_unwind)]
    pub fn test_arm_work(env: Env, engine: Unknown<'_>, phase: Unknown<'_>) -> napi_value {
        call(env, 16, &[engine.raw(), phase.raw()])
    }

    #[napi(js_name = "testReleaseWork", ts_return_type = "unknown", catch_unwind)]
    pub fn test_release_work(env: Env, engine: Unknown<'_>) -> napi_value {
        call(env, 17, &[engine.raw()])
    }

    #[napi(
        js_name = "testHoldPublication",
        ts_return_type = "unknown",
        catch_unwind
    )]
    pub fn test_hold_publication(
        env: Env,
        engine: Unknown<'_>,
        stream: Unknown<'_>,
        value: Unknown<'_>,
        text: Unknown<'_>,
    ) -> napi_value {
        call(
            env,
            18,
            &[engine.raw(), stream.raw(), value.raw(), text.raw()],
        )
    }

    #[napi(
        js_name = "testReleasePublication",
        ts_return_type = "unknown",
        catch_unwind
    )]
    pub fn test_release_publication(
        env: Env,
        engine: Unknown<'_>,
        commit: Unknown<'_>,
    ) -> napi_value {
        call(env, 19, &[engine.raw(), commit.raw()])
    }

    #[napi(js_name = "testFailClone", ts_return_type = "unknown", catch_unwind)]
    pub fn test_fail_clone(
        env: Env,
        engine: Unknown<'_>,
        nth: Unknown<'_>,
        mode: Unknown<'_>,
    ) -> napi_value {
        call(env, 20, &[engine.raw(), nth.raw(), mode.raw()])
    }

    #[napi(
        js_name = "testFailCompletion",
        ts_return_type = "unknown",
        catch_unwind
    )]
    pub fn test_fail_completion(env: Env, engine: Unknown<'_>, stage: Unknown<'_>) -> napi_value {
        call(env, 21, &[engine.raw(), stage.raw()])
    }

    #[napi(js_name = "testObserve", ts_return_type = "unknown", catch_unwind)]
    pub fn test_observe(env: Env, runtime: Unknown<'_>) -> napi_value {
        call(env, 22, &[runtime.raw()])
    }

    #[napi(js_name = "testExportKey", ts_return_type = "unknown", catch_unwind)]
    pub fn test_export_key(env: Env, runtime: Unknown<'_>) -> napi_value {
        call(env, 23, &[runtime.raw()])
    }

    #[napi(js_name = "testInspect", ts_return_type = "unknown", catch_unwind)]
    pub fn test_inspect(env: Env, key: Unknown<'_>) -> napi_value {
        call(env, 24, &[key.raw()])
    }

    #[napi(
        js_name = "testObserveOperation",
        ts_return_type = "unknown",
        catch_unwind
    )]
    pub fn test_observe_operation(env: Env, operation: Unknown<'_>) -> napi_value {
        call(env, 25, &[operation.raw()])
    }

    #[napi(
        js_name = "testArmPartialOpenFailure",
        ts_return_type = "unknown",
        catch_unwind
    )]
    pub fn test_arm_partial_open_failure(env: Env) -> napi_value {
        call(env, 26, &[])
    }

    #[napi(
        js_name = "testLastAdmissionAudit",
        ts_return_type = "unknown",
        catch_unwind
    )]
    pub fn test_last_admission_audit(env: Env) -> napi_value {
        call(env, 27, &[])
    }

    #[napi(js_name = "testDeliveryGate", ts_return_type = "unknown", catch_unwind)]
    pub fn test_delivery_gate(
        env: Env,
        subscription: Unknown<'_>,
        command: Unknown<'_>,
    ) -> napi_value {
        call(env, 28, &[subscription.raw(), command.raw()])
    }

    #[napi(
        js_name = "testHandleAdmission",
        ts_return_type = "unknown",
        catch_unwind
    )]
    pub fn test_handle_admission(
        env: Env,
        engine: Unknown<'_>,
        scenario: Unknown<'_>,
        foreign_observation_key: Unknown<'_>,
    ) -> napi_value {
        call(
            env,
            29,
            &[engine.raw(), scenario.raw(), foreign_observation_key.raw()],
        )
    }

    #[napi(
        js_name = "testBootstrapTotals",
        ts_return_type = "unknown",
        catch_unwind
    )]
    pub fn test_bootstrap_totals(env: Env) -> napi_value {
        call(env, 30, &[])
    }
    #[napi(js_name = "testMemory", ts_return_type = "unknown", catch_unwind)]
    pub fn test_memory(env: Env, runtime: Unknown<'_>) -> napi_value {
        call(env, 31, &[runtime.raw()])
    }
}
