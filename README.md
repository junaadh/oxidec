# OxideX

A modern message-based dynamic language combining Swift's ergonomic syntax with Rust's safety principles, built on OxideC—a custom Objective-C-inspired runtime.

**Version**: See [Cargo.toml](Cargo.toml) for current version
**Status**: Runtime Phase 4c Complete ✓ | Language Phase 5a Complete ✓ | Language Phase 5b In Progress | Language Phase 6 Planned

**Runtime Achievements:**
- 639 tests passing (639/639 with 6 ignored)
- MIRI validated with strict provenance
- Zero heap allocations in lexer hot paths
- 5-6x memory reduction for token storage
- String interning with Symbol(u32) IDs
- Full Stacked Borrows compliance

**Language Frontend Achievements (Phase 5b):**
- Complete lexer with 24 keywords and all token types
- Full parser with 231 tests passing (208 unit + 23 integration)
- AST for all declaration types
- Enum support with `case` keyword
- Enum methods (directly in body and impl blocks)
- Method visibility resolution (most restrictive of parent and method)
- Labeled parameters (Swift-style)
- `self` and `Self` keywords
- `init` keyword for initializers
- `mut fn` for mutable methods
- `static fn` for static methods
- Pretty-printer for all AST nodes
- 15 example programs demonstrating language features
- Parser performance benchmarks
- Comprehensive integration tests
- Rich error reporting with source highlighting
- MIRI validated with strict provenance
- Zero heap allocations in lexer hot paths
- 5-6x memory reduction for token storage
- Keywords properly separated into syntax layer
- Generic type parsing with angle bracket support

## Overview

OxideX is a unified project with two main components:

### OxideC Runtime (COMPLETE ✓)
A high-performance dynamic object runtime in Rust providing:
- Message-based dispatch (`objc_msgSend` semantics)
- Full Objective-C-style four-stage message forwarding pipeline
- Invocation objects for message manipulation
- Manual memory management with optimized arena allocation
- Type-safe abstractions over zero-cost unsafe internals
- **Status**: Phase 4c complete (see [RFC.md](RFC.md) for test counts, MIRI validated)
  - 452 tests passing (runtime only)
  - MIRI validated with strict provenance (280 tests)
  - Global arena: 3.98ns (47.6% improvement over baseline)

### OxideX Language (Phase 5b In Progress)
A modern programming language featuring:
- Swift-inspired syntax with clean ergonomics
- Message-based execution where `.method()` compiles to `objc_msgSend`
- Multiple execution modes (interpret, bytecode, JIT, AOT)
- Rust-inspired safety with immutability by default
- **Status**: Phase 5a complete, Phase 5b in progress, Phase 6 planned
  - Phase 5a: Memory infrastructure (oxidex-mem crate, string interning)
  - Phase 5b: Language frontend (lexer, parser, AST, diagnostics, pretty-printer)
  - 231 tests passing (208 unit + 23 integration)
  - 15 example programs covering all language features
  - Parser performance benchmarks
  - Integration tests for parsing, roundtrip, diagnostics
  - MIRI validated with strict provenance
  - Zero heap allocations in lexer hot paths
  - 5-6x memory reduction for token storage
  - Keywords properly separated into syntax layer
  - Generic type parsing with angle bracket support
  - Rich error reporting with Rust-style diagnostics
  - Enum support with `case` keyword and enum methods
  - Method visibility resolution (Rust-style, most restrictive wins)

## Architecture

The project uses a Cargo workspace with clear separation of concerns:

```
oxidex/
├── crates/
│   ├── oxidec/                   # Runtime (Phase 1-4c: COMPLETE)
│   ├── oxidex-mem/               # Memory infrastructure (Phase 5a: COMPLETE)
│   ├── oxidex-syntax/            # Language syntax (Phase 5b: IN PROGRESS)
│   ├── oxidex-typecheck/         # Type checker (Phase 6: PLANNED)
│   ├── oxidex-codegen/           # Code generation (Phase 7: PLANNED)
│   ├── oxidex-interpreter/       # Interpreter (Phase 8: PLANNED)
│   ├── oxidex-bytecode/          # Bytecode VM (Phase 9: PLANNED)
│   ├── oxidex-jit/               # JIT compiler (Phase 10: PLANNED)
│   ├── oxidex-aot/               # AOT compiler (Phase 11: PLANNED)
│   ├── oxidex-std/               # Standard library (Phase 12: PLANNED)
│   └── oxidex-cli/               # CLI tools (Phase 13: PLANNED)
└── docs/
    ├── language/                 # Language specification
    ├── runtime/                  # Runtime documentation
    └── examples/                 # Code examples
```

