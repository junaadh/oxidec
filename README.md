# OxideX

A modern message-based dynamic language combining Swift's ergonomic syntax with Rust's safety principles, built on OxideC—a custom Objective-C-inspired runtime.

**Version**: See [Cargo.toml](Cargo.toml) for current version
**Status**: See [RFC.md](RFC.md) for roadmap status

## Overview

OxideX is a unified project with two main components:

### OxideC Runtime (COMPLETE ✓)
A high-performance dynamic object runtime in Rust providing:
- Message-based dispatch (`objc_msgSend` semantics)
- Full Objective-C-style forwarding and introspection
- Manual memory management with arena allocation
- Type-safe abstractions over zero-cost unsafe internals
- **Status**: Phase 3 complete (see [RFC.md](RFC.md) for test counts, MIRI validated)

### OxideX Language (PLANNED)
A modern programming language featuring:
- Swift-inspired syntax with clean ergonomics
- Message-based execution where `.method()` compiles to `objc_msgSend`
- Multiple execution modes (interpret, bytecode, JIT, AOT)
- Rust-inspired safety with immutability by default
- **Status**: Phase 5-13 planned, runtime optimization phases (3b-4c) must complete first

## Architecture

The project uses a Cargo workspace with clear separation of concerns:

```
oxidex/
├── crates/
│   ├── oxidec/                   # Runtime (Phase 1-3: COMPLETE)
│   ├── oxidex-syntax/            # Language syntax (Phase 5)
│   ├── oxidex-typecheck/         # Type checker (Phase 6)
│   ├── oxidex-codegen/           # Code generation (Phase 7)
│   ├── oxidex-interpreter/       # Interpreter (Phase 8)
│   ├── oxidex-bytecode/          # Bytecode VM (Phase 9)
│   ├── oxidex-jit/               # JIT compiler (Phase 10)
│   ├── oxidex-aot/               # AOT compiler (Phase 11)
│   ├── oxidex-std/               # Standard library (Phase 12)
│   └── oxidex-cli/               # CLI tools (Phase 13)
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
- [x] Manual memory management with arena allocation
- [x] Arena allocator for long-lived metadata (classes, selectors, protocols)
- [x] Thread-local arena for zero-contention allocation
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
- [x] `forwardingTargetForSelector:` support
- [x] Dynamic message forwarding
- [x] Runtime method implementation swapping
- [x] Atomic swizzle operations
- [x] Cache invalidation on swizzle

### Planned
- [ ] Automatic Reference Counting (ARC)
- [ ] Weak references
- [ ] Autorelease pools
- [ ] Protocol composition (multiple inheritance)
- [ ] Default protocol method implementations
- [ ] Protocol method signatures in type encoding
- [ ] Inline caches for hot call sites
- [ ] Polymorphic inline cache (PIC)
- [ ] Selector profiling and optimization
- [ ] C FFI bindings
- [ ] Objective-C bridging
- [ ] C++ integration layer
- [ ] Runtime inspector
- [ ] Method tracing/profiling
- [ ] Memory usage statistics
- [ ] Protocol conformance validator CLI tool

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

| Operation | Time |
|-----------|------|
| Selector intern (cache hit) | ~50ns |
| Selector intern (cache miss) | ~300ns |
| Message send (cache hit) | ~50ns |
| Message send (cache miss) | ~150ns |
| Arena allocation | ~13-15ns |
| LocalArena allocation | ~2-3ns |
| Class creation | ~1-2μs |
| Protocol creation | ~500ns |

## Documentation

- **RFC & Roadmap**: See [RFC.md](RFC.md) for development status and roadmap
- **Architecture**: See [ARCHITECTURE.md](ARCHITECTURE.md) for design decisions
- **Safety Guidelines**: See [SAFETY.md](SAFETY.md) for unsafe code patterns
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
