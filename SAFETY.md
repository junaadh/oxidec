# OxideC Safety Guidelines

**Version:** See [Cargo.toml](Cargo.toml) for current version
**Testing Status:** See [RFC.md](RFC.md) for MIRI validation status and test coverage

## Unsafe Code Philosophy

This document establishes patterns and requirements for all unsafe code in OxideC.

**Core Rule:** Every `unsafe` block must have a **SAFETY** comment that proves the code is sound.

## MIRI Validation
All unsafe code is validated with MIRI using `-Zmiri-strict-provenance` to ensure:
- No undefined behavior
- Proper pointer provenance
- Correct alignment handling
- No Stacked Borrows violations

**Validation Status:** See [RFC.md](RFC.md) for current MIRI validation status and test coverage

---

## SAFETY Comment Format

Every `unsafe` block must include a comment explaining:
1. **What** the code does
2. **Why** it's safe (invariant proof)
3. **Preconditions** that must hold
4. **Postconditions** it establishes

```rust
unsafe {
    // SAFETY: <what> <why>
    // Preconditions:
    // - Invariant 1
    // - Invariant 2
    // Postconditions:
    // - Result is valid for X operations
    let result = dangerous_operation();
}
```

---

## Strict Provenance Compliance

OxideC follows Rust's strict provenance model to ensure undefined behavior-free code.

### Pointer Tagging
When storing metadata in pointer bits (e.g., encoding tags):

```rust
// [OK] CORRECT: Use map_addr to preserve provenance
let tagged_ptr = ptr.map_addr(|addr| addr | ENCODING_BIT) as *mut u8;

// [WRONG] Loses provenance
let tagged_ptr = (ptr as usize | ENCODING_BIT) as *mut u8;
```

### Pointer Untagging
When extracting metadata from pointers:

```rust
// [OK] CORRECT: Use map_addr to preserve provenance
let heap_ptr = ptr.map_addr(|addr| addr & POINTER_MASK) as *const HeapString;

// [WRONG] Loses provenance
let heap_ptr = (ptr as usize & POINTER_MASK) as *const HeapString;
```

### Atomic Pointer Storage
When storing pointers in atomic types:

```rust
// [OK] CORRECT: Use AtomicPtr with with_addr
pub struct Chunk {
    ptr: AtomicPtr<u8>,  // Preserves provenance
}

// Load and modify address
let current_ptr = self.ptr.load(Ordering::Acquire);
let new_ptr = current_ptr.with_addr(new_addr);
self.ptr.store(new_ptr, Ordering::Release);

// [WRONG] Loses provenance
pub struct Chunk {
    ptr: AtomicUsize,  // Loses provenance
}
```

### Address Arithmetic
When doing pointer arithmetic:

```rust
// [OK] CORRECT: Use addr() for arithmetic, with_addr() to reconstruct
let current_addr = ptr.addr();
let new_addr = current_addr.wrapping_add(offset);
let new_ptr = ptr.with_addr(new_addr);

// [WRONG] Loses provenance
let new_ptr = (ptr as usize + offset) as *mut u8;
```

### Field Access Without References
When avoiding Stacked Borrows violations:

```rust
// [OK] CORRECT: Use addr_of! to get pointer without creating reference
let data_ptr = unsafe { std::ptr::addr_of!((*heap_ptr).data).cast::<u8>() };

// [OK] CORRECT: Use offset_of! to calculate field offsets
let data_offset = std::mem::offset_of!(HeapString, data);
let data_ptr = unsafe { (ptr as *const u8).add(data_offset) };

// [WRONG] Creates temporary reference that violates Stacked Borrows
let data_ptr = unsafe { (*heap_ptr).data.as_ptr() };
```

### Unaligned Access
When accessing potentially misaligned memory:

```rust
// [OK] CORRECT: Use read_unaligned/write_unaligned
unsafe {
    std::ptr::write_unaligned(ret_ptr as *mut usize, 42);
    let value = std::ptr::read_unaligned(ret_ptr as *const usize);
}

// [WRONG] Undefined behavior on misaligned access
unsafe {
    *(ret_ptr as *mut usize) = 42;
    let value = *(ret_ptr as *const usize);
}
```

### Stacked Borrows: Box Ownership Management
**CRITICAL:** Never mix `Box::leak` with `Box::from_raw` - this causes Stacked Borrows violations!

#### Problem Pattern
```rust
// [WRONG] Creates &mut reference tied to Box lifetime
let chunk_ptr = Box::leak(Box::new(chunk));

match atomic_cas(&mut ptr, old, chunk_ptr) {
    Ok(_) => { /* ... */ }
    Err(_) => {
        // VIOLATION: Cannot Box::from_raw on a leaked reference!
        // This causes Stacked Borrows violations.
        unsafe {
            let _ = Box::from_raw(chunk_ptr);  // [WRONG] UB!
        }
    }
}
```

