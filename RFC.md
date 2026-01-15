# OxideC RFC: Alpha ABI Specification

**Author:** Junaadh
**Status:** Alpha (Phase 3 Complete)
**Date:** 2026-01-14
**Version:** See [Cargo.toml](Cargo.toml) for current version
**Last Updated:** 2026-01-16

## Phase Status Summary

| Phase | Status | Completion Date | Test Coverage |
|-------|--------|-----------------|---------------|
| Phase 1: Foundation | COMPLETE | 2026-01-15 | 42 unit tests |
| Phase 2: Dispatch | COMPLETE | 2026-01-15 | 61 unit tests |
| Phase 3: Extensions | COMPLETE | 2026-01-15 | 45 unit tests + 16 integration tests |
| **Total** | **COMPLETE** | **2026-01-15** | **148 unit + 16 integration + 74 doctests = 238 tests** |

**MIRI Validation:** All tests pass with `-Zmiri-strict-provenance -Zmiri-ignore-leaks`

---

## 1. Abstract

OxideC is a dynamic object runtime inspired by Objective-C, redesigned for modern systems programming. This RFC defines the initial ABI, runtime object model, and feature set.

**Core Objectives:**
- Dynamic dispatch with late binding
- Manual memory management for performance
- Runtime reflection metadata
- Extensible classes and methods
- C ABI compatibility for FFI
- Modern language features

**Target Audience:** Systems programmers, language designers, plugin system developers, framework authors

---

## 2. Design Principles

- **C ABI First** — All public types and functions are `extern "C"` compatible
- **Separation of Concerns** — Internal runtime complexity hidden behind stable ABI
- **Dynamic but Type-Aware** — Runtime polymorphism with compile-time type hints
- **Extensible and Observable** — Full metadata introspection for reflection and tooling
- **Performance-Conscious** — Selector caches, arenas, tagged pointers for optimization

---

## 3. Core Runtime Types

### 3.1 Object

```rust
pub struct Object {
    ptr: NonNull<RawObject>,
}

#[repr(C)]
pub(crate) struct RawObject {
    class_ptr: *const RawClass,
    flags: u32,
    refcount: AtomicU32,
    payload: *mut u8,
}
```

**Invariants:**
- `class_ptr` must always point to valid `RawClass`
- `refcount` is atomic and never overflows
- `payload` is allocated and deallocated with matching allocator
- Object is Send + Sync (requires atomic refcount)

### 3.2 Class

```rust
pub struct Class {
    // Manual memory management for performance
    inner: *mut ClassInner,
}

struct ClassInner {
    name: String,
    super_class: Option<Box<Class>>,
    methods: HashMap<String, Method>,
    flags: u32,
}
```

**Invariants:**
- Single inheritance chain only
- Methods form a hashmap for O(1) lookup
- Immutable after creation (except method addition)
- All classes inherit from root NSObject
- ClassInner pointer lifetime managed manually, valid for program duration
- Pointer dereference protected by SAFETY comments and unsafe blocks

### 3.3 Selector

```rust
pub struct Selector {
    name: String,
    hash: u64,
}
```

**Invariants:**
- All selectors are interned globally (one per unique name)
- Hash is stable and computed once
- Comparison uses pointer equality (fast)

---

## 4. Feature Implementation Matrix

### Phase 1: Foundation (Alpha 0.1) - COMPLETE

| Feature | Status | Target | Completed |
|---------|--------|--------|-----------|
| Selector interning | Complete | Week 1 | 2026-01-15 |
| Method registry | Complete | Week 1 | 2026-01-15 |
| Class creation | Complete | Week 2 | 2026-01-15 |
| Object allocation | Complete | Week 2 | 2026-01-15 |
| Reference counting | Complete | Week 2 | 2026-01-15 |

### Phase 2: Dispatch (Alpha 0.2) - COMPLETE

| Feature | Status | Target | Completed |
|---------|--------|--------|-----------|
| Message dispatch | Complete | Week 3 | 2026-01-15 |
| Method lookup | Complete | Week 3 | 2026-01-15 |
| Method caching | Complete | Week 3 | 2026-01-15 |
| Inheritance | Complete | Week 4 | 2026-01-15 |
| Method overriding | Complete | Week 4 | 2026-01-15 |

### Phase 3: Extensions (Alpha 0.3) - COMPLETE

| Feature | Status | Target | Completed |
|---------|--------|--------|-----------|
| Categories | Complete | Week 5 | 2026-01-15 |
| Protocols | Complete | Week 5 | 2026-01-15 |
| Forwarding | Complete | Week 6 | 2026-01-15 |
| Swizzling | Complete | Week 6 | 2026-01-15 |

### Phase 4: Optimization & Performance (Planned)

