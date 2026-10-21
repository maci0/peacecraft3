use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    hint::black_box,
};

struct Counting;
thread_local! { static ALLOCATIONS: Cell<Option<usize>> = const { Cell::new(None) }; }
fn count() {
    let _ = ALLOCATIONS.try_with(|n| {
        if let Some(value) = n.get() {
            n.set(Some(value.saturating_add(1)));
        }
    });
}
// SAFETY: layouts and valid allocator pointers pass unchanged to System;
// the const thread-local counter neither allocates nor unwinds.
#[allow(unsafe_code)]
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count();
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        count();
        unsafe { System.realloc(ptr, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: Counting = Counting;

#[test]
fn normalized_paths_allocate_once_and_keep_unicode_and_validation() {
    ALLOCATIONS.set(Some(0));
    for _ in 0..1000 {
        black_box(pcx_assets::AssetPath::new(black_box("Textures\\Ä\\SOME.BLP")).unwrap());
    }
    let allocations = ALLOCATIONS.replace(None).unwrap();
    eprintln!("asset-path allocations={allocations}");
    assert_eq!(allocations, 1000, "one owned normalized path per request");
    assert_eq!(
        pcx_assets::AssetPath::new("Textures\\Ä\\SOME.BLP")
            .unwrap()
            .as_str(),
        "textures/Ä/some.blp"
    );
}