#### Safe Pattern with Atomic Operations
```rust
// [OK] CORRECT: Use Box::into_raw to keep ownership in raw pointer
let chunk_box = Box::new(chunk);
let chunk_raw = Box::into_raw(chunk_box);  // Ownership in raw pointer

match atomic_cas(&mut ptr, old, chunk_raw) {
    Ok(_) => {
        // Success: chunk_raw is now leaked (owned by arena)
        // DO NOT call Box::from_raw on chunk_raw - it's no longer ours
        unsafe {
            let old_box = Box::from_raw(old);  // Reclaim old chunk
            chunks.push(*old_box);
        }
    }
    Err(_) => {
        // Failure: reclaim our chunk (safe because we own it)
        unsafe {
            drop(Box::from_raw(chunk_raw));  // Safe to reclaim
        }
    }
}
```

#### Critical Rules

1. **Before Atomic CAS**: Use `Box::into_raw` (keeps ownership in raw pointer)
2. **After Successful CAS**: Raw pointer is leaked (transferred to arena)
3. **After Failed CAS**: Use `Box::from_raw` to reclaim ownership
4. **NEVER**: Use `Box::leak` before CAS (creates conflicting &mut reference)
5. **NEVER**: Call `Box::from_raw` on a leaked reference (Stacked Borrows violation)

#### Ownership Flow
```
Box::into_raw → CAS Operation → Success → (pointer leaked)
                    ↳ Failure → Box::from_raw → (Box dropped)

Box::leak → CAS Operation → Success → (pointer leaked)
          ↳ Failure → Box::from_raw → [WRONG] UB! Stacked Borrows violation
```

#### When to Use Each Function

| Function | Purpose | Returns | Ownership |
|----------|---------|--------|-----------|
| `Box::into_raw` | Transfer ownership to raw pointer | `*mut T` | Transferred to raw pointer |
| `Box::leak` | Create 'static reference from Box | `&'static mut T` | **Still owned by Box!** |
| `Box::from_raw` | Reclaim ownership from raw pointer | `Box<T>` | Ownership back to Box |

**Key Insight**: `Box::leak` creates a reference while the Box still exists, causing lifetime conflicts. `Box::into_raw` fully transfers ownership, avoiding the conflict.

---

## Allowed Unsafe Patterns

### 1. Raw Pointer Dereference

**When:** Accessing interior through NonNull that we own.

```rust
unsafe {
    // SAFETY: ptr is NonNull (never null), always valid (constructed from Box)
    // Object lifetime is managed by Drop impl which holds this pointer
    let obj = &*self.ptr.as_ptr();
}
```

**Must prove:**
- Pointer is not null ✓
- Pointer is properly aligned ✓
- Pointer is valid for the type ✓
- No other mutable references exist ✓

### 2. Atomic Operations

**When:** Implementing reference counting or flags.

```rust
unsafe {
    // SAFETY: AtomicU32 is always valid, atomic ops never panic
    // Ordering::AcqRel ensures happens-before relationships
    let old = refcount.fetch_add(1, Ordering::AcqRel);
    if old == u32::MAX {
        panic!("overflow");
    }
}
```

**Must prove:**
- Atomic exists and is properly aligned ✓
- Ordering matches usage pattern ✓
- Result is checked before use ✓

### 3. Memory Layout Assumptions

**When:** Using repr(C) types as FFI boundaries.

```rust
#[repr(C)]
pub struct OxObject {
    class: *const OxClass,
    flags: u32,
    refcount: u32,
}

unsafe {
    // SAFETY: Field offsets are guaranteed by repr(C)
    // Layout matches C headers at compile time
    let class_ptr = &(*obj).class as *const _;
}
```

**Must prove:**
- Type has repr(C) attribute ✓
- Field sizes are platform-independent ✓
- Alignment is safe for platform ✓

### 4. Function Pointer Calls

**When:** Invoking method implementations.

```rust
type MethodImp = unsafe extern "C" fn(
    obj: *mut Object,
    sel: *const Selector,
    args: *const *mut u8,
    ret: *mut u8,
);

