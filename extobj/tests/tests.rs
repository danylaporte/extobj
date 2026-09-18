use extobj::{DynObj, ExtObj, RwLock, extobj};
use std::sync::atomic::{AtomicUsize, Ordering};

// Minimal sanity

extobj!(pub struct TestObj);

extobj!(impl TestObj {
    FOO: i32,
});

#[test]
fn default_zero() {
    let o = ExtObj::<TestObj>::new();
    assert_eq!(o[*FOO], 0);
}

#[test]
fn index_mut() {
    let mut o = ExtObj::<TestObj>::new();
    o[*FOO] = 42;
    assert_eq!(o[*FOO], 42);
}

// Multiple types in one object

extobj!(impl TestObj {
    COUNTER: AtomicUsize,
    VEC: Vec<String>,
});

#[test]
fn heterogeneous_fields() {
    let mut o = ExtObj::<TestObj>::new();

    o[*COUNTER].store(7, Ordering::Relaxed);
    o[*VEC].push("hello".into());

    assert_eq!(o[*COUNTER].load(Ordering::Relaxed), 7);
    assert_eq!(o[*VEC], ["hello"]);
}

// Drop is executed

static DROP_COUNT: AtomicUsize = AtomicUsize::new(0);

#[derive(Default)]
struct Droppable;

impl Drop for Droppable {
    fn drop(&mut self) {
        DROP_COUNT.fetch_add(1, Ordering::SeqCst);
    }
}

extobj!(struct DropTest);

extobj!(impl DropTest {
    D: Droppable,
});

#[test]
fn drops_are_called() {
    {
        let _o = ExtObj::<DropTest>::new();
        assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 0);
    }
    assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 1);
}

// Two independent extension objects

extobj!(struct A);
extobj!(impl A { XA: u8 });

extobj!(struct B);
extobj!(impl B { XB: u16 });

#[test]
fn independent_types() {
    let mut a = ExtObj::<A>::new();
    let mut b = ExtObj::<B>::new();
    a[*XA] = 11;
    b[*XB] = 2222;
    assert_ne!(a[*XA] as u16, b[*XB]);
}

// Clone, Copy, Default impls for ExtObj

#[test]
fn ext_obj_is_default() {
    let o: ExtObj<TestObj> = Default::default();
    assert_eq!(o[*FOO], 0);
}

// Thread-safety (smoke test)

#[test]
fn concurrent_access() {
    use std::thread;

    let o = std::sync::Arc::new(RwLock::new(ExtObj::<TestObj>::new()));

    let mut handles = vec![];
    for i in 0..4 {
        let o = o.clone();
        handles.push(thread::spawn(move || {
            let g = o.write();
            g[*COUNTER].fetch_add(i, Ordering::Relaxed);
        }));
    }
    for h in handles {
        h.join().unwrap();
    }

    let total = o.read()[*COUNTER].load(Ordering::Relaxed);
    assert_eq!(total, 0 + 1 + 2 + 3);
}

mod inside_another_mod {
    super::extobj!(pub(super) struct Insider);
}

extobj!(impl inside_another_mod::Insider { InsiderID: u32 });

// DynObj: inline and boxed storage paths

static DYN_DROPS: AtomicUsize = AtomicUsize::new(0);

struct SmallDrop(u8);

impl Drop for SmallDrop {
    fn drop(&mut self) {
        DYN_DROPS.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn dynobj_inline_plain() {
    let mut o = DynObj::new(0xABu8);
    unsafe {
        assert_eq!(*o.get::<u8>(), 0xAB);
        *o.get_mut::<u8>() = 7;
        assert_eq!(o.into_inner::<u8>(), 7);
    }

    let o = DynObj::new(u64::MAX);
    assert_eq!(unsafe { o.into_inner::<u64>() }, u64::MAX);
}

#[test]
fn dynobj_inline_with_drop() {
    let before = DYN_DROPS.load(Ordering::SeqCst);
    {
        let o = DynObj::new(SmallDrop(1));
        assert_eq!(unsafe { o.get::<SmallDrop>() }.0, 1);
    }
    assert_eq!(DYN_DROPS.load(Ordering::SeqCst), before + 1);

    // into_inner must move without running the destructor twice
    let o = DynObj::new(SmallDrop(2));
    let v = unsafe { o.into_inner::<SmallDrop>() };
    assert_eq!(DYN_DROPS.load(Ordering::SeqCst), before + 1);
    drop(v);
    assert_eq!(DYN_DROPS.load(Ordering::SeqCst), before + 2);
}

#[test]
fn dynobj_boxed() {
    let mut o = DynObj::new([1u64, 2, 3, 4]);
    unsafe {
        o.get_mut::<[u64; 4]>()[0] = 9;
        assert_eq!(*o.get::<[u64; 4]>(), [9, 2, 3, 4]);
    }

    let s = DynObj::new(String::from("hello"));
    assert_eq!(unsafe { s.into_inner::<String>() }, "hello");
}
