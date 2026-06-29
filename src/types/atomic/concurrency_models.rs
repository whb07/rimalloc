use crate::core_types::{ThreadFreeOwner, ThreadFreeWord};
use loom::sync::Arc;
use loom::sync::atomic::{AtomicUsize as LoomAtomicUsize, Ordering as LoomOrdering};
use loom::thread;

const HEAD_A: usize = 0x10;
const HEAD_B: usize = 0x20;

fn head_bits(raw: usize) -> usize {
    raw & !ThreadFreeWord::OWNED_BIT
}

fn push_remote_word(cell: &LoomAtomicUsize, next: &LoomAtomicUsize, head: usize) {
    let mut old = ThreadFreeWord(cell.load(LoomOrdering::Acquire));
    loop {
        next.store(head_bits(old.raw()), LoomOrdering::Relaxed);
        let new = ThreadFreeWord(head | ThreadFreeWord::OWNED_BIT);
        match cell.compare_exchange_weak(
            old.raw(),
            new.raw(),
            LoomOrdering::AcqRel,
            LoomOrdering::Acquire,
        ) {
            Ok(_) => return,
            Err(actual) => old = ThreadFreeWord(actual),
        }
    }
}

fn collect_word(cell: &LoomAtomicUsize) -> usize {
    let mut old = ThreadFreeWord(cell.load(LoomOrdering::Relaxed));
    loop {
        let head = head_bits(old.raw());
        if head == 0 {
            return 0;
        }
        let new = old.without_head();
        match cell.compare_exchange_weak(
            old.raw(),
            new.raw(),
            LoomOrdering::AcqRel,
            LoomOrdering::Acquire,
        ) {
            Ok(_) => return head,
            Err(actual) => old = ThreadFreeWord(actual),
        }
    }
}

fn claim_word(cell: &LoomAtomicUsize) -> bool {
    (cell.fetch_or(ThreadFreeWord::OWNED_BIT, LoomOrdering::AcqRel) & ThreadFreeWord::OWNED_BIT)
        == 0
}

fn release_ownership_word(cell: &LoomAtomicUsize) -> bool {
    loop {
        let current = cell.load(LoomOrdering::Acquire);
        let current_word = ThreadFreeWord(current);
        let new = current_word.with_owner(ThreadFreeOwner::Unowned).raw();
        if cell
            .compare_exchange_weak(current, new, LoomOrdering::AcqRel, LoomOrdering::Acquire)
            .is_ok()
        {
            return current_word.is_owned();
        }
    }
}

fn next_for(head: usize, next_a: &LoomAtomicUsize, next_b: &LoomAtomicUsize) -> usize {
    match head {
        HEAD_A => next_a.load(LoomOrdering::Acquire),
        HEAD_B => next_b.load(LoomOrdering::Acquire),
        _ => panic!("unexpected modeled head value: {head:#x}"),
    }
}

fn mark_reachable(
    mut head: usize,
    next_a: &LoomAtomicUsize,
    next_b: &LoomAtomicUsize,
    seen: &mut [bool; 2],
) {
    let mut steps = 0;
    while head != 0 {
        steps += 1;
        assert!(steps <= 2, "modeled free list contains a cycle");
        match head {
            HEAD_A => {
                assert!(!seen[0], "HEAD_A appeared more than once");
                seen[0] = true;
            }
            HEAD_B => {
                assert!(!seen[1], "HEAD_B appeared more than once");
                seen[1] = true;
            }
            _ => panic!("unexpected modeled head value: {head:#x}"),
        }
        head = next_for(head, next_a, next_b);
    }
}

