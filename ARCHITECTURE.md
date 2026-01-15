# OxideC Architecture & Design

**Version:** See [Cargo.toml](Cargo.toml) for current version
**Status:** See [RFC.md](RFC.md) for implementation status and roadmap

## System Architecture

OxideC is organized in three layers with clear safety boundaries:

### Layer 1: Safe Public API
```
┌─────────────────────────────────────┐
│     Safe Rust Public API (pub)      │
│  ├── Object, Class, Selector        │
│  ├── Message dispatch with Result   │
│  └── Protocol conformance checks    │
└─────────────────────────────────────┘
         ↓ (enforces invariants)
```

### Layer 2: Unsafe Runtime Implementation
```
┌─────────────────────────────────────┐
│  Unsafe Internals (pub(crate))      │
│  ├── RawObject memory management    │
│  ├── Method dispatch primitives     │
│  ├── Selector interning pool        │
│  └── Method caching with atomics    │
└─────────────────────────────────────┘
         ↓ (manages resources)
```

### Layer 3: Host Memory
```
┌─────────────────────────────────────┐
│  System Memory & Allocator          │
│  ├── Heap allocations               │
│  ├── Atomic operations              │
│  └── Platform primitives            │
└─────────────────────────────────────┘
```

---

## Module Map

```
src/
├── lib.rs                 # Public API entry, re-exports
├── error.rs               # Error types (Result, Error enum)
│
└── runtime/
    ├── mod.rs             # Runtime singleton, initialization
    ├── arena.rs           # Arena allocator for long-lived metadata
    ├── object.rs          # Object allocation, lifecycle, refcounting
    ├── class.rs           # Class creation, inheritance, methods
    ├── selector.rs        # Selector interning, caching, hashing
    ├── dispatch.rs        # Message dispatch, method lookup
    ├── encoding.rs        # Method signature type encoding
    ├── message.rs         # Message argument handling
    ├── string.rs          # Runtime string with SSO and interning
    ├── cache.rs           # Method call caching, optimization
    └── protocol.rs        # Protocol definition, conformance, inheritance
```

---

## Core Concepts

### Selector Interning

**Goal:** Ensure each unique selector name has exactly one global `Selector` instance.

**Implementation:**
```rust
static SELECTOR_CACHE: OnceLock<DashMap<String, *const Selector>> = OnceLock::new();

pub fn selector(name: &str) -> *const Selector {
    let cache = SELECTOR_CACHE.get_or_init(DashMap::new);
    cache.entry(name.to_string())
        .or_insert_with(|| unsafe {
            // SAFETY: Box::into_raw creates a stable pointer in global cache
            // Lifetime is tied to static SELECTOR_CACHE (never deallocated)
            Box::into_raw(Box::new(Selector {
                name: name.to_string(),
                hash: compute_hash(name),
            }))
        })
        .clone()
}
```

**Invariants:**
- All selectors are cached globally with manually managed lifetime
- Same name = same pointer (fast equality check, pointer equality)
- Hash is precomputed and stable
- Pointers remain valid for entire program lifetime (static cache ownership)

### Reference Counting

**Goal:** Prevent use-after-free while maintaining performance.

**Implementation:**
```rust
pub fn retain(&self) {
    unsafe {
        // SAFETY: We hold NonNull, refcount access is atomic
        let refcount = &self.ptr.as_ref().refcount;
        let old = refcount.fetch_add(1, Ordering::AcqRel);
        
        if old == u32::MAX {
            panic!("reference count overflow");
        }
    }
}
```

**Invariants:**
- Refcount never wraps (checked)
- Atomic operations prevent races
- Drop decrements refcount automatically
- Zero refcount triggers deallocation

### Method Lookup

**Goal:** Fast resolution of selectors to method implementations.

**Strategy:**
1. Check inline cache (zero cost on hit)
2. Search method table in class
3. Walk inheritance chain
4. Call forwarding hook if not found
5. Cache result for future calls

**Caching:**
```rust
struct MethodCache {
    // Per-class cache of selector -> IMP
    // Manually managed with unsafe pointer access for performance
    entries: *mut DashMap<SelectorHash, Method>,
}

fn lookup(class: &Class, sel: &Selector) -> Option<&Method> {
    // 1. Check cache (fast path)
    if let Some(cached) = class.cache.get(&sel.hash) {
        return Some(cached);
    }
    
    // 2. Walk inheritance chain
    for cls in class.ancestors() {
        if let Some(method) = cls.methods.get(&sel.name) {
            // 3. Cache the result
            class.cache.insert(sel.hash, method.clone());
            return Some(method);
        }
    }
    
    None
}
```

### Protocols

**Goal:** Type-safe interface definitions with optional validation.

**Implementation:**
Protocols define interfaces that classes can conform to, with a hybrid validation approach:

```rust
// Declarative conformance (default, like Objective-C)
class.add_protocol(&protocol)?;  // No validation, flexible

// Optional runtime validation
class.validate_protocol_conformance(&protocol)?;  // Explicit validation
```

**Validation Strategy:**
1. **Declarative (Default)**: Classes declare protocol conformance without validation
   - Flexible for dynamic classes that add methods via categories
   - No upfront validation cost
   - Matches Objective-C's approach

2. **Optional Runtime Validation**: Explicit validation when stricter safety is desired
   - Walks class hierarchy: local → categories → superclass
   - Returns `Err(MissingProtocolMethod)` if required methods missing
   - Recommended for static/trusted classes at load-time