| Feature | Status | Target | Priority |
|---------|--------|--------|----------|
| Tagged pointers | Planned | Week 7 | High |
| Inline method caches | Planned | Week 7 | High |
| Selector optimization | Planned | Week 8 | Medium |
| Benchmark suite | Planned | Week 8 | High |

**Performance Targets:**
- Message dispatch (cached): < 50ns
- Message dispatch (uncached): < 100ns
- Object allocation: < 200ns
- Selector lookup: < 50ns
- Method swizzle: < 300ns

### Phase 5-6: Advanced Features (Future)

See ARCHITECTURE.md for detailed roadmap.

---

## 5. Memory Safety Model

### Safe Public API

All public functions:
- Validate input parameters
- Return `Result<T>` for fallible operations
- Prevent use-after-free via careful lifetime management and documentation
- Provide safe wrappers around unsafe internal implementation

### Unsafe Internal Implementation

Internal unsafe code:
- All raw pointer dereferences include SAFETY comments
- Manual memory management via Box::into_raw and explicit deallocation
- Atomic operations prevent data races for shared state
- Memory is managed with proper allocation/deallocation pairs
- Field access is validated before dereference
- Pointer validity proven before every use

### Example Pattern

```rust
// PUBLIC SAFE API
pub fn send_message(&self, selector: &Selector, args: &[*mut u8]) -> Result<*mut u8> {
    // Validation
    if args.len() > 32 {
        return Err(Error::ArgumentCountMismatch { expected: 32, got: args.len() });
    }
    
    // Delegate to unsafe runtime
    unsafe {
        self.dispatch_unsafe(selector, args)
    }
}

// INTERNAL UNSAFE IMPLEMENTATION
unsafe fn dispatch_unsafe(&self, selector: &Selector, args: &[*mut u8]) -> Result<*mut u8> {
    // SAFETY: class_ptr is valid (checked in constructor), dereference proves invariant
    // Lifetime: managed by Object ownership, remains valid for dispatch duration
    let class = &*self.ptr.as_ref().class_ptr;
    
    // Method lookup with caching
    let method = class.lookup_method(selector)?;
    
    // Call the implementation
    let mut ret = std::ptr::null_mut();
    // SAFETY: method.imp is function pointer from validated class, args properly constructed
    (method.imp)(self as *const _ as *mut _, selector, args.as_ptr() as *mut _, &mut ret);
    
    Ok(ret)
}
```

---

## 6. Compilation & Testing

### Build Features

```toml
[features]
default = ["safe-defaults"]
safe-defaults = []           # Full safety checks
unsafe-optimizations = []    # Skip some checks for speed
debug-assertions = []        # Extra validation in debug builds
thread-safe = ["parking_lot"] # Atomic refcounting
```

### Testing Strategy

1. **Unit Tests** - Test individual components in module files (148 tests passing)
2. **Integration Tests** - Test full workflows in /tests directory (16 tests passing)
3. **Doctests** - Test code examples in documentation (74 tests, 68 passing, 6 ignored)
4. **Property Tests** - Use proptest for generative testing
5. **Benchmarks** - Use criterion for performance validation
6. **Safety Validation** - Review all SAFETY comments manually
7. **MIRI Validation** - All code passes MIRI with `-Zmiri-strict-provenance -Zmiri-ignore-leaks`
   - Validates no undefined behavior
   - Checks pointer provenance correctness
   - Ensures proper alignment and memory safety
   - Current status: All 238 tests pass with strict provenance

---

## 7. C ABI Compatibility

All public types:
- Use `#[repr(C)]` for memory layout
- Have stable field offsets
- Are FFI-compatible
- Include `extern "C"` wrapper functions

```rust
#[repr(C)]
pub struct OxObject {
    class: *const OxClass,
    flags: u32,
    refcount: u32,
}

#[no_mangle]
pub extern "C" fn ox_object_retain(obj: *mut OxObject) {
    // Implementation
}
```

---

## 8. Performance Targets

### Latency
- Message dispatch: < 100ns (with cache hit)
- Selector lookup: < 10ns (cached)
- Object allocation: < 1µs
- Reference count ops: < 10ns

### Throughput
- 1M+ messages/sec per core
- 10K+ objects/sec allocation

### Memory
- 32 bytes per empty object (minimum)
- 8 bytes per method cache entry
- Zero allocation for selector interning (global pool)

---

## 9. Open Questions

1. **Metaclass Implementation:** Full Objective-C style or simplified?
2. **Thread Safety:** Atomic operations or external synchronization?
3. **ABI Versioning:** How to handle future breaking changes?
4. **Weak References:** Cost vs benefit trade-off?

---

## See Also

- [ARCHITECTURE.md](ARCHITECTURE.md) - Detailed design decisions
- [SAFETY.md](SAFETY.md) - Unsafe code guidelines
- [CLAUDE.md](CLAUDE.md) - Project coordination guide
