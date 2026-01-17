// Functions - Demonstrates function definitions and calls

fn add(x: Int, y: Int) -> Int {
    x + y
}

fn greet(name: String) -> String {
    "Hello, " + name
}

fn apply_twice(f: fn(Int) -> Int, x: Int) -> Int {
    f(f(x))
}

fn double(x: Int) -> Int {
    x * 2
}

fn main() {
    let sum = add(5, 3)
    print(sum)

    let message = greet("OxideX")
    print(message)

    let result = apply_twice(double, 5)
    print(result)
}
