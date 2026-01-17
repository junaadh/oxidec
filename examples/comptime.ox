// Comptime - Demonstrates compile-time evaluation

fn comptime square(x: Int) -> Int {
    x * x
}

const MAX_SIZE: Int = square(10)

fn comptime factorial(n: Int) -> Int {
    if n <= 1 {
        1
    } else {
        n * factorial(n - 1)
    }
}

const FACTORIAL_5: Int = factorial(5)

fn main() {
    print("MAX_SIZE: " + MAX_SIZE)
    print("FACTORIAL_5: " + FACTORIAL_5)

    // Runtime calculation
    let x = 7
    let result = factorial(x)
    print("factorial(7): " + result)
}
