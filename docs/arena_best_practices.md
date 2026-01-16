# Arena Best Practices and Leak Prevention

This guide covers common memory leak patterns when using arena allocators and strategies to prevent them.

## Overview

Arena allocators provide high-performance memory allocation but require careful lifetime management. Unlike traditional allocators, arenas do not support individual deallocation - all allocations live until the arena is dropped or reset.

## Leak Detection

The OxideC runtime provides built-in leak detection in debug builds:

```rust
// Enable leak detection with stack traces
cargo build --features arena_backtrace

// Run with leak tracking enabled
RUST_LOG=debug cargo test
```

When an arena is dropped with unfreed allocations, you'll see:

```
[WARNING] Arena leaked 3 allocations:
  - 32 bytes (alignment: 8) at address 0x7f8e4c000000 [alloc::string::String]
  - 64 bytes (alignment: 16) at address 0x7f8e4c000020 [mycrate::Data]
  - 16 bytes (alignment: 8) at address 0x7f8e4c000060 [u32]
    Allocated at:
      0. mycrate::process_data
      1. mycrate::handle_request
      2. core::ops::function::FnOnce::call_once
[WARNING] Memory leaks detected! Ensure all arena-allocated objects are properly freed.
```

## Common Leak Patterns

### Pattern 1: Store Pointer Beyond Arena Lifetime

**[WRONG]** Leak: Storing arena pointer in a long-lived structure

```rust
// [WRONG] - LEAK!
struct Context {
    data: *mut u8,  // Allocated from arena
}

fn process_request(arena: &ScopedArena) -> Context {
    let data = arena.alloc_string("important data".to_string(), 100);
    Context { data }  // [LEAK] Context outlives the arena!
}

// After arena is dropped, context.data is a dangling pointer
```

**[OK]** Solution: Match lifetimes

```rust
// [OK] - Lifetime bounds prevent leaks
struct Context<'arena> {
    data: *mut u8,
    _phantom: std::marker::PhantomData<&'arena ()>,
}

fn process_request<'arena>(arena: &'arena ScopedArena) -> Context<'arena> {
    let data = arena.alloc_string("important data".to_string(), 100);
    Context { data, _phantom: std::marker::PhantomData }
}

// Context cannot outlive the arena due to lifetime bounds
```

**[OK]** Alternative: Use ScopedArena directly

```rust
// [OK] - ScopedArena owns the data
struct Context {
    arena: ScopedArena,
    data: *mut u8,
}

fn process_request() -> Context {
    let arena = ScopedArena::new(4096);
    let data = arena.alloc_string("important data".to_string(), 100);
    Context { arena, data }  // Arena and data move together
}
// Context owns the arena, so no leak
```

---

### Pattern 2: Forget to Reset Arena

**[WRONG]** Leak: Continuous allocation without reset

```rust
// [WRONG] - LEAK!
fn process_requests(arena: &ScopedArena) {
    for i in 0..1000 {
        let _data = arena.alloc(i);  // [LEAK] Never freed!
    }
    // Arena accumulates 1000 allocations
}
```

**[OK]** Solution: Reset arena between iterations

```rust
// [OK] - Reset reuses memory
fn process_requests(arena: &ScopedArena) {
    for i in 0..1000 {
        let _data = arena.alloc(i);
        arena.reset();  // Reuse memory
    }
    // Arena only has one allocation at a time
}
```

---

### Pattern 3: Leak Through Global Static

**[WRONG]** Leak: Store arena pointer in global variable

```rust
// [WRONG] - LEAK!
static GLOBAL_CACHE: Mutex<Option<*mut u8>> = Mutex::new(None);

fn init_cache(arena: &ScopedArena) {
    let data = arena.alloc(42u32);
    *GLOBAL_CACHE.lock().unwrap() = Some(data);
    // [LEAK] Global outlives arena!
}
```

**[OK]** Solution: Use global arena for global data

```rust
// [OK] - Global arena for global data
use oxidec::runtime::get_global_arena;

fn init_cache() {
    let arena = get_global_arena();  // Static lifetime
    let data = arena.alloc(42u32);
    *GLOBAL_CACHE.lock().unwrap() = Some(data);
    // OK: Global arena lives forever
}
```

---

### Pattern 4: Circular References

**[WRONG]** Leak: Circular references through arena pointers

```rust
// [WRONG] - LEAK!
struct Node {
    next: *mut Node,
    prev: *mut Node,
}

fn create_cycle(arena: &ScopedArena) {
    let node1 = arena.alloc(Node { next: std::ptr::null_mut(), prev: std::ptr::null_mut() });
    let node2 = arena.alloc(Node { next: std::ptr::null_mut(), prev: std::ptr::null_mut() });

    // Create cycle
    unsafe {
        (*node1).next = node2;
        (*node2).prev = node1;
    }
    // [LEAK] Both nodes leaked - arena can't detect or free them
}
```

**[OK]** Solution: Use indices or avoid cycles

```rust
// [OK] - Use indices instead of raw pointers
struct Node {
    next: Option<usize>,
    prev: Option<usize>,
}

fn create_list(arena: &ScopedArena) -> Vec<*mut Node> {
    let mut nodes = Vec::new();
    nodes.push(arena.alloc(Node { next: None, prev: None }));
    nodes.push(arena.alloc(Node { next: None, prev: None }));

    // Link by index
    unsafe {
        (*nodes[0]).next = Some(1);
        (*nodes[1]).prev = Some(0);
    }

    nodes  // Return vector for explicit tracking
}
// Caller can manage node lifetimes through the vector
```

