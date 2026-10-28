//! The test binary's single counting global allocator. Counters are process-lifetime
//! monotonic atomics (all threads); callers subtract two [`totals`] snapshots.
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    sync::atomic::{AtomicU64, Ordering},
};

struct Counting;
static CALLS: AtomicU64 = AtomicU64::new(0);
static REALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);
thread_local! {
    // Const thread-locals have no destructor and never allocate, so the allocator may touch them.
    static THREAD: Cell<Totals> = const { Cell::new(Totals { calls: 0, reallocations: 0, bytes: 0 }) };
}

/// Allocation calls, reallocation calls and requested bytes (new size for a reallocation).
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub(crate) struct Totals {
    pub(crate) calls: u64,
    pub(crate) reallocations: u64,
    pub(crate) bytes: u64,
}

impl std::ops::Sub for Totals {
    type Output = Self;
    fn sub(self, earlier: Self) -> Self {
        Self {
            calls: self.calls - earlier.calls,
            reallocations: self.reallocations - earlier.reallocations,
            bytes: self.bytes - earlier.bytes,
        }
    }
}

pub(crate) fn totals() -> Totals {
    Totals {
        calls: CALLS.load(Ordering::Relaxed),
        reallocations: REALLOCATIONS.load(Ordering::Relaxed),
        bytes: BYTES.load(Ordering::Relaxed),
    }
}

/// The calling thread's own counters, for attributing allocations to a span on that thread.
pub(crate) fn thread_totals() -> Totals {
    THREAD.with(Cell::get)
}

fn record(calls: u64, reallocations: u64, bytes: u64) {
    CALLS.fetch_add(calls, Ordering::Relaxed);
    REALLOCATIONS.fetch_add(reallocations, Ordering::Relaxed);
    BYTES.fetch_add(bytes, Ordering::Relaxed);
    // try_with: the slot is gone only during thread teardown.
    let _ = THREAD.try_with(|slot| {
        let t = slot.get();
        slot.set(Totals {
            calls: t.calls + calls,
            reallocations: t.reallocations + reallocations,
            bytes: t.bytes + bytes,
        });
    });
}

// GlobalAlloc is required to count actual allocations in the test binary.
// SAFETY: System receives unchanged layouts and valid pointers from GlobalAlloc callers;
// counters use only atomics and cannot allocate, unwind or affect returned pointers.
#[allow(unsafe_code)]
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record(1, 0, layout.size() as u64);
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record(1, 0, layout.size() as u64);
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, old: Layout, size: usize) -> *mut u8 {
        record(0, 1, size as u64);
        unsafe { System.realloc(ptr, old, size) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;
