//! Benchmarks for memory allocators.

use criterion::{black_box, criterion_group, criterion_main, Criterion};

use graphos_common::memory::arena::Arena;
use graphos_common::memory::bump::BumpAllocator;
use graphos_common::memory::pool::ObjectPool;
use graphos_common::types::EpochId;

fn bench_epoch_arena_allocate(c: &mut Criterion) {
    c.bench_function("epoch_arena_allocate_1000", |b| {
        b.iter(|| {
            let arena = Arena::new(EpochId::INITIAL);
            for _ in 0..1000 {
                black_box(arena.alloc_value(42u64));
            }
        });
    });
}

fn bench_bump_allocator(c: &mut Criterion) {
    c.bench_function("bump_allocate_1000", |b| {
        b.iter(|| {
            let bump = BumpAllocator::with_capacity(64 * 1024);
            for _ in 0..1000 {
                black_box(bump.alloc(42u64));
            }
        });
    });
}

fn bench_object_pool(c: &mut Criterion) {
    c.bench_function("object_pool_get_put_1000", |b| {
        let pool: ObjectPool<Vec<u8>> = ObjectPool::new(|| Vec::with_capacity(1024));

        b.iter(|| {
            let mut handles = Vec::with_capacity(100);
            for _ in 0..100 {
                handles.push(pool.get());
            }
            for handle in handles {
                drop(handle);
            }
        });
    });
}

criterion_group!(
    benches,
    bench_epoch_arena_allocate,
    bench_bump_allocator,
    bench_object_pool,
);

criterion_main!(benches);
