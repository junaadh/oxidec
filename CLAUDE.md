# OxideC: Modern Objective-C Runtime in Rust

**Version:** See [Cargo.toml](Cargo.toml)
**Status:** See [RFC.md](RFC.md) for current phase and roadmap
**Last Updated:** 2026-01-16

You are assisting with the OxideC project—a dynamic object runtime inspired by Objective-C, redesigned for modern systems programming in Rust.

## Project Overview

OxideC is building a safe, high-performance dynamic runtime with:
- **Dynamic Dispatch** with late binding and method caching
- **Memory Safety** with manual management in unsafe internals, safe public API wrapping
- **C ABI Compatibility** for FFI and interoperability
- **Runtime Reflection** for introspection and tooling
- **Modern Extensions** including tagged pointers and type-aware dispatch

## Architecture & Safety Model

### Safety Invariants
The entire runtime is built on this core principle:
- **Public API Layer** (`pub`): Type-safe, validated, safe abstractions
- **Runtime Layer** (`pub(crate)`): Unsafe internals, heavily audited
- **Safety Guarantees**: The public API design prevents misuse of unsafe code

### Key Modules
- **arena.rs**: Arena allocator for long-lived metadata
- **object.rs**: Object allocation, reference counting, lifecycle
- **class.rs**: Class creation, inheritance, method registration
- **selector.rs**: Selector interning and caching
- **dispatch.rs**: Message dispatch with method lookup
- **encoding.rs**: Method signature type encoding
- **message.rs**: Message argument handling
- **string.rs**: Runtime string with SSO and interning
- **cache.rs**: Method call caching for performance
- **protocol.rs**: Protocol conformance checking (planned)

## Code Style & Conventions

### Rust Best Practices
- Use `unsafe` blocks with explicit SAFETY comments explaining invariants and lifetimes
- Manually manage memory with Box::into_raw/from_raw for performance
- Prefer checked arithmetic over unchecked for production code
- Use atomic operations for shared state access (prevents data races)
- Document pointer lifetimes and validity preconditions in SAFETY comments
- Prove pointer validity before every dereference
- Document public APIs thoroughly with examples
- **Strict Provenance**: Always use `map_addr()` for pointer arithmetic, never `as usize`
- **Atomic Pointers**: Use `AtomicPtr<T>` instead of `AtomicUsize` for pointer storage
- **Stacked Borrows**: Use `addr_of!` and `offset_of!` to avoid temporary references
- **Aligned Access**: Use `read_unaligned`/`write_unaligned` for potentially misaligned pointers

### Documentation Style
- **NO EMOJIS**: Do not use emojis in documentation, code comments, or any project files
- Use text markers instead: "[OK]", "[WRONG]", "[WARNING]", etc.
- Use markdown checkboxes for lists: [x] for completed, [ ] for pending
- Keep documentation technical and professional
- Use bullet points and numbered lists for clarity

### Module Structure
```
src/
├── lib.rs              # Public API entry point
├── error.rs            # Error types
└── runtime/
    ├── mod.rs          # Runtime initialization
    ├── arena.rs        # Arena allocator
    ├── object.rs       # Object implementation
    ├── class.rs        # Class system
    ├── selector.rs     # Selector interning
    ├── dispatch.rs     # Message dispatch
    ├── encoding.rs     # Type encoding
    ├── message.rs      # Message arguments
    ├── string.rs       # Runtime strings
    ├── cache.rs        # Method caching
    ├── category.rs     # Category support
    ├── protocol.rs     # Protocol support
    └── forwarding.rs   # Message forwarding
```

### Documentation Requirements
- Public items: Explain **what**, **why**, and **when** to use
- Safety comments: Document all unsafe blocks with SAFETY:
- Examples: Include runnable examples for public APIs
- Architecture: Document safety invariants clearly

## Current Roadmap

### Phase 1: Foundation (Alpha 0.1) - COMPLETE
- [x] Selector interning system
- [x] Method registry implementation
- [x] Class creation and registration
- [x] Object allocation and deallocation
- [x] Reference counting with atomic operations
- [x] Arena allocator for metadata
- [x] Runtime string with SSO

### Phase 2: Dispatch (Alpha 0.2) - COMPLETE
- [x] Basic message dispatch
- [x] Method lookup with caching
- [x] Inheritance resolution
- [x] Method overriding
- [x] Message argument handling
- [x] Type encoding system
- [x] MIRI validation (all 115 tests pass with strict provenance)

### Phase 3: Extensions (Alpha 0.3) - COMPLETE
- [x] Categories (dynamic methods)
- [x] Protocols
- [x] Message forwarding (per-class and global hooks)
- [x] Method swizzling (runtime method replacement)
- [x] Integration tests (10 new tests)
- [x] MIRI validation (all 148 unit tests pass with strict provenance)

### Phase 4+: See RFC.md for full roadmap

## Performance Considerations

When implementing features, prioritize in this order:
1. **Correctness** - Unsafe code must be sound with proper pointer lifetimes
2. **Safety** - Public API prevents misuse of unsafe internals
3. **Performance** - Optimize via manual memory management after safety is guaranteed

### Optimization Techniques
- Manual memory management with unsafe pointers for zero-overhead abstractions
- Selector hashing with precomputed values
- Method caching per class
- Inline caches for hot paths
- Tagged pointer encoding for small objects
- Minimal allocations via careful lifetime management

## Testing & Validation

All new features require:
- Unit tests in module files (see [RFC.md](RFC.md) for current test count)
- Integration tests in /tests directory (see [RFC.md](RFC.md) for current test count)
- Safety proofs in SAFETY comments
- Benchmark code for performance-critical paths
- **MIRI validation** with `-Zmiri-strict-provenance` to ensure no undefined behavior

### MIRI Validation
Run MIRI before committing changes:
```bash
MIRIFLAGS="-Zmiri-strict-provenance -Zmiri-ignore-leaks" cargo +nightly miri test
```

All code must pass MIRI validation with strict provenance to ensure:
- No undefined behavior
- Proper pointer provenance
- Correct alignment handling
- No Stacked Borrows violations

### Code Quality Enforcement

Before committing or pushing changes, always run:

```bash
# 1. Fix all clippy warnings (pedantic level required)
cargo clippy --all-targets --all-features -- -W clippy::pedantic --deny warnings

# 2. Run all tests
cargo test

# 3. Validate with MIRI
MIRIFLAGS="-Zmiri-strict-provenance -Zmiri-ignore-leaks" cargo +nightly miri test

# 4. Format code
cargo fmt --all
```

**Essential:** All warnings must be fixed before merging. Use `cargo clippy --fix` to automatically fix issues where possible.

All code must pass MIRI validation with strict provenance to ensure:
- No undefined behavior
- Proper pointer provenance
- Correct alignment handling
- No Stacked Borrows violations

## When Working on This Project

1. **Always check .claude/settings.json** for current permissions and configurations
2. **Reference RFC.md** for feature specifications
3. **Read ARCHITECTURE.md** for design decisions
4. **Consult SAFETY.md** for unsafe code guidelines
5. **Document** all public APIs with examples and safety notes

## Current Context

- **Edition**: 2024 (Rust stable)
- **Target**: Systems programming, FFI compatibility
- **Status**: Alpha 0.3 - Phase 3 Complete
- **Next Milestone**: Phase 4 - Optimization & Performance
- **Testing**: See [RFC.md](RFC.md) for test coverage and validation status

---

**Primary Author**: Junaadh
**Version**: See [Cargo.toml](Cargo.toml)
**MIRI Status**: See [RFC.md](RFC.md) for MIRI validation status