**Protocol Inheritance:**
```rust
// Base protocol
let base = Protocol::new("BaseProtocol", None)?;
base.add_required(sel1, "v@:", arena)?;

// Derived protocol inherits from base
let derived = Protocol::new("DerivedProtocol", Some(&base))?;
derived.add_required(sel2, "v@:", arena)?;

// All required methods include base protocol methods
let all_req = derived.all_required();  // [sel1, sel2]
```

**Conformance Checking:**
```rust
// Conformance is transitive through inheritance
parent.add_protocol(&protocol)?;
child.conforms_to(&protocol);  // true (inherited from parent)

// Get all protocols (including inherited)
let protocols = class.protocols();  // [protocol, ...]
```

**Invariants:**
- Protocols allocated in global arena (never deallocated)
- Protocol definitions are immutable after creation
- Methods can only be added during protocol construction
- Conformance is transitive through inheritance
- Protocol methods don't participate in dispatch (type checking only)

---

## Development Phases

### Phase 1: Foundation - COMPLETE
**Goal:** Core runtime infrastructure
**Completed:** 2026-01-15

Tasks:
- [x] Project structure
- [x] Error types
- [x] Object representation
- [x] Reference counting
- [x] Selector interning
- [x] Class registry
- [x] Arena allocator
- [x] Runtime string with SSO

### Phase 2: Dispatch - COMPLETE
**Goal:** Message passing and method resolution
**Completed:** 2026-01-15

Tasks:
- [x] Method dispatch
- [x] Method caching
- [x] Inheritance walking
- [x] Performance optimization
- [x] Method overriding
- [x] Message argument handling
- [x] Type encoding system

### Phase 3: Extensions - COMPLETE
**Goal:** Dynamic features
**Completed:** 2026-01-15

Tasks:
- [x] Categories (dynamic methods) - Phase 3.1
- [x] Protocols with inheritance and hybrid validation - Phase 3.2
- [x] Message forwarding - Phase 3.3
- [x] Method swizzling - Phase 3.4

### Phase 4: Optimization
**Goal:** Performance improvements

Tasks:
- [ ] Tagged pointers
- [ ] Inline method caches
- [ ] Compile-time registration
- [ ] SIMD optimizations

### Phase 5: Introspection
**Goal:** Reflection API

Tasks:
- [ ] Class introspection
- [ ] Method enumeration
- [ ] Protocol queries
- [ ] Runtime debugging

### Phase 6: Advanced
**Goal:** Production features

Tasks:
- [ ] Metaclasses
- [ ] Weak references
- [ ] Autorelease pools
- [ ] Thread safety

---

## Safety Guarantees

### By Layer

**Public API:**
- All operations return `Result<T>`
- No panics (except overflow detection)
- Thread-safe by default
- Lifetime management automatic

**Unsafe Internals:**
- All pointer access protected by valid ownership
- Atomic refcounting prevents races
- SAFETY comments justify each unsafe block
- No undefined behavior (verified by MIRI)
- Strict provenance compliance (verified with `-Zmiri-strict-provenance`)
- See [RFC.md](RFC.md) for MIRI validation status and test coverage

**Memory:**
- No dangling pointers
- No use-after-free
- No double-free
- No data races

---

## Performance Model

### Hot Paths

1. **Selector Lookup** (100ns target)
   - Precomputed hash (O(1))
   - Direct pointer comparison
   - Inline cache on hit

2. **Method Dispatch** (100ns target)
   - Cache hit: direct function call
   - Cache miss: walk inheritance
   - Selector lookup + method call

3. **Reference Counting** (10ns target)
   - Atomic fetch_add/sub
   - No allocations
   - Branch prediction friendly

### Memory Efficiency

- Object overhead: 32 bytes minimum
- Method cache: 8 bytes per entry
- Selector pool: 64 bytes per unique selector
- Class: ~200 bytes base

---

## Testing Strategy

### Unit Tests
Located in each module file:
- Selector interning correctness
- Refcount arithmetic
- Inheritance resolution
- Method lookup
- Arena allocation and deallocation
- Message dispatch and method calling
- String SSO and heap allocation
- Type encoding parsing
- Protocol creation and inheritance
- Protocol conformance validation

**Test Coverage:** See [RFC.md](RFC.md) for comprehensive test status
**MIRI Validation:** All tests pass with `-Zmiri-strict-provenance`

### Integration Tests
Located in /tests directory:
- Full message dispatch workflow
- Multiple class inheritance
- Protocol conformance checking
- Protocol inheritance and validation
- Error handling

### Benchmarks
Located in benches directory:
- Selector lookup latency
- Message dispatch throughput
- Allocation performance
- Cache effectiveness

### Property Tests
Using proptest:
- Refcount correctness under concurrent operations
- Selector identity consistency
- Class hierarchy invariants

---

## Future Directions

### Rust Integration
- Safe bindings for dynamic dispatch
- Macro-based method registration
- Type-safe message passing

### Platform Support
- WASM compilation
- Embedded systems
- RISC-V architecture

### Performance
- JIT compilation for hot methods
- SIMD message batching
- Adaptive cache sizing

---

**Document Status:** Living document
**Last Updated:** 2026-01-16
**Version:** See [Cargo.toml](Cargo.toml)
**Status:** See [RFC.md](RFC.md) for phase status and test coverage
**MIRI Status:** See [RFC.md](RFC.md) for MIRI validation status
