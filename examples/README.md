# OxideX Example Programs

This directory contains 15 example programs demonstrating the OxideX language features.

## Example Programs

### 1. hello_world.ox
The simplest OxideX program - demonstrates basic print functionality.

### 2. functions.ox
Demonstrates function definitions, parameters, return types, and higher-order functions.

### 3. structs.ox
Shows struct definitions, field access, and methods.

### 4. enums.ox
Demonstrates enum definitions with variants and pattern matching.

### 5. control_flow.ox
Shows if/else expressions and guard statements.

### 6. loops.ox
Demonstrates for loops and while loops with iterators.

### 7. generics.ox
Shows generic types and functions with type parameters.

### 8. protocols.ox
Demonstrates protocol definitions and protocol conformance (traits).

### 9. classes.ox
Shows class definitions with methods and mutable state.

### 10. collections.ox
Demonstrates arrays and dictionaries with indexing and iteration.

### 11. operators.ox
Shows arithmetic, comparison, logical, and bitwise operators.

### 12. pattern_matching.ox
Advanced pattern matching with structs, enums, and arrays.

### 13. string_interpolation.ox
Demonstrates string interpolation with embedded expressions.

### 14. comptime.ox
Shows compile-time function evaluation and constants.

### 15. advanced.ox
Combines multiple advanced features: protocols, generics, recursive types.

## Running Examples

To parse an example (once the full toolchain is implemented):

```bash
oxidexc examples/hello_world.ox
```

To check syntax:

```bash
oxidexc --parse examples/advanced.ox
```

## Coverage

These examples cover all major language features:
- [x] Functions (including higher-order)
- [x] Structs and classes
- [x] Enums with pattern matching
- [x] Control flow (if, guard, match)
- [x] Loops (for, while)
- [x] Generics
- [x] Protocols (traits)
- [x] Collections (arrays, dicts)
- [x] Operators (arithmetic, logical, bitwise)
- [x] String interpolation
- [x] Compile-time evaluation
- [x] Visibility modifiers (pub, prv)
- [x] Static and const declarations
