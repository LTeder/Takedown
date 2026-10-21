//! core-types §3.6: `render_offline_matches_and_cancels`.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use proptest::prelude::*;
use takedown_core::time::RenderPos;
use takedown_render::contract::testkit::{first_bit_difference, render_case, render_cases};
use takedown_render::contract::{Cancelled, Driver, RenderBackend, render_offline};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(render_cases()))]

    /// Equals the testkit loop; reports progress; cancels; works through
    /// `dyn RenderBackend`.
    #[test]
    fn render_offline_matches_and_cancels((case, pick) in (render_case(), any::<usize>())) {
        for v in case.bind() {
            let t = &v.bound;
            let from = pick % t.segments().len();
            let start = t.segments()[from].start.0;
            let end = RenderPos(v.render_len());
            let expected = v.from_segment(from, end.0, &[v.max_block]);

            let mut b: Box<dyn RenderBackend> = Box::new(v.backend());
            let mut d = Driver::new(&b.info(), v.prepared, v.max_block);
            let cancel = AtomicBool::new(false);
            let progress = AtomicU64::new(u64::MAX);
            let mut got = Vec::new();
            let mut next = start;
            let mut contiguous = true;
            let mut longest = 0;
            let done = render_offline(
                &mut *b, t, &mut d, from, end,
                &mut |pos, block| {
                    contiguous &= pos.0 == next;
                    next = pos.0 + block.len() as u64;
                    longest = longest.max(block.len());
                    got.extend_from_slice(block);
                },
                &cancel, &progress,
            );
            prop_assert_eq!(done, Ok(()));
            prop_assert!(contiguous, "blocks are not contiguous");
            prop_assert!(longest <= v.max_block);
            prop_assert_eq!(first_bit_difference(&got, &expected), None);
            prop_assert_eq!(progress.load(Ordering::Relaxed), end.0);

            // Cancelled from inside the first block: no second block. A render
            // that fits in one block has nothing left to cancel.
            let one_block = end.0 - start <= v.max_block as u64;
            let mut calls = 0;
            let first = AtomicU64::new(0);
            let done = render_offline(
                &mut *b, t, &mut d, from, end,
                &mut |pos, block| {
                    calls += 1;
                    first.store(pos.0 + block.len() as u64, Ordering::Relaxed);
                    cancel.store(true, Ordering::Relaxed);
                },
                &cancel, &progress,
            );
            prop_assert_eq!(done, if one_block { Ok(()) } else { Err(Cancelled) });
            prop_assert_eq!(calls, 1);
            prop_assert_eq!(progress.load(Ordering::Relaxed), first.load(Ordering::Relaxed));

            // Already cancelled: nothing is rendered.
            let mut calls = 0;
            let done = render_offline(&mut *b, t, &mut d, from, end, &mut |_, _| calls += 1, &cancel, &progress);
            prop_assert_eq!(done, Err(Cancelled));
            prop_assert_eq!(calls, 0);
        }
    }
}