unsafe {
    // SAFETY: method.imp comes from registered class method table
    // All arguments are properly constructed before call
    // Caller ensures obj pointer is valid
    (*method.imp)(obj, sel, args, ret);
}
```

**Must prove:**
- Function pointer is valid ✓
- All arguments are properly constructed ✓
- Function type signature matches ✓

---

## Forbidden Patterns

### [WRONG] Unchecked Arithmetic

```rust
// NEVER DO THIS
unsafe {
    let new_count = refcount.wrapping_add(1);  // Bad: unchecked wrap
}
```

Instead:
```rust
// DO THIS
let old = refcount.fetch_add(1, Ordering::AcqRel);
if old == u32::MAX {
    return Err(Error::RefCountOverflow);
}
```

### [WRONG] Unvalidated Pointer Casts

```rust
// NEVER DO THIS
unsafe {
    let obj = user_pointer as *const Object;  // Bad: no validation
    (*obj).method();
}
```

Instead:
```rust
// DO THIS
unsafe {
    // SAFETY: Pointer comes from our allocation, validated before cast
    let obj = NonNull::new(user_pointer)?;
    let obj = obj.cast::<Object>();
    (*obj.as_ptr()).method();
}
```

### [WRONG] Non-Atomic Refcount

```rust
// NEVER DO THIS
unsafe {
    self.refcount += 1;  // Bad: non-atomic, data races
}
```

Instead:
```rust
// DO THIS
unsafe {
    // SAFETY: Atomic operation prevents data races
    self.refcount.fetch_add(1, Ordering::AcqRel);
}
```

### [WRONG] Ignoring Error States

```rust
// NEVER DO THIS
unsafe {
    let result = risky_operation();
    if result == INVALID {
        // Ignored!
    }
}
```

Instead:
```rust
// DO THIS
unsafe {
    // SAFETY: Check result before proceeding
    let result = risky_operation()?;
    if result == INVALID {
        return Err(Error::InvalidState);
    }
}
```

---

## Ownership & Lifetime Rules

### Manual Memory Management

**Rule:** Use Box::into_raw for stable pointers, carefully manage lifetimes.

```rust
unsafe {
    // SAFETY: Box::into_raw creates stable pointer, no move/reallocation
    // Lifetime: caller must ensure deallocation with Box::from_raw
    let ptr = Box::into_raw(Box::new(object));

    // Later...
    drop(Box::from_raw(ptr));  // Reclaim memory
}
```

**Atomic Operations Pattern:**
When using atomic operations with Boxes:
```rust
// [OK] CORRECT: Atomic CAS with Box::into_raw
let boxed = Box::new(value);
let raw = Box::into_raw(boxed);

match atomic_ptr.compare_exchange_weak(old, raw, ...) {
    Ok(_) => {
        // Success: raw is leaked (owned by arena)
        // Reclaim old value
        let _ = Box::from_raw(old);
    }
    Err(_) => {
        // Failure: reclaim our new value
        let _ = Box::from_raw(raw);
    }
}

// [WRONG] Atomic CAS with Box::leak
let leaked = Box::leak(Box::new(value));  // &mut T tied to Box

match atomic_ptr.compare_exchange_weak(old, leaked, ...) {
    Ok(_) => { /* ... */ }
    Err(_) => {
        // VIOLATION: Cannot Box::from_raw on leaked reference!
        let _ = Box::from_raw(leaked);  // [WRONG] UB!
    }
}
```

### Pointer Validity

**Rule:** Prove pointer validity and lifetime before every dereference.

```rust
unsafe {
    // SAFETY: Pointer validity proven by:
    // 1. Created from Box::into_raw (proper allocation, alignment)
    // 2. Lifetime valid: owned by static cache, lives for program duration
    // 3. Accessed while owner holds reference
    let obj = &*self.ptr.as_ptr();
}
```

### Slice Creation

**Rule:** Never create overlapping or invalid slices.

```rust
unsafe {
    // SAFETY: args_ptr points to valid array of u32::MAX arguments max
    // from argument validation in public API
    let args = slice::from_raw_parts(args_ptr, arg_count);
}
```

---

## Concurrent Access

### Atomic Operations

All shared state must use atomics:
```rust
pub struct Object {
    refcount: AtomicU32,  // OK: atomic
    flags: u32,           // NO: would need synchronization elsewhere
}
```

### Ordering Semantics

Use appropriate memory ordering:

| Ordering | Use Case |
|----------|----------|
| Relaxed | Simple counters, no synchronization needed |
| Acquire | Loading; establishes acquire-release pair |
| Release | Storing; establishes acquire-release pair |
| AcqRel | Modify-and-swap; both acquire and release |
| SeqCst | Full sequential consistency (use sparingly) |

```rust
// Reference counting - use AcqRel
unsafe {
    // SAFETY: AcqRel ordering ensures happens-before
    let old = refcount.fetch_add(1, Ordering::AcqRel);
}

// Lazy init - use Acquire for load, Release for store
static INIT: AtomicBool = AtomicBool::new(false);