#[test]
fn concurrent_remote_pushes_keep_both_nodes_reachable() {
    loom::model(|| {
        let cell = Arc::new(LoomAtomicUsize::new(0));
        let next_a = Arc::new(LoomAtomicUsize::new(0));
        let next_b = Arc::new(LoomAtomicUsize::new(0));

        let push_a = {
            let cell = Arc::clone(&cell);
            let next_a = Arc::clone(&next_a);
            thread::spawn(move || push_remote_word(&cell, &next_a, HEAD_A))
        };
        let push_b = {
            let cell = Arc::clone(&cell);
            let next_b = Arc::clone(&next_b);
            thread::spawn(move || push_remote_word(&cell, &next_b, HEAD_B))
        };

        push_a.join().expect("push A thread failed");
        push_b.join().expect("push B thread failed");

        let final_word = ThreadFreeWord(cell.load(LoomOrdering::Acquire));
        assert!(final_word.is_owned());

        let mut seen = [false; 2];
        mark_reachable(head_bits(final_word.raw()), &next_a, &next_b, &mut seen);
        assert_eq!(seen, [true, true]);
    });
}

#[test]
fn collect_racing_with_push_does_not_drop_nodes() {
    loom::model(|| {
        let cell = Arc::new(LoomAtomicUsize::new(HEAD_A | ThreadFreeWord::OWNED_BIT));
        let next_a = Arc::new(LoomAtomicUsize::new(0));
        let next_b = Arc::new(LoomAtomicUsize::new(0));
        let collected = Arc::new(LoomAtomicUsize::new(0));

        let collect = {
            let cell = Arc::clone(&cell);
            let collected = Arc::clone(&collected);
            thread::spawn(move || {
                collected.store(collect_word(&cell), LoomOrdering::Release);
            })
        };
        let push = {
            let cell = Arc::clone(&cell);
            let next_b = Arc::clone(&next_b);
            thread::spawn(move || push_remote_word(&cell, &next_b, HEAD_B))
        };

        collect.join().expect("collect thread failed");
        push.join().expect("push thread failed");

        let final_word = ThreadFreeWord(cell.load(LoomOrdering::Acquire));
        assert!(final_word.is_owned());

        let mut seen = [false; 2];
        mark_reachable(
            collected.load(LoomOrdering::Acquire),
            &next_a,
            &next_b,
            &mut seen,
        );
        mark_reachable(head_bits(final_word.raw()), &next_a, &next_b, &mut seen);
        assert_eq!(seen, [true, true]);
    });
}

#[test]
fn concurrent_claims_allow_exactly_one_owner_transition() {
    loom::model(|| {
        let cell = Arc::new(LoomAtomicUsize::new(0));
        let wins = Arc::new(LoomAtomicUsize::new(0));

        let first = {
            let cell = Arc::clone(&cell);
            let wins = Arc::clone(&wins);
            thread::spawn(move || {
                if claim_word(&cell) {
                    wins.fetch_add(1, LoomOrdering::Relaxed);
                }
            })
        };
        let second = {
            let cell = Arc::clone(&cell);
            let wins = Arc::clone(&wins);
            thread::spawn(move || {
                if claim_word(&cell) {
                    wins.fetch_add(1, LoomOrdering::Relaxed);
                }
            })
        };

        first.join().expect("first claim thread failed");
        second.join().expect("second claim thread failed");

        assert_eq!(wins.load(LoomOrdering::Acquire), 1);
        assert_eq!(
            cell.load(LoomOrdering::Acquire) & ThreadFreeWord::OWNED_BIT,
            ThreadFreeWord::OWNED_BIT
        );
    });
}

#[test]
fn release_ownership_racing_with_push_preserves_heads() {
    loom::model(|| {
        let cell = Arc::new(LoomAtomicUsize::new(HEAD_A | ThreadFreeWord::OWNED_BIT));
        let next_a = Arc::new(LoomAtomicUsize::new(0));
        let next_b = Arc::new(LoomAtomicUsize::new(0));

        let release = {
            let cell = Arc::clone(&cell);
            thread::spawn(move || {
                assert!(release_ownership_word(&cell));
            })
        };
        let push = {
            let cell = Arc::clone(&cell);
            let next_b = Arc::clone(&next_b);
            thread::spawn(move || push_remote_word(&cell, &next_b, HEAD_B))
        };

        release.join().expect("release thread failed");
        push.join().expect("push thread failed");

        let final_word = ThreadFreeWord(cell.load(LoomOrdering::Acquire));
        let mut seen = [false; 2];
        mark_reachable(head_bits(final_word.raw()), &next_a, &next_b, &mut seen);
        assert_eq!(seen, [true, true]);
    });
}