### Runtime Architecture

The OxideC runtime is built on a layered architecture:

- **Public API Layer**: Type-safe, validated abstractions with zero overhead
- **Runtime Layer**: Unsafe internals with comprehensive safety documentation
- **Memory Layer**: Arena allocators for high-performance metadata allocation

## Features

### Supported
- [x] Dynamic message dispatch with late binding
- [x] Method caching for fast lookup (~50ns cache hit, ~150ns cache miss)
- [x] Selector interning with global hash table (256 buckets)
- [x] Single inheritance with cycle detection
- [x] Automatic reference counting with overflow detection
- [x] Manual memory management with optimized arena allocation
- [x] Global arena for long-lived metadata (classes, selectors, protocols)
- [x] Thread-local arena for zero-contention allocation
- [x] Scoped arena with RAII guards (automatic cleanup)
- [x] Thread-local arena pools for fast temporary allocation
- [x] Arena reset capability for memory reuse
- [x] Debug-mode leak detection with zero release overhead
- [x] Lock-free bump pointer allocation
- [x] Stable pointers (never moves or reallocates)
- [x] Strict provenance compliance (MIRI validated)
- [x] Classes with single inheritance
- [x] Categories (dynamic method addition to existing classes)
- [x] Protocols (interface definitions with hybrid validation)
- [x] Declarative protocol conformance (default, like Objective-C)
- [x] Optional runtime protocol validation API
- [x] Protocol inheritance (protocols can extend other protocols)
- [x] Transitive protocol conformance through inheritance
- [x] Message send with variable arguments
- [x] Argument marshalling with type encoding
- [x] Return value handling with unaligned access support
- [x] Method resolution order: local → categories → superclass
- [x] Method cache invalidation on dynamic updates
- [x] Thread-safe class/selector/protocol creation
- [x] Concurrent method registration with RwLock protection
- [x] Atomic reference counting
- [x] Lock-free arena allocation (global arena)
- [x] Thread-local arena for single-threaded contexts
- [x] MIRI validated with `-Zmiri-strict-provenance` (all tests pass)
- [x] Stacked Borrows compliance
- [x] Comprehensive SAFETY comments on all unsafe code
- [x] Arena deallocation safety (Box ownership tracking)
- [x] Optional protocol conformance validation
- [x] Objective-C type encoding parser
- [x] Type validation and verification
- [x] Size calculation for encoded types
- [x] Forwarding target for unhandled messages
- [x] `forwardingTargetForSelector:` support (Stage 1)
- [x] `methodSignatureForSelector:` support (Stage 2)
- [x] `forwardInvocation:` support (Stage 3)
- [x] `doesNotRecognizeSelector:` support (Stage 4)
- [x] Four-stage message forwarding pipeline
- [x] Invocation objects for message manipulation
- [x] Type-erased argument storage
- [x] Message rewriting and retargeting
- [x] Forwarding loop detection (max depth: 32)
- [x] Signature caching for performance
- [x] Per-class and global forwarding hooks
- [x] Forwarding event callbacks for diagnostics
- [x] Dynamic message forwarding
- [x] Runtime method implementation swapping
- [x] Atomic swizzle operations
- [x] Cache invalidation on swizzle
- [x] Runtime introspection APIs (class hierarchy, method lookup, protocol conformance)
- [x] Arena lifecycle management (ScopedArena, thread-local pools, leak detection)
- [x] Memory infrastructure crate (oxidex-mem)
- [x] Symbol type for interned string IDs (Symbol(u32))
- [x] String interning with bidirectional mapping
- [x] Pre-interned keywords (19 OxideX keywords, IDs 0-18)
- [x] Arena-allocated string storage (2-3ns allocations)
- [x] Lexer token migration from String to Symbol
- [x] Zero heap allocations in lexer hot paths
- [x] 5-6x memory reduction for token storage
- [x] MIRI validated memory infrastructure (157 tests)

