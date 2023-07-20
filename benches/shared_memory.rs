use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::Rng;

use shared_memory::*;

fn shared_memory_access(shmem: &mut Shmem, stride: usize) -> u64 {
    let slice = unsafe { std::slice::from_raw_parts_mut(shmem.as_ptr(), shmem.len()) };
    for i in (0..slice.len()).step_by(stride) {
        slice[i] = rand::thread_rng().gen();
    }
    let mut res = 0;
    for i in (0..slice.len()).step_by(stride) {
        res += slice[i] as u64;
    }
    res
}

fn shared_memory_recreate(len: usize, stride: usize) -> u64 {
    let mut shmem = shared_memory_create(len);
    shared_memory_access(&mut shmem, stride)
}

fn shared_memory_create(len: usize) -> Shmem {
    ShmemConf::new().size(len).create().unwrap()
}

fn criterion_benchmark(c: &mut Criterion) {
    let len: usize = 2048 * 2048;
    let stride = 4096; // pagesize
    let mut shmem = shared_memory_create(len);
    c.bench_function("shared memory reuse", |b| {
        b.iter(|| shared_memory_access(black_box(&mut shmem), black_box(stride)))
    });
    c.bench_function("shared memory create & access", |b| {
        b.iter(|| shared_memory_recreate(black_box(len), black_box(stride)))
    });
    c.bench_function("shared memory init", |b| {
        b.iter(|| shared_memory_create(black_box(len)))
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
