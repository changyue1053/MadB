use std::task::Waker;

use super::*;
use crate::{LowerReturn, RustBuffer, RustCallError, UniFfiTag};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicUsize, Ordering},
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
struct OwnedObject(Box<u64>);
unsafe impl LowerReturn<UniFfiTag> for OwnedObject {
    type ReturnType = u64;
    fn lower_return(v: Self) -> Result<u64, RustCallError> {
        Ok(Box::into_raw(v.0) as u64)
    }
}
#[test]
fn native_ready_value_is_owned_until_completed_or_freed() {
    for complete in [false, true] {
        let text = "native allocated ready result".to_owned();
        WATCH.store(text.as_ptr() as usize, Ordering::SeqCst);
        RELEASES.store(0, Ordering::SeqCst);
        let mut future = WrappedFuture::<String, UniFfiTag>::new(Box::pin(async move { Ok(text) }));
        assert!(future.poll(&mut Context::from_waker(Waker::noop())));
        if complete {
            let raw: RustBuffer = future.complete(&mut RustCallStatus::default());
            raw.destroy();
        } else {
            future.free();
        }
        let released = RELEASES.load(Ordering::SeqCst);
        WATCH.store(0, Ordering::SeqCst);
        assert_eq!(released, 1, "ready string release; complete={complete}");
        let object = Box::new(55_u64);
        WATCH.store((&*object) as *const u64 as usize, Ordering::SeqCst);
        RELEASES.store(0, Ordering::SeqCst);
        let mut future = WrappedFuture::<OwnedObject, UniFfiTag>::new(Box::pin(async move {
            Ok(OwnedObject(object))
        }));
        assert!(future.poll(&mut Context::from_waker(Waker::noop())));
        if complete {
            let raw = future.complete(&mut RustCallStatus::default());
            unsafe {
                drop(Box::from_raw(raw as *mut u64));
            }
        } else {
            future.free();
        }
        let released = RELEASES.load(Ordering::SeqCst);
        WATCH.store(0, Ordering::SeqCst);
        assert_eq!(released, 1, "ready object release; complete={complete}");
    }
}
#[test]
fn native_poll_panic_error_buffer_is_freed_on_abandon() {
    let mut future = WrappedFuture::<String, UniFfiTag>::new(Box::pin(async {
        panic!("owned panic message");
        #[allow(unreachable_code)]
        Ok(String::new())
    }));
    assert!(future.poll(&mut Context::from_waker(Waker::noop())));
    let status = future.result.as_ref().unwrap().as_ref().err().unwrap();
    WATCH.store(status.error_buf.data_pointer() as usize, Ordering::SeqCst);
    RELEASES.store(0, Ordering::SeqCst);
    drop(future);
    let released = RELEASES.load(Ordering::SeqCst);
    WATCH.store(0, Ordering::SeqCst);
    assert_eq!(released, 1, "abandoned panic status buffer release");
}
