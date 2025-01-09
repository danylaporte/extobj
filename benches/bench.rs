use criterion::{criterion_group, criterion_main, Criterion};
use extobj::{extobj, ExtObj, TyStore, UnsafeObj};
use std::hint::black_box;

fn ext_obj_benchmark(c: &mut Criterion) {
    extobj!(struct M);
    extobj!(impl M {
        V: i32,
    });

    let mut obj = ExtObj::<M>::new();

    c.bench_function("ext_obj::get_i32", |b| b.iter(|| black_box(&obj).get(*V)));

    c.bench_function("ext_obj::get_mut_i32", |b| {
        b.iter(|| {
            let _ = black_box(black_box(&mut obj).get_mut(*V));
        })
    });
}

fn obj_benchmark(c: &mut Criterion) {
    let mut obj = UnsafeObj::new(34i32);

    c.bench_function("obj::get_i32", |b| {
        b.iter(|| unsafe { black_box(&obj).get_unchecked::<i32>() });
    });

    c.bench_function("obj::get_mut_i32", |b| {
        b.iter(|| {
            let _ = black_box(unsafe { black_box(&mut obj).get_unchecked_mut::<i32>() });
        })
    });
}

fn ty_store_benchmark(c: &mut Criterion) {
    let mut store = TyStore::new();
    store.insert(34i32);

    c.bench_function("TyStore::get_i32", |b| {
        b.iter(|| black_box(&store).get::<i32>())
    });

    c.bench_function("TyStore::get_mut_i32", |b| {
        b.iter(|| {
            let _ = black_box(black_box(&mut store).get_mut::<i32>());
        })
    });
}

criterion_group!(
    benches,
    ty_store_benchmark,
    obj_benchmark,
    ext_obj_benchmark
);
criterion_main!(benches);
