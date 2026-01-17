// Pattern Matching - Demonstrates various pattern types

enum Shape {
    Circle { radius: Float },
    Rectangle { width: Float, height: Float },
    Point { x: Float, y: Float },
}

fn area(shape: Shape) -> Float {
    match shape {
        Shape::Circle { radius } => 3.14159 * radius * radius,
        Shape::Rectangle { width, height } => width * height,
        Shape::Point { .. } => 0.0,
    }
}

fn describe_point(point: [Float]) {
    match point {
        [0.0, 0.0] => print("Origin"),
        [x, 0.0] => print("On x-axis: " + x),
        [0.0, y] => print("On y-axis: " + y),
        [x, y] => print("Point: (" + x + ", " + y + ")"),
    }
}

fn main() {
    let circle = Shape::Circle { radius: 5.0 }
    print(area(circle))

    let rect = Shape::Rectangle { width: 10.0, height: 20.0 }
    print(area(rect))

    let point = Shape::Point { x: 3.0, y: 4.0 }
    print(area(point))

    describe_point([0.0, 0.0])
    describe_point([5.0, 0.0])
    describe_point([3.0, 4.0])
}
