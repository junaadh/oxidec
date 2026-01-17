// Pattern Matching - Demonstrates various pattern types

enum Shape {
    Circle { radius: Float },
    Rectangle { width: Float, height: Float },
    Point { x: Float, y: Float },
}

fn area(shape: Shape) -> Float {
    match shape {
        .circle { radius } => 3.14159 * radius * radius,
        .rectangle { width, height } => width * height,
        .point { .. } => 0.0,
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

fn get_radius(shape: Shape) -> Option<Float> {
    match shape {
        .circle { radius } => .some(radius),
        _ => .none,
    }
}

fn main() {
    let circle = .circle { radius: 5.0 }
    print(area(circle))

    let rect = .rectangle { width: 10.0, height: 20.0 }
    print(area(rect))

    let point = .point { x: 3.0, y: 4.0 }
    print(area(point))

    // if let pattern matching
    if let .some(r) = get_radius(circle) {
        print("Circle radius: " + r)
    }

    if let .some(r) = get_radius(rect) {
        print("Rectangle radius?: " + r)
    } else {
        print("Not a circle")
    }

    describe_point([0.0, 0.0])
    describe_point([5.0, 0.0])
    describe_point([3.0, 4.0])
}
