// Injected as a test module in an isolated copy of the pinned UniFFI core crate.
use super::*;
use crate::{test_util::TestError, Lower, RustBuffer, UniFfiTag};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    future::Future,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    task::{Context, Poll, Waker},
};

static WATCH: AtomicUsize = AtomicUsize::new(0);
static RELEASES: AtomicUsize = AtomicUsize::new(0);
struct TrackingAllocator;
unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        System.alloc(layout)
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if ptr as usize == WATCH.load(Ordering::SeqCst) {
            RELEASES.fetch_add(1, Ordering::SeqCst);
        }
        System.dealloc(ptr, layout)
    }
}
#[global_allocator]
static ALLOCATOR: TrackingAllocator = TrackingAllocator;

fn verify<T: LiftReturn<UniFfiTag>>(
    raw: ForeignFutureResult<T::ReturnType>,
    pointer: usize,
    order: u8,
) {
    RELEASES.store(0, Ordering::SeqCst);
    WATCH.store(pointer, Ordering::SeqCst);
    let info = Arc::new(Mutex::new(None));
    let capture = info.clone();
    extern "C" fn free(_: u64) {}
    let mut future = Box::pin(foreign_async_call::<_, T, UniFfiTag>(
        move |callback, handle, dropped| {
            *capture.lock().unwrap() = Some((callback, handle));
            *dropped = ForeignFutureDroppedCallbackStruct {
                callback_data: 1,
                callback: free,
            };
        },
    ));
    let mut context = Context::from_waker(Waker::noop());
    assert!(future.as_mut().poll(&mut context).is_pending());
    let (callback, handle) = info.lock().unwrap().take().unwrap();
    if order == 0 {
        drop(future);
        callback(handle, raw);
    } else {
        callback(handle, raw);
        if order == 2 {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(value) => drop(value),
                Poll::Pending => panic!("completion was lost"),
            }
        }
        drop(future);
    }
    let releases = RELEASES.load(Ordering::SeqCst);
    WATCH.store(0, Ordering::SeqCst);
    assert_eq!(
        releases, 1,
        "allocated foreign result must be freed exactly once; order={order}"
    );
}

struct OwnedObject {
    _value: Box<u64>,
}
unsafe impl LiftReturn<UniFfiTag> for OwnedObject {
    type ReturnType = u64;
    fn try_lift_successful_return(raw: u64) -> crate::Result<Self> {
        Ok(Self {
            _value: unsafe { Box::from_raw(raw as *mut u64) },
        })
    }
}

#[test]
fn allocated_late_and_unpolled_foreign_results_are_released_once() {
    for order in 0..=2 {
        let raw = <String as Lower<UniFfiTag>>::lower("actual allocated string result".into());
        let pointer = raw.data_pointer() as usize;
        verify::<String>(
            ForeignFutureResult {
                return_value: raw,
                call_status: RustCallStatus::default(),
            },
            pointer,
            order,
        );
        let raw = <Vec<String> as Lower<UniFfiTag>>::lower(vec![
            "nested snapshot field".into(),
            "second field".into(),
        ]);
        let pointer = raw.data_pointer() as usize;
        verify::<Vec<String>>(
            ForeignFutureResult {
                return_value: raw,
                call_status: RustCallStatus::default(),
            },
            pointer,
            order,
        );
        let raw = Box::into_raw(Box::new(41_u64)) as u64;
        verify::<OwnedObject>(
            ForeignFutureResult {
                return_value: raw,
                call_status: RustCallStatus::default(),
            },
            raw as usize,
            order,
        );
        let err = <TestError as crate::LowerError<UniFfiTag>>::lower_error(TestError(
            "declared error".into(),
        ));
        let pointer = err.data_pointer() as usize;
        verify::<Result<String, TestError>>(
            ForeignFutureResult {
                return_value: RustBuffer::new(),
                call_status: RustCallStatus {
                    code: crate::RustCallStatusCode::Error,
                    error_buf: std::mem::ManuallyDrop::new(err),
                },
            },
            pointer,
            order,
        );
    }
}
