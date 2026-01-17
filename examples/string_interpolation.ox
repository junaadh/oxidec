// String Interpolation - Demonstrates string interpolation

fn greet(name: String, age: Int) -> String {
    "Hello, \(name)! You are \(age) years old."
}

fn format_point(x: Float, y: Float) -> String {
    "Point: (\(x), \(y))"
}

fn main() {
    let name = "Alice"
    let age = 30
    let message = greet(name, age)
    print(message)

    let x = 3.14
    let y = 2.71
    let point_str = format_point(x, y)
    print(point_str)

    let calculation = "Result: \(5 + 3) = \(5 * 3)"
    print(calculation)
}