---

### Pattern 5: Arena Reset With Active Pointers

**[WRONG]** Use-after-free: Using pointers after reset

```rust
// [WRONG] - UNDEFINED BEHAVIOR!
fn use_after_reset(arena: &ScopedArena) {
    let data = arena.alloc(42u32);
    arena.reset();
    unsafe {
        let val = *data;  // [UB!] data was invalidated by reset
    }
}
```

**[OK]** Solution: Scope pointers properly

```rust
// [OK] - Drop pointers before reset
{
    let data = arena.alloc(42u32);
    unsafe {
        let val = *data;  // Use data
    }
} // data goes out of scope
arena.reset();  // Safe: no pointers to data exist
```

---

## Prevention Strategies

### Strategy 1: Use ScopedArena by Default

For temporary allocations, prefer `ScopedArena` over manual `Arena`:

```rust
// [PREFERRED] Automatic cleanup
{
    let arena = ScopedArena::new(4096);
    let data = arena.alloc(42);
    // ... use data ...
} // Arena automatically dropped
```

### Strategy 2: Match Arena Lifetime to Data

Use lifetimes to prevent dangling pointers:

```rust
fn process<'arena>(arena: &'arena ScopedArena) -> &'arena mut u32 {
    unsafe { &mut *arena.alloc(42) }
}
// Return value cannot outlive arena
```

### Strategy 3: Reset Explicitly Between Phases

When reusing arenas, reset at clear phase boundaries:

```rust
fn handle_connection(arena: &ScopedArena, request: &Request) -> Response {
    // Phase 1: Parse request
    let parsed = parse_request(arena, request);

    // Phase 2: Process
    let result = process(arena, parsed);

    // Clear temporary data before response
    arena.reset();

    // Phase 3: Build response
    build_response(arena, result)
}
```

### Strategy 4: Document Lifetime Requirements

Clearly document when pointers become invalid:

```rust
/// Allocates a request context.
///
/// # Returns
///
/// Pointer to context valid until `arena` is reset or dropped.
///
/// # Safety
///
/// Caller must ensure the pointer is not used after:
/// - `arena.reset()` is called
/// - `arena` is dropped
/// - Any operation that invalidates arena memory
pub fn alloc_context(arena: &ScopedArena) -> *mut Context {
    arena.alloc(Context::new())
}
```

### Strategy 5: Use Leak Detection in Development

Enable leak tracking during development:

```toml
# Cargo.toml
[dependencies]
oxidec = { path = "../oxidec", features = ["arena_backtrace"] }

[profile.dev]
debug = true  # Enable debug assertions for leak tracking
```

## Testing for Leaks

### Unit Test Leak Detection

```rust
#[test]
fn test_no_leaks() {
    let arena = ScopedArena::new(4096);

    // Allocate and use data
    let data = arena.alloc(42u32);
    unsafe {
        assert_eq!(*data, 42);
    }

    // Drop goes here - leak tracker verifies no leaks
}

#[test]
fn test_reset_clears_leaks() {
    let arena = ScopedArena::new(4096);

    let _data = arena.alloc(42u32);
    arena.reset();  // Clears leak tracker

    // No leak warning on drop
}
```

### Integration Test with Sanitizers

```bash
# Run with Valgrind to detect memory errors
cargo test --test arena_integration
valgrind --leak-check=full --error-exitcode=1 ./target/debug/arena_integration-*

# Run with AddressSanitizer
RUSTFLAGS="-Zsanitizer=address" cargo test
```

## Performance vs Safety Trade-offs

| Approach | Performance | Safety | Use When... |
|----------|-------------|--------|-------------|
| **Global Arena** | Fastest | Manual | Long-lived metadata (classes, selectors) |
| **ScopedArena** | Fast | Automatic | Request-scoped allocations |
| **Arena + Reset** | Fastest | Manual | Reusable temporary buffers |
| **Manual alloc** | Slowest | Automatic | General-purpose memory |

## Debugging Leaks

### Enable Detailed Leak Reporting

```bash
# Build with backtrace support
cargo build --features arena_backtrace

# Run tests with leak tracking
RUST_BACKTRACE=1 cargo test

# Check for leak warnings in output
```

### Common Leak Indicators

1. **Growing memory usage** over time
2. **Leak warnings** in debug builds
3. **Valgrind errors** about lost records
4. **AddressSanitizer** reports of leaks

### Debug Checklist

- [ ] Leak detection enabled in debug builds
- [ ] All `ScopedArena` instances properly scoped
- [ ] No arena pointers stored beyond arena lifetime
- [ ] `reset()` called at appropriate phase boundaries
- [ ] No circular references through arena pointers
- [ ] Global data uses global arena, not scoped arenas

## Summary

Arena allocators provide exceptional performance but require careful lifetime management:

- **DO** use `ScopedArena` for automatic cleanup
- **DO** match lifetimes to arena scope
- **DO** reset arenas between logical phases
- **DO** enable leak tracking in development
- **DO NOT** store arena pointers beyond arena lifetime
- **DO NOT** use pointers after `reset()` or drop
- **DO NOT** create circular references with arena pointers

Following these patterns will help you avoid memory leaks while benefiting from arena allocator performance.
