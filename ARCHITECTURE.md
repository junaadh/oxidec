# OxideX & OxideC: Architecture and Design

**Version:** See workspace root [Cargo.toml](Cargo.toml)
**Status:** See [RFC.md](RFC.md) for implementation status and roadmap

---

## Table of Contents

1. [System Overview](#1-system-overview)
2. [Runtime Architecture (OxideC)](#2-runtime-architecture-oxidec)
3. [Language Architecture (OxideX)](#3-language-architecture-oxidex)
4. [Workspace Structure](#4-workspace-structure)
5. [Safety Model](#5-safety-model)
6. [Performance Model](#6-performance-model)
7. [Future Directions](#7-future-directions)

---

## 1. System Overview

OxideX is a message-based dynamic language built on OxideC, a custom Objective-C-inspired runtime written in Rust. The system is organized in layers with clear safety boundaries.

### 1.1 Architectural Layers

```
┌─────────────────────────────────────────────────────────────┐
│              OxideX Language (Safe Public API)              │
│  ├── Syntax (let, fn, class, protocol, etc.)                │
│  ├── Type System (inference, checking, generics)            │
│  └── Execution (interpret, bytecode, JIT, AOT)              │
└─────────────────────────────────────────────────────────────┘
         ↓ (compiles to runtime calls)
┌─────────────────────────────────────────────────────────────┐
│           OxideC Runtime (Unsafe Internals)                 │
│  ├── Object Model (isa, refcount, allocation)               │
│  ├── Message Dispatch (cache → table → forward)             │
│  ├── Selector Interning (global, pointer-comparable)        │
│  ├── Method Lookup (inheritance, protocols)                 │
│  ├── Arena Allocator (global + scoped)                      │
│  └── Forwarding (four-stage ObjC semantics)                 │
└─────────────────────────────────────────────────────────────┘
         ↓ (manages resources)
┌─────────────────────────────────────────────────────────────┐
│          System Memory & Platform Primitives                │
│  ├── Heap allocations                                       │
│  ├── Atomic operations                                      │
│  └── Platform syscalls                                      │
└─────────────────────────────────────────────────────────────┘
```

### 1.2 Design Philosophy

**The runtime is a language feature, not an implementation detail.**

This means:
- Message forwarding is control flow (if/match-like power)
- Runtime introspection is built-in (like reflection in Java/C#)
- Dynamic behavior is first-class (like Smalltalk/Ruby)
- Performance is predictable (like C/Rust)

**Core Principles:**
1. **Correctness first**: Get semantics right before optimizing
2. **Expressiveness matters**: Enable powerful abstractions
3. **Performance predictability**: No GC pauses, no allocator surprises
4. **Safety through layers**: Safe API, audited unsafe core

---

## 2. Runtime Architecture (OxideC)

### 2.1 Module Map

```
crates/oxidec/
├── src/
│   ├── lib.rs                 # Public API entry, re-exports
│   ├── error.rs               # Error types (Result, Error enum)
│   │
│   └── runtime/
│       ├── mod.rs             # Runtime singleton, initialization
│       ├── arena.rs           # Arena allocator
│       ├── object.rs          # Object lifecycle, refcounting
│       ├── class.rs           # Class creation, inheritance
│       ├── selector.rs        # Selector interning, caching
│       ├── dispatch.rs        # Message dispatch, method lookup
│       ├── encoding.rs        # Method signature type encoding
│       ├── message.rs         # Message argument handling
│       ├── string.rs          # Runtime string (SSO + interning)
│       ├── cache.rs           # Method call caching
│       ├── category.rs        # Dynamic method addition
│       ├── protocol.rs        # Protocol conformance
│       ├── forwarding.rs      # Message forwarding
│       └── swizzling.rs       # Method replacement
│
├── benches/                   # Performance benchmarks
├── tests/                     # Integration tests
└── examples/                  # Usage examples
```

### 2.2 Object Model

Every object has:

```rust
#[repr(C)]
pub(crate) struct RawObject {
    /// Class pointer (tagged for optimizations)
    class_ptr: *const RawClass,
    
    /// Flags (unused bits for tagging)
    flags: u32,
    
    /// Atomic reference count
    refcount: AtomicU32,
    
    /// Payload (inline or heap)
    payload: *mut u8,
}
```

**Invariants:**
- `class_ptr` always points to valid `RawClass`
- `refcount` is atomic, never overflows
- `payload` allocated/deallocated with matching allocator
- Object is `Send + Sync` (atomic refcount)

**Small Object Optimization:**
- Objects < 24 bytes stored inline
- Larger objects heap-allocated via arena

### 2.3 Class Metadata

Every class contains:

```rust
pub(crate) struct ClassInner {
    /// Class name (interned string)
    name: String,
    
    /// Superclass pointer (single inheritance)
    super_class: Option<Box<Class>>,
    
    /// Method table (selector → IMP)
    methods: HashMap<String, Method>,
    
    /// Method cache (hot path optimization)
    method_cache: MethodCache,
    
    /// Protocol conformance list
    protocols: Vec<ProtocolId>,
    
    /// Instance variable layout
    ivar_layout: IvarLayout,
    
    /// Flags
    flags: u32,
}
```

### 2.4 Selector Interning

```rust
pub struct Selector {
    /// Selector name
    name: String,
    
    /// Precomputed hash (stable)
    hash: u64,
}

static SELECTOR_CACHE: OnceLock<DashMap<String, *const Selector>> 
    = OnceLock::new();
```

**Invariants:**
- All selectors globally interned (one per unique name)
- Pointer equality = selector equality (fast comparison)
- Hash precomputed, stable
- Selectors never deallocated (static cache)

### 2.5 Message Dispatch Pipeline

```
objc_msgSend(receiver, selector, args)
    ↓
1. Nil check
    if receiver == nil → return nil
    ↓
2. Extract class
    class = receiver->isa
    ↓
3. Cache lookup (HOT PATH)
    if cached → return IMP (< 20ns target)
    ↓ (miss)
4. Method table lookup
    if found → cache and return IMP (< 100ns)
    ↓ (miss)
5. Walk superclass chain
    for each superclass:
        if found → cache and return IMP
    ↓ (miss)
6. Message forwarding
    (see Forwarding Pipeline below)
    ↓ (all forwarding failed)
7. doesNotRecognizeSelector
    fatal error
```

### 2.6 Message Forwarding (Four-Stage Pipeline)

**This is the most important and complex part of the runtime.**

```
Message not found in class hierarchy
    ↓
Stage 1: forwardingTargetForSelector:
    Purpose: Fast redirect to another object
    Cost: 50-100ns
    ↓ (returned nil or same object)
Stage 2: methodSignatureForSelector:
    Purpose: Get type signature for invocation
    Cost: < 50ns (cached)
    ↓ (returned nil)
doesNotRecognizeSelector: (fatal)
    ↓ (returned signature)
Stage 3: Create NSInvocation
    Purpose: Package selector + args for manipulation
    Cost: 200-300ns (allocation overhead)
    ↓
Stage 4: forwardInvocation:
    Purpose: Rewrite invocation (change target, args, return)
    Cost: Variable (user code)
    ↓
Execute invocation
```

### 2.7 Arena Allocator

**Why Arenas?**

The runtime allocates constantly:
- Message argument frames (every send)
- Invocation objects (every forwarding call)
- Selector strings (every new method)
- Method metadata (class registration)

**Arena Strategy:**

```rust
// Global arena: long-lived runtime metadata
static GLOBAL_ARENA: OnceLock<Arena> = OnceLock::new();

pub fn allocate_global<T>(value: T) -> *mut T {
    let arena = GLOBAL_ARENA.get_or_init(Arena::new);
    arena.allocate(value)
}

// Scoped arena: transient message sends
pub struct ScopedArena {
    arena: Arena,
}

impl ScopedArena {
    pub fn allocate<T>(&self, value: T) -> *mut T {
        self.arena.allocate(value)
    }
}

impl Drop for ScopedArena {
    fn drop(&mut self) {
        self.arena.reset();
    }
}
```

**Performance:**
- Global arena: ~7-8ns per allocation
- Scoped arena: ~2-3ns per allocation
- Bulk deallocation: < 10ns (reset entire arena)

---

## 3. Language Architecture (OxideX)

### 3.1 Workspace Structure

```
crates/
├── oxidex-syntax/         # Lexer, parser, AST
├── oxidex-typecheck/      # Type inference, checking
├── oxidex-codegen/        # AST → runtime calls
├── oxidex-interpreter/    # Direct AST execution
├── oxidex-bytecode/       # Bytecode compiler + VM
├── oxidex-jit/            # JIT compiler (hot paths)
├── oxidex-aot/            # AOT compiler (native)
├── oxidex-std/            # Standard library
└── oxidex-cli/            # Command-line tools
```

### 3.2 Compilation Pipeline

```
Source Code (.ox)
    ↓
Lexer (oxidex-syntax)
    ↓
Tokens
    ↓
Parser (oxidex-syntax)
    ↓
AST
    ↓
Type Checker (oxidex-typecheck)
    ↓
Typed AST
    ↓
┌─────────────────┬────────────────┬─────────────┐
│                 │                │             │
Interpreter   Bytecode Compiler   Codegen    JIT/AOT
    │             │                │             │
    ↓             ↓                ↓             ↓
 Execute      Bytecode VM     Runtime Calls  Native Code
                              (objc_msgSend)
```

### 3.3 Execution Modes

**1. Interpreter (REPL)**
- Direct AST evaluation
- Fast startup (< 100ms)
- No compilation overhead

**2. Bytecode**
- Compile AST → bytecode
- Portable format
- Faster than interpretation

**3. JIT**
- Profile hot paths
- Compile to native
- Adaptive optimization

**4. AOT**
- Whole-program analysis
- Native code generation
- Maximum performance

---

### 3.4 Type Checker Architecture (oxidex-typecheck)

**Status**: COMPLETE ✓ (Phase 6)

The type checker implements Hindley-Milner type inference with bidirectional checking, providing complete type safety for the OxideX language.

#### Module Structure

```
crates/oxidex-typecheck/src/
├── lib.rs                    # Public API
├── types/                    # Type representation
│   ├── ty.rs                # Core Ty enum (inferred types)
│   └── display.rs           # Type pretty-printing
├── context/                  # Type checking context
│   ├── env.rs               # Type environments (scope chains)
│   ├── subst.rs             # Substitutions (union-find)
│   └── registry.rs          # Struct/enum/class/protocol registry
├── infer/                    # Type inference engine
│   ├── unify.rs             # Unification algorithm
│   └── context.rs           # Main type checking context
├── check/                    # Type checking validators
│   ├── expr.rs              # Expression type checking
│   ├── stmt.rs              # Statement type checking
│   ├── decl.rs              # Declaration type checking
│   ├── pat.rs               # Pattern type checking
│   └── ty.rs                # Type annotation conversion
└── error/                    # Error reporting
    └── mod.rs               # Error types and display
```

#### Core Features

**Type Inference:**
- Hindley-Milner (Algorithm W) with bidirectional checking
- Union-find unification with occurs check
- Let-polymorphism (generalization at let bindings)
- Generic type parameters with instantiation

**Type Checking:**
- All expression types (literals, operators, functions, methods)
- All statement types (let, mut, return, if, match, for, while)
- All declaration types (functions, structs, enums, classes, protocols)
- Pattern type checking (struct, enum, tuple, array patterns)
- Match exhaustiveness for enums
- Protocol conformance validation
- Mutability enforcement

**Validation:**
- Return type tracking
- Field access validation
- Method signature validation
- Generic constraint verification
- Scope management (proper push/pop)

#### Implementation Details

**Type Representation:**
```rust
pub enum Ty {
    // Type variables (for inference)
    TypeVar(u32),

    // Primitives (Int8-128, UInt8-128, Float32/64, Bool, String, Char, Unit, Never)
    Primitive(PrimTy),

    // Complex types
    Struct { name: Symbol, type_args: Vec<Ty> },
    Class { name: Symbol, type_args: Vec<Ty> },
    Enum { name: Symbol, type_args: Vec<Ty> },
    Tuple(Vec<Ty>),
    Array(Box<Ty>),
    Dict { key: Box<Ty>, value: Box<Ty> },
    Optional(Box<Ty>),
    Result { ok: Box<Ty>, error: Box<Ty> },

    // Functions
    Function { params: Vec<Ty>, return_type: Box<Ty>, labels: Vec<Option<Symbol>> },

    // Special
    SelfType,
    Error,
}
```

**Unification Algorithm:**
- Union-find with path compression (near O(1) operations)
- Occurs check prevents infinite types
- Structural equality for complex types
- Comprehensive error messages

**Type Environment:**
- Lexical scoping with proper push/pop
- Polymorphic types (Scheme: quantify free variables)
- Instantiation (fresh type variables for quantified vars)
- Generalization (quantify at let bindings)

#### Code Quality Metrics

**Lines of Code**: ~6,500
**Test Coverage**: 80 unit tests (all passing)
**MIRI Validation**: Clean with `-Zmiri-strict-provenance`
**Performance**: Target > 50k LOC/sec (not yet benchmarked)

#### Production Readiness

The type checker is **PRODUCTION READY** for:
- All expression/statement/declaration types
- Generic functions and types
- Pattern matching with exhaustiveness
- Protocol conformance validation
- Method calls and field access

**NOT yet implemented** (deferred to future phases):
- String interpolation (AST support needed)
- Multi-segment path expressions (module system needed)
- Advanced class features (inheritance validation)
- Error recovery (nice-to-have)
- Duplicate detection (language design question)

---

## 4. Workspace Structure

### 4.1 Cargo Workspace

```toml
[workspace]
resolver = "3"
members = [
    "crates/oxidec",
    "crates/oxidex-syntax",
    "crates/oxidex-typecheck",
    "crates/oxidex-codegen",
    "crates/oxidex-interpreter",
    "crates/oxidex-bytecode",
    "crates/oxidex-jit",
    "crates/oxidex-aot",
    "crates/oxidex-std",
    "crates/oxidex-cli",
]
```

### 4.2 Dependency Graph

```
oxidex-cli
    ├── oxidex-interpreter
    │   ├── oxidex-typecheck
    │   │   └── oxidex-syntax
    │   └── oxidec
    ├── oxidex-bytecode
    │   └── oxidex-syntax
    ├── oxidex-jit
    │   ├── oxidex-bytecode
    │   └── oxidec
    └── oxidex-aot
        ├── oxidex-codegen
        │   ├── oxidex-typecheck
        │   └── oxidec
        └── oxidec

oxidex-std
    └── oxidec
```

---

## 5. Safety Model

### 5.1 Layered Safety

**Layer 1: Language (Safe)**
- Type system prevents common bugs
- Pattern matching ensures exhaustiveness
- Immutability by default
- Explicit mutability (`let mut`)

**Layer 2: Runtime API (Safe)**
- All public functions return `Result<T>`
- Lifetime management automatic
- Thread-safe by default
- No panics (except overflow)

**Layer 3: Runtime Core (Unsafe)**
- Manual memory management
- Pointer arithmetic
- Atomic operations
- SAFETY comments justify everything

### 5.2 MIRI Validation

All code must pass MIRI with strict provenance:

```bash
MIRIFLAGS="-Zmiri-strict-provenance -Zmiri-ignore-leaks" \
    cargo +nightly miri test
```

**Current Status:**
- See [RFC.md](RFC.md) for MIRI validation status
- No UB detected
- Strict provenance compliant

---

## 6. Performance Model

### 6.1 Hot Paths

**Selector Lookup:**
- Precomputed hash (O(1))
- Pointer comparison (inline)
- Cache hit: < 5ns (target)

**Method Dispatch:**
- Cache hit: < 20ns (target)
- Cache miss: < 100ns
- Forwarding (fast): < 100ns
- Forwarding (full): < 500ns

**Reference Counting:**
- Atomic fetch_add: ~10ns
- No allocations
- Branch prediction friendly

**Arena Allocation:**
- Global: ~7-8ns
- Scoped: ~2-3ns
- Bulk deallocation: < 10ns

### 6.2 Performance Targets

| Operation | Target | Current | Priority |
|-----------|--------|---------|----------|
| Selector lookup (hit) | < 5ns | Regressed | HIGH |
| Message dispatch (cached) | < 20ns | ~30ns | HIGH |
| Message dispatch (uncached) | < 100ns | ~80ns | OK |
| Forwarding (fast) | < 100ns | TBD | MEDIUM |
| Forwarding (full) | < 500ns | TBD | MEDIUM |
| Arena allocation (global) | < 8ns | ~7-8ns | OK |
| Arena allocation (scoped) | < 3ns | ~2-3ns | Good |

---

## 7. Future Directions

### 7.1 Runtime Optimizations

**Tagged Pointers:**
- Encode small values inline (int, bool, nil)
- Save allocation and indirection

**Inline Method Caches:**
- Per-call-site caching (faster than per-class)
- Requires JIT or AOT compilation

**Adaptive Caching:**
- Grow cache size based on workload
- Evict cold entries
- Profile-guided optimization

### 7.2 Language Features

**Async/Await:**
- Cooperative concurrency
- Runtime integration
- Message sending across await points

**Macros:**
- Hygiene system
- Syntax extensions
- Procedural macros

### 7.3 Platform Support

**WebAssembly:**
- Compile to WASM
- Runtime in browser
- Sandboxed execution

**Embedded Systems:**
- No-std support
- Minimal runtime footprint
- Static allocation mode

---

## 8. Documentation Requirements

### 8.1 NO EMOJIS

**Use text markers instead:**
- [OK] / [DONE] for completed items
- [WARNING] / [!] for warnings
- [ERROR] / [X] for errors
- [TODO] / [PENDING] / [ ] for pending items

### 8.2 Code Documentation

**Public APIs:**
- Explain what, why, when
- Include examples
- Document error conditions
- Link to related functions

**SAFETY Comments:**
- Explain why unsafe is necessary
- Document pointer lifetimes
- Prove validity before dereference
- Reference relevant invariants

---

## 9. Summary

**OxideX is:**
- A message-based dynamic language
- Built on a custom runtime (OxideC)
- Focused on expressiveness and performance
- Safety through layered design
- Multiple execution modes (interpret, bytecode, JIT, AOT)

**Current Status:**
- Runtime Phase 1-3: COMPLETE
- Runtime Phase 3b-3d: COMPLETE (selector optimization and sharding)
- Runtime Phase 4a.1: COMPLETE (invocation objects)
- Runtime Phase 4a.2-4c: PLANNED
- Language Phase 5-13: PLANNED
- See [RFC.md](RFC.md) for test counts and validation status
- MIRI validated (no UB)

**Next Steps:**
1. Complete four-stage forwarding pipeline (Phase 4a.2-4a.5)
2. Add runtime introspection APIs (Phase 4b)
3. Formalize arena lifecycle management (Phase 4c)

**This is a multi-year project. The foundation is solid. The vision is clear.**

---

**Primary Author**: Junaadh
**Status:** Runtime Complete, Language Planned

