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
        Option::Some(v) => print("Valid: " + v),
        Option::None => print("Invalid"),
    }

    match validate_positive(-5) {
        Option::Some(v) => print("Valid: " + v),
        Option::None => print("Invalid"),
    }

    let m = max_of_three(3, 7, 5)
    print(m)
}
