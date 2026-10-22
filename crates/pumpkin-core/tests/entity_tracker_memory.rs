//! Guards the memory footprint of the per-entity tracking data.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::hint::black_box;

use pumpkin_core::world::entity_tracker::SeenBy;
use uuid::Uuid;

/// Tracks the bytes that this thread has allocated.
struct CountingAlloc;

thread_local! {
    static ALLOCATED_BYTES: Cell<usize> = const { Cell::new(0) };
}

// SAFETY: every call is forwarded to `System` with the arguments it was
// given, so the `GlobalAlloc` contract is upheld. The counter only observes
// sizes and never changes what an allocation does.
unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATED_BYTES.with(|allocated| allocated.set(allocated.get() + layout.size()));
        // SAFETY: the layout is passed on unchanged.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: the pointer and layout were produced by `System`.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCATED_BYTES.with(|allocated| allocated.set(allocated.get() + new_size));
        // SAFETY: the pointer and layout were produced by `System`.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAlloc = CountingAlloc;

/// A `DashSet` used to allocate a shard array of `shard_count * 128` bytes
/// for every tracked entity, which is 4 KiB on an 8-core machine.
const BUDGET_PER_ENTITY: usize = 512;

#[test]
fn seen_by_does_not_allocate_a_shard_array_per_entity() {
    const ENTITIES: usize = 1_000;

    let mut trackers = Vec::with_capacity(ENTITIES);
    let before = ALLOCATED_BYTES.with(Cell::get);
    for _ in 0..ENTITIES {
        trackers.push(black_box(SeenBy::default()));
    }
    let after = ALLOCATED_BYTES.with(Cell::get);

    let per_entity = (after - before) / ENTITIES;
    assert!(
        per_entity < BUDGET_PER_ENTITY,
        "SeenBy allocated {per_entity} bytes per entity"
    );

    black_box(trackers);
}

#[test]
fn seen_by_tracks_which_players_can_see_an_entity() {
    let seen_by = SeenBy::default();
    let first = Uuid::from_u128(1);
    let second = Uuid::from_u128(2);

    assert!(seen_by.is_empty());

    assert!(seen_by.insert(first));
    assert!(!seen_by.insert(first));
    assert!(seen_by.contains(&first));
    assert!(!seen_by.contains(&second));
    assert!(!seen_by.is_empty());

    assert!(seen_by.remove(&first));
    assert!(!seen_by.remove(&first));
    assert!(seen_by.is_empty());

    seen_by.insert(first);
    seen_by.insert(second);
    seen_by.clear();
    assert!(seen_by.is_empty());
}
