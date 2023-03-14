use std::time::{Duration, Instant};

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

trait BarrierLike: Clone + Send {
    fn wait(self);
}

impl BarrierLike for rendezvous::Rendezvous {
    fn wait(self) {
        rendezvous::Rendezvous::wait(self);
    }
}

impl BarrierLike for adaptive_barrier::Barrier {
    fn wait(mut self) {
        adaptive_barrier::Barrier::wait(&mut self);
    }
}

impl BarrierLike for crossbeam_utils::sync::WaitGroup {
    fn wait(self) {
        crossbeam_utils::sync::WaitGroup::wait(self)
    }
}

fn recurse_barrier<B: BarrierLike + 'static>(n_child: usize, rem_depth: usize, b: B) {
    if rem_depth == 0 {
        b.wait();
        return;
    }
    for _i in 0..n_child {
        let b = b.clone();
        let _h = std::thread::spawn(move || recurse_barrier(n_child, rem_depth - 1, b));
    }
    // This drop is made explicit but would have been done implicitely anyway
    drop(b)
}

const N_CHILD: usize = 2;

fn bench_rendezvous(depth: usize) -> Duration {
    let start = Instant::now();
    let b = rendezvous::Rendezvous::new();
    recurse_barrier(N_CHILD, depth, b.clone());
    b.wait();
    start.elapsed()
}
fn bench_adaptive(depth: usize) -> Duration {
    let start = Instant::now();
    let b = adaptive_barrier::Barrier::new(adaptive_barrier::PanicMode::Decrement);
    recurse_barrier(N_CHILD, depth, b.clone());
    b.wait();
    start.elapsed()
}
fn bench_crossbeam(depth: usize) -> Duration {
    let start = Instant::now();
    let b = crossbeam_utils::sync::WaitGroup::new();
    recurse_barrier(N_CHILD, depth, b.clone());
    b.wait();
    start.elapsed()
}

fn recurse_thread(n_child: usize, rem_depth: usize) {
    if rem_depth == 0 {
        return;
    }
    let mut handles = Vec::new();
    for _i in 0..n_child {
        let h = std::thread::spawn(move || recurse_thread(n_child, rem_depth - 1));
        handles.push(h);
    }
    for h in handles {
        h.join().unwrap();
    }
}

fn bench_threads(depth: usize) -> Duration {
    let start = Instant::now();
    recurse_thread(N_CHILD, depth);
    start.elapsed()
}

fn bench_power_2(c: &mut Criterion) {
    let mut group = c.benchmark_group("Thread three (2 children)");
    for depth in 1..=10 {
        group.bench_with_input(BenchmarkId::new("Rendezvous", depth), &depth, |b, i| {
            b.iter(|| bench_rendezvous(*i))
        });
        group.bench_with_input(BenchmarkId::new("Adaptive", depth), &depth, |b, i| {
            b.iter(|| bench_adaptive(*i))
        });
        group.bench_with_input(BenchmarkId::new("Crossbeam", depth), &depth, |b, i| {
            b.iter(|| bench_crossbeam(*i))
        });
        group.bench_with_input(BenchmarkId::new("Threads", depth), &depth, |b, i| {
            b.iter(|| bench_threads(*i))
        });
    }
    group.finish();
}

criterion_group!(benches, bench_power_2);
criterion_main!(benches);
