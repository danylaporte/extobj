use criterion::{Criterion, criterion_group, criterion_main};
use extobj::{DynObj, ExtObj, extobj};
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

    extobj!(struct N);
    extobj!(impl N {
        A: i32,
        B: u64,
        C: String,
        D: Vec<u8>,
        E: bool,
    });

    c.bench_function("ext_obj::new_drop_5_fields", |b| {
        b.iter(|| black_box(ExtObj::<N>::new()))
    });
}

fn dyn_obj_benchmark(c: &mut Criterion) {
    c.bench_function("dyn_obj::new_drop_u64", |b| {
        b.iter(|| black_box(DynObj::new(black_box(42u64))))
    });

    c.bench_function("dyn_obj::new_drop_string", |b| {
        b.iter(|| black_box(DynObj::new(black_box(String::new()))))
    });

    let small = DynObj::new(42u64);
    c.bench_function("dyn_obj::get_u64", |b| {
        b.iter(|| unsafe { *black_box(&small).get::<u64>() })
    });
}

criterion_group!(benches, ext_obj_benchmark, dyn_obj_benchmark);
criterion_main!(benches);
