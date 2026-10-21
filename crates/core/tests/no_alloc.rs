//! core-types §3.4: `value_at`, `at_clock`, `gate_at` and `events()` make no
//! allocations. This binary installs a counting global allocator.

mod common;

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::hint::black_box;

use common::{any_track, std_rate};
use proptest::prelude::*;
use proptest::test_runner::TestRunner;
use takedown_core::time::{ControlClock, TimelinePos, rescale_ceil};

/// Counts alloc, dealloc and realloc calls on the current thread.
struct CountingAlloc;

thread_local! {
    // Const-initialised and without Drop, so access never allocates.
    static CALLS: Cell<u64> = const { Cell::new(0) };
}

fn bump() {
    let _ = CALLS.try_with(|c| c.set(c.get() + 1));
}

fn calls() -> u64 {
    CALLS.with(Cell::get)
}

// GlobalAlloc is an unsafe trait. This impl only counts calls on the current
// thread and forwards every request unchanged to the system allocator, so it
// upholds exactly the contract System does.
#[allow(unsafe_code)]
unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        bump();
        // SAFETY: forwarded unchanged; the caller upholds alloc's contract.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        bump();
        // SAFETY: forwarded unchanged; the caller upholds alloc_zeroed's contract.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        bump();
        // SAFETY: ptr came from System through this allocator with this layout.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        bump();
        // SAFETY: ptr came from System through this allocator with this layout.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: CountingAlloc = CountingAlloc;

#[test]
fn views_and_events_do_not_allocate() {
    // The counter checks itself.
    let before = calls();
    drop(black_box(vec![1u8; 64]));
    assert!(
        calls() >= before + 2,
        "the counting allocator is not counting"
    );

    let mut runner = TestRunner::default();
    runner
        .run(
            &(any_track(), std_rate(), any::<u64>()),
            |(t, out, start)| {
                let end = rescale_ceil(t.len(), t.rate(), out).0;
                let start = start % (end + 1_000);
                let mut clock = ControlClock::at(out, start);
                let mut acc = 0u32;
                let before = calls();
                for i in 0..2_048 {
                    let a = t.value_at(out, start + i);
                    let b = t.at_clock(&clock);
                    clock.advance();
                    acc ^= a.pitch_st.to_bits() ^ b.vowel[1].to_bits();
                    acc ^= t
                        .gate_at(TimelinePos(start / 3 + i))
                        .map_or(0, |s| s as u32);
                }
                for e in t.events() {
                    acc ^= e.at.0 as u32 ^ e.anchor.0 as u32;
                }
                let n = calls() - before;
                black_box(acc);
                prop_assert_eq!(n, 0, "allocations on the evaluation path");
                Ok(())
            },
        )
        .unwrap();
}
