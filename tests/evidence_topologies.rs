//! Realistic ownership-transfer coverage for the allocator evidence layer.

#![cfg(feature = "std")]

use mechanical_mimalloc::Allocation;
use std::sync::{Arc, Barrier, mpsc};
use std::thread;

const TRANSFERS: usize = 256;
const ROUNDS: usize = 16;

fn batch(seed: usize) -> Vec<Allocation> {
    (0..TRANSFERS)
        .map(|index| Allocation::new_bytes(8 + (index + seed) % 2048).unwrap())
        .collect()
}

#[test]
fn persistent_producer_owner_remote_free() {
    let (commands, owner_rx) = mpsc::sync_channel::<usize>(1);
    let (allocations, remote_rx) = mpsc::sync_channel(1);
    let owner = thread::spawn(move || {
        while let Ok(seed) = owner_rx.recv() {
            allocations.send(batch(seed)).unwrap();
        }
    });
    for round in 0..ROUNDS {
        commands.send(round).unwrap();
        drop(remote_rx.recv().unwrap());
    }
    drop(commands);
    owner.join().unwrap();
}

#[test]
fn fan_in_remote_free() {
    let (tx, rx) = mpsc::sync_channel(4);
    let producers: Vec<_> = (0..4)
        .map(|seed| {
            let tx = tx.clone();
            thread::spawn(move || {
                for round in 0..ROUNDS {
                    tx.send(batch(seed + round)).unwrap();
                }
            })
        })
        .collect();
    drop(tx);
    for allocations in rx {
        drop(allocations);
    }
    for producer in producers {
        producer.join().unwrap();
    }
}

#[test]
fn fan_out_remote_free() {
    let mut workers = Vec::new();
    for seed in 0..4 {
        let (tx, rx) = mpsc::sync_channel(1);
        workers.push((
            tx,
            thread::spawn(move || {
                for _ in 0..ROUNDS {
                    drop(rx.recv().unwrap());
                }
            }),
        ));
        for round in 0..ROUNDS {
            workers.last().unwrap().0.send(batch(seed + round)).unwrap();
        }
    }
    for (_, worker) in workers {
        worker.join().unwrap();
    }
}

#[test]
fn cyclic_ownership_transfer() {
    let (tx0, rx0) = mpsc::sync_channel(1);
    let (tx1, rx1) = mpsc::sync_channel(1);
    let (tx2, rx2) = mpsc::sync_channel(1);
    let (done_tx, done_rx) = mpsc::sync_channel(1);
    let first = thread::spawn(move || {
        while let Ok(allocations) = rx0.recv() {
            tx1.send(allocations).unwrap();
        }
    });
    let second = thread::spawn(move || {
        while let Ok(allocations) = rx1.recv() {
            tx2.send(allocations).unwrap();
        }
    });
    let third = thread::spawn(move || {
        while let Ok(allocations) = rx2.recv() {
            done_tx.send(allocations).unwrap();
        }
    });
    for round in 0..ROUNDS {
        tx0.send(batch(round)).unwrap();
        drop(done_rx.recv().unwrap());
    }
    drop(tx0);
    first.join().unwrap();
    second.join().unwrap();
    third.join().unwrap();
}

#[test]
fn thread_churn_and_owner_exit() {
    for seed in 0..32 {
        let abandoned = thread::spawn(move || batch(seed)).join().unwrap();
        drop(abandoned);
    }
}

#[test]
fn oversubscribed_remote_free() {
    let workers = thread::available_parallelism().map_or(4, |count| count.get() * 2);
    let barrier = Arc::new(Barrier::new(workers + 1));
    let mut senders = Vec::with_capacity(workers);
    let handles: Vec<_> = (0..workers)
        .map(|_seed| {
            let barrier = Arc::clone(&barrier);
            let (tx, rx) = mpsc::sync_channel(1);
            senders.push(tx);
            thread::spawn(move || {
                barrier.wait();
                for _ in 0..ROUNDS {
                    drop(rx.recv().unwrap());
                }
            })
        })
        .collect();
    barrier.wait();
    for round in 0..ROUNDS {
        for (seed, sender) in senders.iter().enumerate() {
            sender.send(batch(seed + round)).unwrap();
        }
    }
    drop(senders);
    for handle in handles {
        handle.join().unwrap();
    }
}
