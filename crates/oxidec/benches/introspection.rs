//! Introspection performance benchmarks (minimal version)
//!
//! Benchmarks for runtime introspection APIs to validate performance targets:
//! - all_classes() < 100μs
//! - instance_methods() < 10μs
//! - class_from_name() < 50ns
//!
//! Run with: `cargo bench --bench introspection`

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use oxidec::runtime::selector::SelectorHandle;
use oxidec::runtime::{
    Class, Method, Object, RuntimeString, Selector, get_global_arena,
    introspection::*,
};
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};

static BENCH_ID: AtomicUsize = AtomicUsize::new(0);

unsafe extern "C" fn test_impl(
    _self: oxidec::runtime::object::ObjectPtr,
    _cmd: SelectorHandle,
    _args: *const *mut u8,
    _ret: *mut u8,
) {
}

/// Generate a unique class name for benchmarks
fn unique_name(prefix: &str) -> String {
    let id = BENCH_ID.fetch_add(1, Ordering::SeqCst);
    let thread_id = std::thread::current().id();
    format!("{}_{:?}_{}", prefix, thread_id, id)
}

fn bench_all_classes(c: &mut Criterion) {
    // Create a small number of test classes
    for _ in 0..10 {
        let class_name = unique_name("BenchClass");
        let _ = Class::new_root(&class_name);
    }

    c.bench_function("all_classes", |b| {
        b.iter(|| {
            let classes = all_classes();
            black_box(classes);
        });
    });
}

fn bench_class_from_name(c: &mut Criterion) {
    let class_name = unique_name("LookupClass");
    let _class = Class::new_root(&class_name).unwrap();

    c.bench_function("class_from_name", |b| {
        b.iter(|| {
            let class = class_from_name(black_box(&class_name));
            black_box(class);
        });
    });
}

fn bench_class_hierarchy(c: &mut Criterion) {
    let root = Class::new_root(&unique_name("HierarchyRoot")).unwrap();
    let level1 = Class::new(&unique_name("Level1"), &root).unwrap();
    let level2 = Class::new(&unique_name("Level2"), &level1).unwrap();
    let level3 = Class::new(&unique_name("Level3"), &level2).unwrap();

    let mut group = c.benchmark_group("class_hierarchy");

    group.bench_function("depth_3", |b| {
        b.iter(|| {
            let hierarchy = class_hierarchy(black_box(&level3));
            black_box(hierarchy);
        });
    });

    group.finish();
}

fn bench_is_subclass(c: &mut Criterion) {
    let parent = Class::new_root(&unique_name("Parent")).unwrap();
    let child = Class::new(&unique_name("Child"), &parent).unwrap();
    let unrelated = Class::new_root(&unique_name("Unrelated")).unwrap();

    let mut group = c.benchmark_group("is_subclass");

    group.bench_function("true", |b| {
        b.iter(|| {
            let result = is_subclass(black_box(&child), black_box(&parent));
            black_box(result);
        });
    });

    group.bench_function("false", |b| {
        b.iter(|| {
            let result = is_subclass(black_box(&unrelated), black_box(&parent));
            black_box(result);
        });
    });

    group.finish();
}

fn bench_instance_methods(c: &mut Criterion) {
    // Create class with a few methods
    let class = Class::new_root(&unique_name("MethodsClass")).unwrap();

    for i in 0..5 {
        let selector = Selector::from_str(&format!("method{}:", i)).unwrap();
        let method = Method {
            selector,
            imp: test_impl,
            types: RuntimeString::new("", get_global_arena()),
        };
        let _ = class.add_method(method);
    }

    c.bench_function("instance_methods", |b| {
        b.iter(|| {
            let methods = instance_methods(black_box(&class));
            black_box(methods);
        });
    });
}

fn bench_has_method(c: &mut Criterion) {
    let class = Class::new_root(&unique_name("HasMethodClass")).unwrap();

    let selector = Selector::from_str("testMethod:").unwrap();
    let method = Method {
        selector: selector.clone(),
        imp: test_impl,
        types: RuntimeString::new("", get_global_arena()),
    };
    let _ = class.add_method(method);

    c.bench_function("has_method", |b| {
        b.iter(|| {
            let result = has_method(black_box(&class), black_box(&selector));
            black_box(result);
        });
    });
}

fn bench_method_provider(c: &mut Criterion) {
    let parent = Class::new_root(&unique_name("ProviderParent")).unwrap();
    let child = Class::new(&unique_name("ProviderChild"), &parent).unwrap();

    let selector = Selector::from_str("testMethod:").unwrap();
    let method = Method {
        selector: selector.clone(),
        imp: test_impl,
        types: RuntimeString::new("", get_global_arena()),
    };
    let _ = parent.add_method(method);

    c.bench_function("method_provider", |b| {
        b.iter(|| {
            let provider =
                method_provider(black_box(&child), black_box(&selector));
            black_box(provider);
        });
    });
}

fn bench_object_get_class(c: &mut Criterion) {
    let class = Class::new_root(&unique_name("ObjectGetClass")).unwrap();
    let object = Object::new(&class).unwrap();

    c.bench_function("object_get_class", |b| {
        b.iter(|| {
            let obj_class = object_get_class(black_box(&object));
            black_box(obj_class);
        });
    });
}

fn bench_object_is_instance(c: &mut Criterion) {
    let class = Class::new_root(&unique_name("IsInstanceClass")).unwrap();
    let object = Object::new(&class).unwrap();
    let other_class = Class::new_root(&unique_name("OtherClass")).unwrap();

    let mut group = c.benchmark_group("object_is_instance");

    group.bench_function("true", |b| {
        b.iter(|| {
            let result =
                object_is_instance(black_box(&object), black_box(&class));
            black_box(result);
        });
    });

    group.bench_function("false", |b| {
        b.iter(|| {
            let result =
                object_is_instance(black_box(&object), black_box(&other_class));
            black_box(result);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_all_classes,
    bench_class_from_name,
    bench_class_hierarchy,
    bench_is_subclass,
    bench_instance_methods,
    bench_has_method,
    bench_method_provider,
    bench_object_get_class,
    bench_object_is_instance,
);

criterion_main!(benches);