## Testing

```bash
# Build entire workspace
cargo build --workspace

# Run all tests
cargo test --workspace

# Run tests for specific crate
cargo test -p oxidec

# Run benchmarks
cargo bench -p oxidec

# MIRI validation
MIRIFLAGS="-Zmiri-strict-provenance -Zmiri-ignore-leaks" \
    cargo +nightly miri test --workspace
```

See [RFC.md](RFC.md) for comprehensive test coverage and validation status.

## Safety Guarantees

OxideC provides memory safety through:

1. **Public API Safety**: All public operations are safe and validated
2. **Unsafe Internals**: Extensively documented with SAFETY comments
3. **MIRI Validated**: All unsafe code proven sound with strict provenance
4. **Arena Safety**: Stable pointers with Box ownership tracking
5. **Thread Safety**: RwLock protection for all shared mutable state

## Performance

Current performance characteristics (Apple M1):

| Operation | Time | Notes |
|-----------|------|-------|
| Selector intern (cache hit) | ~50ns | |
| Selector intern (cache miss) | ~300ns | |
| Message send (cache hit) | ~50ns | |
| Message send (cache miss) | ~150ns | |
| Forwarding Stage 1 (fast redirect) | < 100ns | |
| Forwarding Stage 2 (signature, cached) | < 50ns | |
| Forwarding Stage 3 (full invocation) | < 500ns | |
| Arena allocation (global) | 3.98ns | 47.6% improvement |
| LocalArena allocation | 2.65ns | Thread-local, zero contention |
| LocalArena string allocation | 2-3ns | oxidex-mem for compiler frontend |
| Thread-local arena pool | ~2.65ns | Fast temporary allocation |
| String intern (cache hit) | ~50ns | O(1) hash lookup |
| String intern (new string) | ~100-200ns | O(n) copy + hash insert |
| Symbol resolve | < 5ns | O(1) array indexing |
| Lexer token (identifier) | < 100ns | Zero heap allocations |
| Lexer token (keyword) | < 50ns | Pre-interned (ID 0-18) |
| Class creation | ~1-2μs | |
| Protocol creation | ~500ns | |

## Documentation

- **RFC & Roadmap**: See [RFC.md](RFC.md) for development status and roadmap
- **Architecture**: See [ARCHITECTURE.md](ARCHITECTURE.md) for design decisions
- **Safety Guidelines**: See [SAFETY.md](SAFETY.md) for unsafe code patterns
- **Arena Best Practices**: See [docs/arena_best_practices.md](docs/arena_best_practices.md) for leak prevention
- **Phase 4c Summary**: See [docs/phase_4c_summary.md](docs/phase_4c_summary.md) for arena optimization details
- **Memory Infrastructure**: See [crates/oxidex-mem/ARCHITECTURE.md](crates/oxidex-mem/ARCHITECTURE.md) for string interning design
- **Changelog**: See [CHANGELOG.md](CHANGELOG.md) for release history

## Example

```rust
use oxidec::{Class, Method, Object, Protocol, RuntimeString, Selector};
use oxidec::runtime::get_global_arena;
use std::str::FromStr;

// Define a protocol
let protocol = Protocol::new("Copyable", None).unwrap();
let copy_sel = Selector::from_str("copy").unwrap();
protocol.add_required(copy_sel.clone(), "@@:", get_global_arena()).unwrap();

// Create a class
let class = Class::new_root("MyObject").unwrap();
class.add_protocol(&protocol).unwrap();

// Add a method
unsafe extern "C" fn copy_impl(
    _self: oxidec::runtime::object::ObjectPtr,
    _cmd: oxidec::runtime::selector::SelectorHandle,
    _args: *const *mut u8,
    _ret: *mut u8,
) {
    // Method implementation
}

let method = Method {
    selector: copy_sel,
    imp: copy_impl,
    types: RuntimeString::new("@@:", get_global_arena()),
};
class.add_method(method).unwrap();

// Validate protocol conformance (optional)
class.validate_protocol_conformance(&protocol).unwrap();

// Create an instance and send messages
let obj = Object::new(&class).unwrap();
obj.send_message("copy", &[]);
```

## License

MIT
