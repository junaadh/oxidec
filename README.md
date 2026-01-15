# OxideC

A modern Objective-C inspired dynamic object runtime in Rust, providing type-safe abstractions over zero-cost unsafe internals.

## Overview

OxideC implements a high-performance dynamic object runtime with manual memory management and compile-time safety guarantees. It combines the flexibility of Objective-C's message passing system with Rust's memory safety model.

**Version**: 0.2.0-alpha

## Architecture

The runtime is built on a layered architecture:

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
- [x] MIRI validated with `-Zmiri-strict-provenance` (all 140 tests pass)
- [x] Stacked Borrows compliance
- [x] Comprehensive SAFETY comments on all unsafe code
- [x] Arena deallocation safety (Box ownership tracking)
- [x] Optional protocol conformance validation
- [x] Objective-C type encoding parser
- [x] Type validation and verification
- [x] Size calculation for encoded types

### Planned
- [ ] Automatic Reference Counting (ARC)
- [ ] Weak references
- [ ] Autorelease pools
- [ ] Forwarding target for unhandled messages
- [ ] `forwardingTargetForSelector:` support
- [ ] Dynamic message forwarding
- [ ] Runtime method implementation swapping
- [ ] Atomic swizzle operations
- [ ] Cache invalidation on swizzle
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

- **Unit Tests**: 140 tests (all passing)
- **Doc Tests**: 69 tests (all passing)
- **MIRI Validation**: All tests pass with `-Zmiri-strict-provenance -Zmiri-ignore-leaks`

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

- **Architecture**: See [ARCHITECTURE.md](ARCHITECTURE.md) for design decisions
- **Safety Guidelines**: See [SAFETY.md](SAFETY.md) for unsafe code patterns
- **RFC**: See [RFC.md](RFC.md) for feature roadmap and planning

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
