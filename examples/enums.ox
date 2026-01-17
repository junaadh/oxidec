// Enums - Demonstrates enum definitions and pattern matching

enum Option<T> {
    Some(T),
    None,
}

enum Result<T, E> {
    Ok(T),
    Err(E),
}

fn divide(x: Float, y: Float) -> Result<Float, String> {
    if y == 0.0 {
        Result::Err("Division by zero")
    } else {
        Result::Ok(x / y)
    }
}

fn main() {
    let maybe_value = Option::Some(42)
    let no_value = Option::None

    match maybe_value {
        Option::Some(v) => print("Value: " + v),
        Option::None => print("No value"),
    }

    let result1 = divide(10.0, 2.0)
    let result2 = divide(5.0, 0.0)

    match result1 {
        Result::Ok(v) => print("Success: " + v),
        Result::Err(e) => print("Error: " + e),
    }

    match result2 {
        Result::Ok(v) => print("Success: " + v),
        Result::Err(e) => print("Error: " + e),
    }
}
