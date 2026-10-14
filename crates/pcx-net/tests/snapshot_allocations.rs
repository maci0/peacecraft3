use pcx_net::{Client, CodecError, decode_world_payload, encode_world_payload};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

struct Meter;
static ACTIVE: AtomicBool = AtomicBool::new(false);
static EVENTS: AtomicUsize = AtomicUsize::new(0);
static BYTES: AtomicUsize = AtomicUsize::new(0);
fn record(size: usize, pointer: *mut u8) {
    if ACTIVE.load(Ordering::Relaxed) && !pointer.is_null() {
        EVENTS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(size, Ordering::Relaxed);
    }
}
// SAFETY: Every operation forwards the exact pointer/layout contract to System.
// Counters use only atomics, with no allocation, logging, pointer access or unwind.
#[allow(unsafe_code)]
unsafe impl GlobalAlloc for Meter {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: The caller supplies a valid allocation layout.
        let pointer = unsafe { System.alloc(layout) };
        record(layout.size(), pointer);
        pointer
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: The caller supplies a valid allocation layout.
        let pointer = unsafe { System.alloc_zeroed(layout) };
        record(layout.size(), pointer);
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: GlobalAlloc callers supply the live allocation and its layout.
        unsafe { System.dealloc(pointer, layout) };
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        // SAFETY: The caller satisfies System's live-pointer/layout/size contract.
        let output = unsafe { System.realloc(pointer, layout, size) };
        record(size, output);
        output
    }
}
#[global_allocator]
static ALLOCATOR: Meter = Meter;
const CASES: u32 = 9;
type Record = (u32, i32, Option<(i32, i32)>);
fn payload(count: u32, records: &[Record]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&u32::MAX.to_le_bytes());
    bytes.extend_from_slice(&count.to_le_bytes());
    for &(id, x, target) in records {
        bytes.extend_from_slice(&id.to_le_bytes());
        bytes.push(3);
        bytes.extend_from_slice(&x.to_le_bytes());
        bytes.extend_from_slice(&0i32.to_le_bytes());
        bytes.push(u8::from(target.is_some()));
        if let Some((x, y)) = target {
            bytes.extend_from_slice(&x.to_le_bytes());
            bytes.extend_from_slice(&y.to_le_bytes());
        }
    }
    bytes
}
fn case(index: u32) -> u32 {
    let records = [(9, 64, None), (3, -11, None), (0, 7, Some((13, -17)))];
    let bytes = match index {
        0 => payload(u32::MAX, &[]),
        1 => payload(u32::MAX, &records[..2]),
        2 => {
            let mut b = payload(3, &records);
            b.pop();
            b
        }
        3 => {
            let mut b = payload(2, &records[..2]);
            b.push(255);
            b
        }
        4 => payload(3, &records),
        5 => payload(0, &records[..1]),
        6 => payload(0, &[]),
        7 => {
            let mut b = payload(1, &records[2..]);
            b[21] = 128;
            b
        }
        8 => vec![0, 0, 0],
        _ => return 128,
    };
    let initial = decode_world_payload(0, &payload(1, &[(9, 0, None)])).unwrap();
    let mut client = Client::from_world(initial.clone(), 19);
    let history = (
        client.history().len(),
        client.history().oldest(),
        client.history().newest(),
    );
    EVENTS.store(0, Ordering::Relaxed);
    BYTES.store(0, Ordering::Relaxed);
    ACTIVE.store(true, Ordering::Relaxed);
    let result = decode_world_payload(1, &bytes);
    ACTIVE.store(false, Ordering::Relaxed);
    let invalid = !matches!(index, 4 | 6 | 7);
    let mut failure = u32::from(invalid && EVENTS.load(Ordering::Relaxed) != 0);
    let correct = match &result {
        Err(CodecError::PayloadTooShort { kind: 3, need, got }) => {
            let wanted = match index {
                0 => 22,
                1 => 50,
                2 => 58,
                8 => 4,
                _ => 0,
            };
            *need == wanted && *got == bytes.len()
        }
        Err(CodecError::TrailingBytes { remaining }) => {
            (index == 3 && *remaining == 1) || (index == 5 && *remaining == 14)
        }
        Ok(world) if index == 4 => {
            world.units.len() == 3
                && world.next_unit_id == u32::MAX
                && encode_world_payload(world) == bytes
        }
        Ok(world) if index == 6 => world.units.is_empty() && world.next_unit_id == u32::MAX,
        Ok(world) if index == 7 => {
            let mut canonical = bytes.clone();
            canonical[21] = 1;
            world.units.len() == 1
                && world.units[0].move_target == Some((13, -17))
                && encode_world_payload(world) == canonical
        }
        _ => false,
    };
    failure |= u32::from(!correct) << 1;
    if let Ok(world) = result {
        if index == 4 {
            let report = client.reconcile_detailed(world);
            failure |= u32::from(
                report.divergence != 64
                    || client.predicted() != &report.snapshot
                    || encode_world_payload(client.predicted()) != bytes,
            ) << 2;
        } else if invalid {
            client.reconcile_detailed(world);
        }
    }
    if invalid {
        let untouched = client.predicted() == &initial
            && client.authoritative() == &initial
            && client.consensus_tick() == 0
            && client.pending_inputs() == 0
            && (
                client.history().len(),
                client.history().oldest(),
                client.history().newest(),
            ) == history;
        failure |= u32::from(!untouched) << 2;
    }
    failure
}
#[test]
fn malformed_frames_allocate_no_unit_metadata_and_valid_delivery_survives() {
    let mut failures = Vec::new();
    for index in 0..CASES {
        let result = case(index);
        println!(
            "case={index} status={result} allocation_events={} requested_bytes={} unit_size={} pointer_bits={}",
            EVENTS.load(Ordering::Relaxed),
            BYTES.load(Ordering::Relaxed),
            std::mem::size_of::<pcx_sim::Unit>(),
            usize::BITS
        );
        if result != 0 {
            failures.push((index, result));
        }
    }
    assert!(failures.is_empty(), "case failures: {failures:?}");
}
