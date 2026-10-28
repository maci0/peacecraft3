//! Counting allocator: forwards to the system allocator and counts calls inside the window.
use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

static MEASURING: AtomicBool = AtomicBool::new(false);
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static REALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static BYTES: AtomicUsize = AtomicUsize::new(0);

pub struct Counts {
    pub allocations: usize,
    pub reallocations: usize,
    pub bytes: usize,
}

pub fn start() {
    ALLOCATIONS.store(0, Ordering::Relaxed);
    REALLOCATIONS.store(0, Ordering::Relaxed);
    BYTES.store(0, Ordering::Relaxed);
    MEASURING.store(true, Ordering::Relaxed);
}

pub fn stop() -> Counts {
    MEASURING.store(false, Ordering::Relaxed);
    Counts {
        allocations: ALLOCATIONS.load(Ordering::Relaxed),
        reallocations: REALLOCATIONS.load(Ordering::Relaxed),
        bytes: BYTES.load(Ordering::Relaxed),
    }
}

fn record(bytes: usize, realloc: bool) {
    if MEASURING.load(Ordering::Relaxed) {
        let counter = if realloc {
            &REALLOCATIONS
        } else {
            &ALLOCATIONS
        };
        counter.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(bytes, Ordering::Relaxed);
    }
}

struct Counting;

// GlobalAlloc is required to count real allocation calls in the deterministic gate.
// SAFETY: System receives unchanged layouts and pointers from GlobalAlloc callers;
// the counters are atomics and cannot allocate, unwind or change returned pointers.
#[allow(unsafe_code)]
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record(layout.size(), false);
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record(layout.size(), false);
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, old: Layout, size: usize) -> *mut u8 {
        record(size, true);
        unsafe { System.realloc(ptr, old, size) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;
