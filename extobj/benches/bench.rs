use criterion::{Criterion, criterion_group, criterion_main};
use extobj::{ExtObj, extobj};
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

criterion_group!(benches, ext_obj_benchmark);
criterion_main!(benches);
