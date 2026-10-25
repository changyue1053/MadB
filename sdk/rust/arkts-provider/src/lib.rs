#![deny(unsafe_op_in_unsafe_fn)]
use std::alloc::{Layout, alloc};
use std::ffi::c_void;
use std::mem::{align_of, offset_of, size_of};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::sync::atomic::{AtomicI32, AtomicU64, Ordering::SeqCst};
const OK: i32 = 0;
const INVALID: i32 = 1;
const ALLOCATION: i32 = 2;
const OVERFLOW: i32 = 3;
const INTERNAL: i32 = 4;
const BUSY: i32 = 5;
#[repr(C)]
pub struct Snapshot {
    value: i32,
    text: *const u8,
    size: usize,
}
#[repr(C)]
pub struct Stats {
    clones: u64,
    allocations: u64,
    releases: u64,
    live: u64,
    work: u64,
}
#[repr(C)]
pub struct Provider {
    context: *mut c_void,
    clone: Option<unsafe extern "C" fn(*mut c_void, Snapshot, *mut *mut c_void) -> i32>,
    read: Option<unsafe extern "C" fn(*mut c_void, *const c_void, *mut Snapshot) -> i32>,
    increment: Option<unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32>,
    release: Option<unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32>,
    stats: Option<unsafe extern "C" fn(*mut c_void, *mut Stats) -> i32>,
    inject: Option<unsafe extern "C" fn(*mut c_void, i32) -> i32>,
    destroy: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
}
#[derive(Default)]
struct Context {
    clones: AtomicU64,
    allocations: AtomicU64,
    releases: AtomicU64,
    live: AtomicU64,
    work: AtomicU64,
    injection: AtomicI32,
    #[cfg(feature = "test-support")]
    memory: MemoryObserver,
}
struct Model {
    value: i32,
    text: String,
}
#[cfg(feature = "test-support")]
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct MemoryObserver {
    context: *mut c_void,
    notify: Option<unsafe extern "C" fn(*mut c_void, u64, u64, u32)>,
}
#[cfg(feature = "test-support")]
fn memory(ctx: &Context, object: usize, text: usize, action: u32) {
    if let Some(notify) = ctx.memory.notify {
        // The already-owned provider lives inside its bound Runtime/Audit.
        unsafe { notify(ctx.memory.context, object as u64, text as u64, action) };
    }
}
#[cfg(feature = "test-support")]
struct CopyAccounting<'a> {
    owner: &'a Context,
    bytes: usize,
    armed: bool,
}
#[cfg(feature = "test-support")]
impl Drop for CopyAccounting<'_> {
    fn drop(&mut self) {
        if self.armed {
            memory(self.owner, 0, self.bytes, 2);
        }
    }
}
fn record_release(ctx: &Context) {
    ctx.releases.fetch_add(1, SeqCst);
    if ctx.live.fetch_sub(1, SeqCst) == 0 {
        std::process::abort();
    }
}
struct OwnedModel<'a> {
    model: Option<Box<Model>>,
    owner: &'a Context,
}
impl OwnedModel<'_> {
    fn into_raw(mut self) -> *mut c_void {
        Box::into_raw(self.model.take().expect("unique model transfer")).cast()
    }
}
impl Drop for OwnedModel<'_> {
    fn drop(&mut self) {
        if let Some(model) = self.model.take() {
            #[cfg(feature = "test-support")]
            let bytes = model.text.capacity();
            drop(model); // Drop String, then free Box storage, then count release.
            #[cfg(feature = "test-support")]
            memory(self.owner, size_of::<Model>(), bytes, 2);
            record_release(self.owner);
        }
    }
}
fn boundary(body: impl FnOnce() -> i32) -> i32 {
    catch_unwind(AssertUnwindSafe(body)).unwrap_or(INTERNAL)
}
// Stable Rust Box::new has a process-abort OOM policy. Allocate its exact layout
// fallibly instead, then construct a genuine uniquely owned Box. This does not
// claim recovery from allocator abort, abort-mode panic, or invalid raw pointers.
fn try_box<T>(value: T) -> Result<Box<T>, i32> {
    let raw = unsafe { alloc(Layout::new::<T>()) }.cast::<T>();
    if raw.is_null() {
        return Err(ALLOCATION);
    }
    // SAFETY: exact non-zero Context/Model allocation, aligned and uniquely owned.
    unsafe {
        raw.write(value);
        Ok(Box::from_raw(raw))
    }
}
unsafe extern "C" fn clone_model(
    opaque: *mut c_void,
    source: Snapshot,
    out: *mut *mut c_void,
) -> i32 {
    if out.is_null() {
        return INVALID;
    }
    unsafe {
        *out = ptr::null_mut();
    }
    if opaque.is_null() || source.size > 1024 || (source.size > 0 && source.text.is_null()) {
        return INVALID;
    }
    boundary(|| {
        // SAFETY: callers uphold context and source span provenance/lifetime.
        let ctx = unsafe { &*opaque.cast::<Context>() };
        let bytes = if source.size == 0 {
            &[][..]
        } else {
            unsafe { std::slice::from_raw_parts(source.text, source.size) }
        };
        let text = match std::str::from_utf8(bytes) {
            Ok(s) => s,
            Err(_) => return INVALID,
        };
        ctx.clones.fetch_add(1, SeqCst);
        let injection = ctx.injection.swap(0, SeqCst);
        match injection {
            1 => return ALLOCATION,
            2 => panic!("controlled Rust clone panic, caught before C ABI return"),
            _ => {}
        }
        // Accounting guard owns no allocation and is declared before the real
        // String, so failure destroys the String before retiring its bytes.
        #[cfg(feature = "test-support")]
        let mut accounting = CopyAccounting {
            owner: ctx,
            bytes: 0,
            armed: true,
        };
        let mut copy = String::new();
        if copy.try_reserve_exact(text.len()).is_err() {
            return ALLOCATION;
        }
        #[cfg(feature = "test-support")]
        {
            accounting.bytes = copy.capacity();
            memory(ctx, 0, accounting.bytes, 1);
        }
        copy.push_str(text);
        let model = match try_box(Model {
            value: source.value,
            text: copy,
        }) {
            Ok(m) => m,
            Err(s) => return s,
        };
        #[cfg(feature = "test-support")]
        memory(ctx, size_of::<Model>(), 0, 1);
        ctx.allocations.fetch_add(1, SeqCst);
        ctx.live.fetch_add(1, SeqCst);
        let owned = OwnedModel {
            model: Some(model),
            owner: ctx,
        };
        #[cfg(feature = "test-support")]
        {
            accounting.armed = false;
        }
        if injection == 4 {
            panic!("controlled Rust panic after Box allocation; Drop must reclaim");
        }
        unsafe {
            *out = owned.into_raw();
        }
        OK
    })
}
unsafe extern "C" fn read_model(
    opaque: *mut c_void,
    ptr: *const c_void,
    out: *mut Snapshot,
) -> i32 {
    if opaque.is_null() || ptr.is_null() || out.is_null() {
        return INVALID;
    }
    boundary(|| {
        let model = unsafe { &*ptr.cast::<Model>() };
        unsafe {
            *out = Snapshot {
                value: model.value,
                text: model.text.as_ptr(),
                size: model.text.len(),
            };
        }
        OK
    })
}
unsafe extern "C" fn increment_model(opaque: *mut c_void, ptr: *mut c_void) -> i32 {
    if opaque.is_null() || ptr.is_null() {
        return INVALID;
    }
    boundary(|| {
        let ctx = unsafe { &*opaque.cast::<Context>() };
        let model = unsafe { &mut *ptr.cast::<Model>() };
        ctx.work.fetch_add(1, SeqCst);
        if ctx.injection.swap(0, SeqCst) == 3 {
            panic!("controlled Rust work panic, caught before C ABI return");
        }
        let Some(value) = model.value.checked_add(1) else {
            return OVERFLOW;
        };
        model.value = value;
        OK
    })
}
unsafe extern "C" fn release_model(opaque: *mut c_void, ptr: *mut c_void) -> i32 {
    if opaque.is_null() || ptr.is_null() {
        return INVALID;
    }
    // SAFETY: this provider created the unique Box and caller consumes it once.
    // Model/String Drop cannot unwind; no foreign cleanup callback is involved.
    let model = unsafe { Box::from_raw(ptr.cast::<Model>()) };
    #[cfg(feature = "test-support")]
    let bytes = model.text.capacity();
    drop(model);
    let ctx = unsafe { &*opaque.cast::<Context>() };
    #[cfg(feature = "test-support")]
    memory(ctx, size_of::<Model>(), bytes, 2);
    record_release(ctx);
    OK
}
unsafe extern "C" fn stats(opaque: *mut c_void, out: *mut Stats) -> i32 {
    if opaque.is_null() || out.is_null() {
        return INVALID;
    }
    let c = unsafe { &*opaque.cast::<Context>() };
    unsafe {
        *out = Stats {
            clones: c.clones.load(SeqCst),
            allocations: c.allocations.load(SeqCst),
            releases: c.releases.load(SeqCst),
            live: c.live.load(SeqCst),
            work: c.work.load(SeqCst),
        };
    }
    OK
}
unsafe extern "C" fn inject(opaque: *mut c_void, mode: i32) -> i32 {
    if opaque.is_null() || !(0..=4).contains(&mode) {
        return INVALID;
    }
    unsafe { &*opaque.cast::<Context>() }
        .injection
        .store(mode, SeqCst);
    OK
}
unsafe extern "C" fn destroy(opaque: *mut c_void) -> i32 {
    if opaque.is_null() {
        return INVALID;
    }
    if unsafe { &*opaque.cast::<Context>() }.live.load(SeqCst) > 0 {
        return BUSY;
    }
    unsafe {
        drop(Box::from_raw(opaque.cast::<Context>()));
    }
    OK
}
/// Create one private provider with a uniquely owned context.
///
/// # Safety
/// Non-null `out` must point to aligned writable Provider storage, exclusively
/// borrowed for this call. The caller must preserve the returned context and DSO
/// until all models and calls drain, then call its destroy function exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mp_rust_create(out: *mut Provider) -> i32 {
    if out.is_null() {
        return INVALID;
    }
    // Null pointer/function pointers are explicitly represented as nullable fields.
    unsafe {
        out.write(Provider {
            context: ptr::null_mut(),
            clone: None,
            read: None,
            increment: None,
            release: None,
            stats: None,
            inject: None,
            destroy: None,
        });
    }
    boundary(|| {
        let ctx = match try_box(Context::default()) {
            Ok(c) => c,
            Err(s) => return s,
        };
        unsafe {
            out.write(Provider {
                context: Box::into_raw(ctx).cast(),
                clone: Some(clone_model),
                read: Some(read_model),
                increment: Some(increment_model),
                release: Some(release_model),
                stats: Some(stats),
                inject: Some(inject),
                destroy: Some(destroy),
            });
        }
        OK
    })
}
#[repr(C)]
pub struct AbiLayout {
    snapshot_size: usize,
    snapshot_align: usize,
    value: usize,
    text: usize,
    length: usize,
    provider_size: usize,
    provider_align: usize,
}
/// Observe this Rust compiler's repr(C) layout for an independent C comparison.
///
/// # Safety
/// Non-null `out` must point to aligned writable AbiLayout storage exclusively
/// borrowed for this call. No ownership transfers through this output.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mp_rust_layout(out: *mut AbiLayout) -> i32 {
    if out.is_null() {
        return INVALID;
    }
    unsafe {
        out.write(AbiLayout {
            snapshot_size: size_of::<Snapshot>(),
            snapshot_align: align_of::<Snapshot>(),
            value: offset_of!(Snapshot, value),
            text: offset_of!(Snapshot, text),
            length: offset_of!(Snapshot, size),
            provider_size: size_of::<Provider>(),
            provider_align: align_of::<Provider>(),
        });
    }
    OK
}

/// Bind a test-only non-owning accounting observer before any models/worker.
/// # Safety
/// Context provenance and lifetime follow mp_rust_create. The observer remains
/// valid until this context and every original model have actually drained.
#[cfg(feature = "test-support")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mp_rust_test_memory_bind(
    opaque: *mut c_void,
    observer: MemoryObserver,
) -> i32 {
    if opaque.is_null() || observer.context.is_null() || observer.notify.is_none() {
        return INVALID;
    }
    let ctx = unsafe { &mut *opaque.cast::<Context>() };
    if let Some(prior) = ctx.memory.notify {
        if ctx.memory.context != observer.context
            || !std::ptr::fn_addr_eq(prior, observer.notify.unwrap())
        {
            return INVALID;
        }
    }
    if ctx.live.load(SeqCst) > 0 {
        return BUSY;
    }
    ctx.memory = observer;
    memory(ctx, size_of::<Model>(), 0, 0);
    OK
}