if !INIT.load(Ordering::Acquire) {
    // Initialize
    INIT.store(true, Ordering::Release);
}
```

---

## Memory Allocation

### Safe Allocation

```rust
// DO THIS: Use Box for owned allocations
let obj = Box::new(RawObject {
    class_ptr: class,
    refcount: AtomicU32::new(1),
});

unsafe {
    // SAFETY: Box guarantees valid allocation, proper alignment
    // Box::into_raw gives stable pointer, lifetime managed by owner
    let ptr = Box::into_raw(obj);
    // ptr now refers to heap allocation, valid until Box::from_raw
}
```

### Custom Allocators

```rust
// If using custom allocator, document the contract
unsafe {
    // SAFETY: Allocation from allocator at address X with size Y
    // Deallocation must use same allocator and match size exactly
    let ptr = allocator.alloc(layout)?;
}
```

### Deallocation

```rust
unsafe {
    // SAFETY: ptr was created with Box::into_raw above
    // Box::from_raw reclaims ownership and drops on scope exit
    drop(Box::from_raw(self.ptr.as_ptr()));
}
```

---

## Testing Unsafe Code

### Unit Test Pattern

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_refcount_increment() {
        unsafe {
            // SAFETY: Test environment, controlled pointer
            let obj = Box::new(RawObject {
                refcount: AtomicU32::new(0),
            });
            
            assert_eq!(obj.refcount.fetch_add(1, Ordering::Relaxed), 0);
            assert_eq!(obj.refcount.load(Ordering::Relaxed), 1);
        }
    }
}
```

### Property Test Pattern

```rust
#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn refcount_never_overflows(initial in 0u32..u32::MAX) {
            unsafe {
                let refcount = AtomicU32::new(initial);
                if initial < u32::MAX {
                    let old = refcount.fetch_add(1, Ordering::Relaxed);
                    assert_ne!(old, u32::MAX);
                }
            }
        }
    }
}
```

---

## Audit Checklist

Before code review, ensure:

- [ ] Every `unsafe` block has a SAFETY comment
- [ ] SAFETY comment explains what, why, preconditions, and postconditions
- [ ] Pointer validity proven at dereference (source, lifetime, alignment)
- [ ] Pointer lifetime is documented and respected
- [ ] All pointer allocations have matching deallocations
- [ ] All FFI boundaries use repr(C)
- [ ] All error cases are handled
- [ ] All tests pass (including property tests)
- [ ] No undefined behavior possible
- [ ] No data races possible
- [ ] No pointer leaks possible
- [ ] Code passes MIRI with `-Zmiri-strict-provenance`
- [ ] Proper use of `map_addr()` for pointer tagging/untagging
- [ ] Proper use of `AtomicPtr` instead of `AtomicUsize` for pointers
- [ ] Proper use of `addr_of!` and `offset_of!` to avoid Stacked Borrows issues
- [ ] Proper use of `read_unaligned`/`write_unaligned` for misaligned access
- [ ] **Proper Box ownership: `Box::into_raw` before CAS, NOT `Box::leak`**
- [ ] **Never call `Box::from_raw` on a `Box::leak` reference (Stacked Borrows violation)**

---

## Examples by Module

### arena.rs
- Arena allocation with AtomicPtr for provenance preservation
- Bump pointer allocation using with_addr() for address modification
- Thread-safe allocation with proper atomic ordering
- **CRITICAL**: Atomic chunk allocation using `Box::into_raw` NOT `Box::leak`
- **CRITICAL**: Proper Box ownership management to prevent Stacked Borrows violations

### object.rs
- Reference counting with overflow detection
- Pointer validity through NonNull
- Automatic cleanup with Drop

### class.rs
- Class hierarchy traversal
- Method lookup with caching
- Protocol adoption and conformance checks
- Protocol validation with method walking

### protocol.rs
- Protocol creation and inheritance (base_protocol)
- Required and optional method registration
- Protocol conformance validation
- Arena allocation for protocol metadata
- Thread-safe method tables with RwLock

### dispatch.rs
- Method invocation through function pointers
- Argument marshalling with MessageArgs enum
- Return value handling with unaligned access

### string.rs
- Small String Optimization (SSO) with inline storage
- Pointer tagging for encoding detection using map_addr()
- String interning with arena allocation
- Flexible array member access using addr_of! and offset_of!

---

**Document Status:** Living document
**Last Updated:** 2026-01-16
**Version:** See [Cargo.toml](Cargo.toml)
**MIRI Status:** See [RFC.md](RFC.md) for MIRI validation status
**Test Coverage:** See [RFC.md](RFC.md) for test coverage details
