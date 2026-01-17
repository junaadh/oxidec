// Control Flow - Demonstrates if/else and guard statements

fn abs(x: Int) -> Int {
    if x < 0 {
        -x
    } else {
        x
    }
}

fn validate_positive(x: Int) -> Option<Int> {
    guard x >= 0 else {
        return Option::None
    }
    Option::Some(x)
}

fn max_of_three(a: Int, b: Int, c: Int) -> Int {
    if a > b {
        if a > c {
            a
        } else {
            c
        }
    } else {
        if b > c {
            b
        } else {
            c
        }
    }
}

fn main() {
    let value = -5
    print(abs(value))

    match validate_positive(10) {
        .some(v) => print("Valid: " + v),
        .none => print("Invalid"),
    }

    match validate_positive(-5) {
        .some(v) => print("Valid: " + v),
        .none => print("Invalid"),
    }

    let m = max_of_three(3, 7, 5)
    print(m)

    // if let with .variant pattern
    if let .some(v) = validate_positive(42) {
        print("Valid positive: " + v)
    }

    // guard with try? for error handling
    let result = safeParse("10")
    match result {
        .some(x) => print("Parsed successfully: " + x),
        .none => print("Failed to parse"),
    }
}

// Example function showing guard with try?
fn safeParse(s: String) -> Option<Int> {
    guard let x = try? parseInt(s) else {
        return .none
    }

    guard x >= 0 else {
        return .none
    }

    .some(x)
}
