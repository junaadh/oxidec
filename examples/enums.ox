// Enums - Demonstrates enum definitions, methods, impl blocks, protocols, and error handling

enum Option<T> {
    case some(T),
    case none,

    // Method directly in enum body
    fn isSome() -> Bool {
        match self {
            .some(_) => true,
            .none => false,
        }
    }
}

// impl block for additional methods
impl Option<T> {
    fn unwrap() -> T {
        match self {
            .some(v) => v,
            .none => panic("Called unwrap on none"),
        }
    }

    fn map<U>(f: fn(T) -> U) -> Option<U> {
        match self {
            .some(v) => .some(f(v)),
            .none => .none,
        }
    }
}

enum Result<T, E> {
    case ok(T),
    case err(E),
}

impl Result<T, E> {
    fn isOk() -> Bool {
        match self {
            .ok(_) => true,
            .err(_) => false,
        }
    }

    fn toOption() -> Option<T> {
        match self {
            .ok(v) => .some(v),
            .err(_) => .none,
        }
    }
}

// Protocol conformance for enums
protocol Equatable {
    fn eq(other: Self) -> Bool
}

impl Equatable for Option<Int> {
    fn eq(other: Self) -> Bool {
        match (self, other) {
            (.some(a), .some(b)) => a == b,
            (.none, .none) => true,
            _ => false,
        }
    }
}

fn divide(x: Float, y: Float) -> Result<Float, String> {
    if y == 0.0 {
        .err("Division by zero")
    } else {
        .ok(x / y)
    }
}

// Error handling with try/try?
fn parseOrThrow(s: String) -> Result<Int, String> {
    // Both syntaxes allowed:
    let x = try parseInt(s)
    // OR: let x = parseInt(s)?
    .ok(x)
}

fn parseOption(s: String) -> Option<Int> {
    // try? converts Result to Option
    try? parseInt(s)
}

// guard with try?
fn safeParse(s: String) -> Option<Int> {
    guard let x = try? parseInt(s) else {
        return .none
    }
    .some(x)
}

fn main() {
    // Type inference with .variant syntax
    let maybe_value: Option<Int> = .some(42)
    let no_value: Option<Int> = .none

    // Method calls from enum body
    print(maybe_value.isSome())
    print(no_value.isSome())

    // Method calls from impl block
    print(maybe_value.unwrap())

    // Pattern matching with .variant
    match maybe_value {
        .some(v) => print("Value: " + v),
        .none => print("No value"),
    }

    // if let with .variant pattern
    if let .some(x) = maybe_value {
        print("Got value: " + x)
    }

    // Error handling
    let result1 = divide(10.0, 2.0)
    let result2 = divide(5.0, 0.0)

    match result1 {
        .ok(v) => print("Success: " + v),
        .err(e) => print("Error: " + e),
    }

    match result2 {
        .ok(v) => print("Success: " + v),
        .err(e) => print("Error: " + e),
    }

    // try? for error conversion
    let parsed = safeParse("42")
    match parsed {
        .some(x) => print("Parsed: " + x),
        .none => print("Failed to parse"),
    }

    // Protocol conformance
    let opt1: Option<Int> = .some(42)
    let opt2: Option<Int> = .some(42)
    let opt3: Option<Int> = .none

    print(opt1.eq(opt2))  // true
    print(opt1.eq(opt3))  // false

    // map method
    let mapped = maybe_value.map(fn (x: Int) -> Int { x * 2 })
    match mapped {
        .some(v) => print("Mapped: " + v),
        .none => print("Nothing to map"),
    }
}
