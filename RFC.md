# RFC: OxideX Language & OxideC Runtime Specification

**Author:** Junaadh
**Status:** Runtime Phase 3 Complete, Language Phase 4-12 Planned
**Date:** 2026-01-14
**Version:** See workspace root [Cargo.toml](Cargo.toml)
**Last Updated:** 2026-01-16

---

## Abstract

This RFC defines OxideX, a modern programming language with Swift-inspired syntax and Rust-inspired safety, built on OxideC—a custom Objective-C-inspired runtime written in Rust. The runtime implements message-based dispatch with full forwarding semantics, dynamic typing at call boundaries, and explicit lifetime management. The language compiles to runtime calls and supports multiple execution modes: interpretation, bytecode, JIT, and AOT compilation.

---

## Table of Contents

1. [Project Overview](#1-project-overview)
2. [Runtime Specification (OxideC)](#2-runtime-specification-oxidec)
3. [Language Specification (OxideX)](#3-language-specification-oxidex)
4. [Development Phases](#4-development-phases)
5. [Performance Targets](#5-performance-targets)
6. [Testing Strategy](#6-testing-strategy)
7. [Open Questions](#7-open-questions)

---

## 1. Project Overview

### 1.1 Vision

OxideX is **not** a general-purpose language. It is a **message-based dynamic language** where:

- Method calls are message sends with dynamic dispatch
- Forwarding is first-class control flow (proxies, RPC, mocking)
- Runtime introspection is built-in and powerful
- Performance is predictable (nanosecond-level dispatch)
- Safety comes from layered design (safe API, unsafe core)

### 1.2 Target Use Cases

- **Dynamic systems**: Plugin architectures, DSLs, scripting
- **RPC and distributed objects**: Network-transparent messaging
- **Metaprogramming**: Runtime code generation, reflection
- **Testing and mocking**: Dynamic test doubles, instrumentation
- **Language research**: Exploring dispatch strategies

### 1.3 Design Goals

1. **Message-centric execution**: Every method call is `objc_msgSend`
2. **Runtime as feature**: Forwarding, introspection, swizzling built-in
3. **Safety through layers**: Safe public API, audited unsafe core
4. **Performance predictability**: No GC, arena allocation, caching
5. **Multiple execution modes**: Interpret, bytecode, JIT, AOT

---

## 2. Runtime Specification (OxideC)

### 2.1 Core Object Model

Every object has:
- **isa pointer** → class metadata (tagged for optimizations)
- **Reference count** (atomic, long-lived objects)
- **Inline storage** (small object optimization, 24 bytes)
- **Heap data** (fallback for large objects)

Every class contains:
- **Method table** (selector → IMP, HashMap)
- **Method cache** (hot path optimization, 85-95% hit rate)
- **Superclass pointer** (single inheritance)
- **Protocol list** (conformance metadata)
- **Instance variable layout**

Selectors are:
- **Globally interned** (pointer equality = selector equality)
- **Precomputed hash** (stable, cached)
- **Inline for short names** (< 24 bytes)
- **Heap for long names** (arena-allocated)

### 2.2 Message Dispatch Pipeline

```
objc_msgSend(receiver, selector, args)
    ↓
1. Nil check → return nil
    ↓
2. Extract class from isa
    ↓
3. Method cache lookup (hot path, < 20ns)
    ↓ (miss)
4. Method table lookup (< 100ns)
    ↓ (miss)
5. Walk superclass chain
    ↓ (miss)
6. Message forwarding (multi-stage)
    ↓
7. doesNotRecognizeSelector (fatal)
```

**Performance targets:**
- Cached send: 15-30ns
- Uncached send: < 100ns
- Forwarding (fast): 50-100ns
- Forwarding (full): 200-500ns

### 2.3 Message Forwarding (First-Class Feature)

Forwarding is **not** an edge case. It enables proxies, RPC, adapters, mocking, lazy loading.

**Four-stage pipeline (Objective-C semantics):**

1. **forwardingTargetForSelector:**
   - Fast redirect to another object
   - Cost: 50-100ns
   - Use: Simple delegation

2. **methodSignatureForSelector:**
   - Return type signature for selector
   - Required for invocation creation
   - Cost: < 50ns (cached)

3. **forwardInvocation:**
   - Full invocation object manipulation
   - Rewrite args, change target, modify return
   - Cost: 200-500ns
   - Use: Complex proxies, RPC

4. **doesNotRecognizeSelector:**
   - Fatal error handler
   - Last resort before crash
   - Use: Debugging, error reporting

### 2.4 Arena Allocation Strategy

**Why arena allocation?**

The runtime allocates constantly:
- Message argument frames (every send)
- Invocation objects (every forwarding call)
- Selector strings (every new method)
- Method metadata (class registration)

General-purpose allocators are too slow (~50-100ns).

**Arena strategy:**

1. **Global arena**: Long-lived metadata (classes, protocols)
   - Never deallocated
   - Bump allocation
   - Fast: ~7-8ns

2. **Scoped arenas**: Transient data (message frames)
   - Dropped at scope exit
   - Bulk deallocation
   - Fast: < 3ns

### 2.5 Selector Interning and String Management

**Why selectors are special:**

Selectors are not normal strings. They are:
- Method identifiers (every message send)
- Cache keys (every dispatch lookup)
- Interning targets (must be globally unique)
- Reflection keys (every runtime query)

**Implementation strategy:**

- **Inline storage**: Short selectors (< 24 bytes)
- **Heap storage**: Long selectors (arena-allocated)
- **Global intern table**: Pointer equality for all
- **Precomputed hash**: Stable, cached

**Performance:**
- Inline ops: ~2-3ns (hardware floor)
- Heap ops: ~7-8ns (allocator-dependent)
- Interning hits: < 5ns (CRITICAL)

### 2.6 Runtime Phase Status

| Phase | Status | Completion | Tests |
|-------|--------|------------|-------|
| Phase 1: Foundation | COMPLETE | 2026-01-15 | 42 unit |
| Phase 2: Dispatch | COMPLETE | 2026-01-15 | 61 unit |
| Phase 3: Extensions | COMPLETE | 2026-01-15 | 45 unit + 16 integration |
| **Total** | **COMPLETE** | **2026-01-15** | **148 unit + 16 integration = 164** |

**MIRI Validation:** All tests pass with `-Zmiri-strict-provenance -Zmiri-ignore-leaks`

---

## 3. Language Specification (OxideX)

### 3.1 Syntax Overview

OxideX combines:
- Swift's ergonomic syntax (clean, modern, familiar)
- Rust's safety principles (immutable by default, explicit mutability)
- Objective-C's dynamic features (message sending, forwarding, runtime)

**Key syntactic features:**
- Immutable by default: `let` vs `let mut`
- Type inference with shorthand: `.variant`, `.method()`
- Pattern matching: `if let some`, `guard let`, `match`
- Protocols and generics
- Compile-time evaluation: `comptime`
- Derivation macros: `@derive(Eq, Hash)`

### 3.2 Variables and Mutability

```oxidex
// Immutable by default
let name = "Alice"              // type inferred as String
let age: Int = 30               // explicit type

// Mutable variables
let mut counter = 0
counter += 1

// Error: cannot reassign immutable
// name = "Bob"
```

### 3.3 Type System

**Core types:**
- `Int`, `UInt`, `Float`, `Double`, `Bool`, `String`
- `Option<T>`, `Result<T, E>`
- `Array<T>`, `Dict<K, V>`, `Set<T>`

**Type inference:**
```oxidex
let x = 42                    // Int inferred
let result = .ok("success")   // Result<String, _> inferred from context
```

### 3.4 Enums (Tagged Unions)

```oxidex
enum Option<T> {
    case some(T)
    case none
}

enum Result<T, E> {
    case ok(T)
    case err(E)
}

// Usage with dot notation
let opt: Option<Int> = .some(42)
```

### 3.5 Pattern Matching

```oxidex
// if let some (Option unwrapping)
if let some(value) = optionalValue {
    print("Got: \(value)")
}

// guard let (early return)
fn processValue(_ opt: Option<Int>) -> Int {
    guard let some(value) = opt else {
        return 0
    }
    return value * 2
}

// match expression
let message = match status {
    .idle => "Not started",
    .running(p) => "Running at \(Int(p * 100))%",
    .completed(r) => "Done: \(r)",
    .failed(e) => "Error: \(e)"
}
```

### 3.6 Functions

```oxidex
// Basic function
fn greet(name: String) -> String {
    return "Hello, \(name)"
}

// Underscore parameter (no label)
fn greet(_ name: String) -> String {
    return "Hello, \(name)"
}

// External and internal names
fn greet(with greeting: String, to name: String) {
    print("\(greeting), \(name)")
}

// Inline hint
@inline
fn fastCompute(x: Int) -> Int {
    return x * x
}
```

### 3.7 Classes

```oxidex
class Animal {
    let name: String
    let mut age: Int
    
    init(name: String, age: Int) {
        self.name = name
        self.age = age
    }
    
    fn makeSound() -> String {
        return "Some sound"
    }
}

// Inheritance
class Dog: Animal {
    let breed: String
    
    init(name: String, age: Int, breed: String) {
        self.breed = breed
        super.init(name: name, age: age)
    }
    
    override fn makeSound() -> String {
        return "Woof!"
    }
}

// Usage
let dog = Dog(name: "Rex", age: 3, breed: "Labrador")
dog.makeSound()  // Compiles to objc_msgSend
```

### 3.8 Protocols

```oxidex
protocol Drawable {
    fn draw() -> Void
    fn area() -> Double
}

// Implementation with 'impl'
struct Circle {
    let radius: Double
}

impl Drawable for Circle {
    fn draw() {
        print("Drawing circle")
    }
    
    fn area() -> Double {
        return 3.14159 * radius * radius
    }
}
```

### 3.9 Generics

```oxidex
struct Box<T> {
    let value: T
    
    fn map<U>(f: fn(T) -> U) -> Box<U> {
        return Box(value: f(value))
    }
}

// Constrained generics
fn findMax<T>(items: [T]) -> Option<T> where T: Comparable {
    // Implementation
}
```

### 3.10 Compile-Time Evaluation

```oxidex
fn comptime getStorageType(bits: Int) -> Type {
    if bits <= 8 {
        return UInt8
    } else if bits <= 16 {
        return UInt16
    } else {
        return UInt32
    }
}

struct BitField<comptime N: Int> {
    let data: getStorageType(N)
}
```

### 3.11 Derivation Macros

```oxidex
@derive(Eq, Hash, Copy, Debug)
struct Point {
    let x: Int
    let y: Int
}
```

---

## 4. Development Phases

### Phase 1-3: Runtime (COMPLETE ✓)

**Goal:** Core OxideC runtime infrastructure

**Status:** COMPLETE
- 148 unit tests + 16 integration tests
- MIRI validated with strict provenance
- All core features working

---

### Phase 4: Language Frontend (Planned)

**Crate:** `oxidex-syntax`

**Duration:** 7-10 weeks

**Scope:**
- Lexer (tokenization)
- Parser (AST construction)
- Error recovery
- Source location tracking

---

### Phase 5: Type Checker (Planned)

**Crate:** `oxidex-typecheck`

**Duration:** 8-11 weeks

**Scope:**
- Type representation
- Hindley-Milner inference
- Constraint solving
- Protocol conformance

---

### Phase 6: Code Generation (Planned)

**Crate:** `oxidex-codegen`

**Duration:** 5-7 weeks

**Scope:**
- AST lowering
- Optimization passes
- Runtime call generation

---

### Phase 7: Interpreter (Planned)

**Crate:** `oxidex-interpreter`

**Duration:** 5-6 weeks

**Scope:**
- AST evaluation
- Environment management
- REPL implementation

---

### Phase 8: Bytecode (Planned)

**Crate:** `oxidex-bytecode`

**Duration:** 9-11 weeks

**Scope:**
- Instruction set design
- Bytecode compiler
- Virtual machine

---

### Phase 9: JIT (Planned)

**Crate:** `oxidex-jit`

**Duration:** 6-9 weeks

**Scope:**
- Hot path detection
- JIT compilation
- Code cache management

---

### Phase 10: AOT (Planned)

**Crate:** `oxidex-aot`

**Duration:** 7-10 weeks

**Scope:**
- Whole-program analysis
- Native code generation
- Linker integration

---

### Phase 11: Standard Library (Planned)

**Crate:** `oxidex-std`

**Duration:** 12-16 weeks

**Scope:**
- Core types
- Collections
- I/O operations
- Concurrency primitives

---

### Phase 12: Developer Tooling (Planned)

**Crate:** `oxidex-cli`

**Duration:** 13-18 weeks

**Scope:**
- CLI interface
- Language Server (LSP)
- Package manager
- Documentation generator

---

## 5. Performance Targets

### 5.1 Runtime Performance

| Operation | Target | Current | Status |
|-----------|--------|---------|--------|
| Message dispatch (cached) | < 20ns | ~30ns | Needs optimization |
| Message dispatch (uncached) | < 100ns | ~80ns | OK |
| Forwarding (fast path) | < 100ns | TBD | Not measured |
| Forwarding (full invocation) | < 500ns | TBD | Not measured |
| Arena allocation (global) | < 8ns | ~7-8ns | OK |
| Arena allocation (scoped) | < 3ns | ~2-3ns | Good |
| Selector interning (hit) | < 5ns | Regression | NEEDS FIX |

### 5.2 Language Performance

| Phase | Target | Status |
|-------|--------|--------|
| Parsing | > 100k LOC/sec | Not implemented |
| Type checking | > 50k LOC/sec | Not implemented |
| Bytecode compilation | < 1ms per 1k LOC | Not implemented |
| JIT compilation | < 10ms per hot function | Not implemented |
| AOT compilation | Comparable to Rust | Not implemented |

---

## 6. Testing Strategy

### 6.1 Runtime Testing (OxideC)

**Current Status:**
- Unit tests: 148 passing
- Integration tests: 16 passing
- Doctests: 74 total (68 passing, 6 ignored)
- **Total: 238 tests**

**MIRI Validation:**
- All tests pass with `-Zmiri-strict-provenance -Zmiri-ignore-leaks`
- No undefined behavior detected
- Pointer provenance correct
- Alignment validated

### 6.2 Language Testing (OxideX)

**Planned:**
- Parser tests (Phase 4)
- Type checker tests (Phase 5)
- Code generation tests (Phase 6)
- Interpreter tests (Phase 7)
- Bytecode tests (Phase 8)
- JIT tests (Phase 9)
- AOT tests (Phase 10)
- Standard library tests (Phase 11)

---

## 7. Open Questions (ANSWERED)

### 7.1 Runtime Questions

**1. Metaclass Implementation**

**Answer:** Full Objective-C style metaclasses.

**Reason:** Classes must be first-class objects; metaclasses avoid special-casing class methods and enable uniform dispatch and introspection with minimal runtime complexity.

---

**2. Weak References**

**Answer:** Exclude by default; design-compatible.

**Reason:** Weak refs add side tables and atomic overhead; benefits only appear in complex object graphs and frameworks.

---

**3. Autorelease Pools**

**Answer:** Scoped, explicit autorelease pools.

**Reason:** Needed for ergonomic APIs and FFI boundaries; integrates cleanly with arena/scoped allocation without global magic.

---

**4. Thread Safety**

**Answer:** Runtime is thread-safe; synchronization is explicit at the language level.

**Reason:** Allows safe multithreaded use while preserving single-threaded performance on the dispatch fast path.

---

### 7.2 Language Questions

**1. Error Handling**

**Answer:** Result types as default, exceptions as secondary.

**Reason:** Result types are predictable and optimizable; exceptions are useful for FFI and non-local failure paths.

---

**2. Concurrency Model**

**Answer:** Threads plus async/await.

**Reason:** Covers most use cases with minimal runtime semantics; avoids early commitment to heavy actor models.

---

**3. Module System**

**Answer:** Explicit modules with explicit imports.

**Reason:** Enables deterministic builds, clear namespaces, and strong tooling support.

---

**4. Macro System**

**Answer:** Hygienic procedural macros only.

**Reason:** Preserves parser stability, tooling, and semantic clarity while enabling powerful compile-time code generation.

---

**5. FFI Design**

**Answer:** C ABI as the foundation.

**Reason:** Universally compatible; enables straightforward interop with Rust, Swift, and system libraries.

---

### 7.3 Ecosystem Questions

**1. Package Registry**

**Answer:** No central registry initially.

**Reason:** Git-based dependencies solve distribution without governance or infrastructure overhead.

---

**2. Tooling Integration**

**Answer:** CLI-first tooling.

**Reason:** Ensures automation, CI compatibility, and editor-agnostic workflows.

---

**3. Documentation**

**Answer:** Architecture-first documentation.

**Reason:** Clear mental model reduces misuse and accelerates ecosystem growth.

---

## 8. Summary

**Current Status:**
- Runtime Phase 1-3: COMPLETE (148 unit + 16 integration = 164 tests)
- Language Phase 4-12: PLANNED

**Immediate Priorities:**
1. Fix selector interning cache regressions (HIGH)
2. Optimize heap hash performance (HIGH)
3. Begin language frontend (Phase 4)

**This is a multi-year project. The foundation is solid. The vision is clear. The hard work is ahead.**

---

**Author:** Junaadh
**Last Updated:** 2026-01-16
**Status:** Alpha 0.3 (Runtime Complete, Language Planned)

