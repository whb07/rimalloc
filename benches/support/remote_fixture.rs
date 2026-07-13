use std::alloc::{GlobalAlloc, Layout};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use super::no_alloc::assert_no_harness_allocations_enabled;
use super::pool::PointerPool;

enum OwnerCommand {
    Prepare(PointerPool),
    Collect(PointerPool),
    Shutdown,
}

enum OwnerReply {
    Prepared(PointerPool),
    Collected(PointerPool),
}

pub struct RemoteFreeFixture<A: GlobalAlloc + Sync + 'static> {
    allocator: &'static A,
    layout: Layout,
    commands: mpsc::SyncSender<OwnerCommand>,
    replies: mpsc::Receiver<OwnerReply>,
    owner: Option<thread::JoinHandle<()>>,
    pool: Option<PointerPool>,
    verify_harness: bool,
}

fn remote_free_timed_routine<A: GlobalAlloc>(
    allocator: &A,
    layout: Layout,
    pool: &mut PointerPool,
) {
    for index in 0..pool.len() {
        let (ptr, pointer_layout) = *pool.slot(index);
        unsafe { allocator.dealloc(ptr, pointer_layout) };
        *pool.slot(index) = (std::ptr::null_mut(), layout);
    }
}

fn measure_remote_free_routine<A: GlobalAlloc>(
    allocator: &A,
    layout: Layout,
    pool: &mut PointerPool,
) -> Duration {
    let started = Instant::now();
    remote_free_timed_routine(allocator, layout, pool);
    started.elapsed()
}

fn measure_remote_free_routine_checked<A: GlobalAlloc>(
    allocator: &A,
    layout: Layout,
    pool: &mut PointerPool,
) -> Duration {
    assert_no_harness_allocations_enabled(|| measure_remote_free_routine(allocator, layout, pool))
}

impl<A: GlobalAlloc + Sync + 'static> RemoteFreeFixture<A> {
    pub fn new(
        allocator: &'static A,
        layout: Layout,
        capacity: usize,
        verify_harness: bool,
    ) -> Self {
        let (commands, command_rx) = mpsc::sync_channel(1);
        let (reply_tx, replies) = mpsc::sync_channel(1);
        let owner = thread::spawn(move || {
            while let Ok(command) = command_rx.recv() {
                match command {
                    OwnerCommand::Prepare(mut pool) => {
                        for index in 0..pool.len() {
                            let pointer = unsafe { allocator.alloc(layout) };
                            assert!(
                                !pointer.is_null(),
                                "remote-free benchmark setup allocation failed"
                            );
                            *pool.slot(index) = (pointer, layout);
                        }
                        if reply_tx.send(OwnerReply::Prepared(pool)).is_err() {
                            break;
                        }
                    }
                    OwnerCommand::Collect(pool) => {
                        assert!(pool.all_empty(), "collected remote-free pool must be empty");
                        if reply_tx.send(OwnerReply::Collected(pool)).is_err() {
                            break;
                        }
                    }
                    OwnerCommand::Shutdown => break,
                }
            }
        });
        Self {
            allocator,
            layout,
            commands,
            replies,
            owner: Some(owner),
            pool: Some(PointerPool::new(capacity)),
            verify_harness,
        }
    }

    pub fn measure_remote_free(&mut self, iterations: u64) -> Duration {
        let measure = if self.verify_harness {
            measure_remote_free_routine_checked::<A>
        } else {
            measure_remote_free_routine::<A>
        };
        self.measure_remote_free_inner(iterations, measure)
    }

    fn measure_remote_free_inner(
        &mut self,
        iterations: u64,
        measure: fn(&A, Layout, &mut PointerPool) -> Duration,
    ) -> Duration {
        let mut total = Duration::ZERO;
        for _ in 0..iterations {
            self.commands
                .send(OwnerCommand::Prepare(
                    self.pool.take().expect("fixture pool"),
                ))
                .expect("prepare remote-free pool");
            let OwnerReply::Prepared(mut pool) = self.replies.recv().expect("owner reply") else {
                panic!("owner must prepare the pool before collection");
            };
            total += measure(self.allocator, self.layout, &mut pool);
            assert!(pool.all_empty());
            self.commands
                .send(OwnerCommand::Collect(pool))
                .expect("return empty remote-free pool");
            let OwnerReply::Collected(pool) = self.replies.recv().expect("owner collection reply")
            else {
                panic!("owner must acknowledge pool collection before preparation");
            };
            self.pool = Some(pool);
        }
        total
    }
}

impl<A: GlobalAlloc + Sync + 'static> Drop for RemoteFreeFixture<A> {
    fn drop(&mut self) {
        assert!(self.pool.as_ref().is_some_and(PointerPool::all_empty));
        let _ = self.commands.send(OwnerCommand::Shutdown);
        if let Some(owner) = self.owner.take() {
            owner.join().expect("remote-free owner thread");
        }
    }
}
