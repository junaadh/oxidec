// Structs - Demonstrates struct definitions with free functions, methods, and impl blocks

struct Point {
    x: Float,
    y: Float,
}

// Approach 1: Standalone functions
fn point_distance(p1: Point, p2: Point) -> Float {
    let dx = p2.x - p1.x
    let dy = p2.y - p1.y
    ((dx * dx) + (dy * dy)).sqrt()
}

fn rectangle_area(r: Rectangle) -> Float {
    r.width * r.height
}

// Approach 2: Methods defined directly in struct body
struct Rectangle {
    width: Float,
    height: Float,

    fn area() -> Float {
        width * height
    }

    fn perimeter() -> Float {
        2.0 * (width + height)
    }
}

// Approach 3: impl blocks for additional methods
impl Point {
    fn distance(to other: Point) -> Float {
        let dx = other.x - x
        let dy = other.y - y
        ((dx * dx) + (dy * dy)).sqrt()
    }

    fn translated(by offset: Float) -> Point {
        Point { x: x + offset, y: y + offset }
    }
}

impl Rectangle {
    fn isSquare() -> Bool {
        width == height
    }

    fn scaled(by factor: Float) -> Rectangle {
        Rectangle {
            width: width * factor,
            height: height * factor,
        }
    }
}

fn main() {
    // Using standalone function
    let origin = Point { x: 0.0, y: 0.0 }
    let p = Point { x: 3.0, y: 4.0 }

    let dist1 = point_distance(origin, p)
    print(dist1)

    // Using method from impl block
    let dist2 = origin.distance(to: p)
    print(dist2)

    // Using translated method
    let p2 = p.translated(by: 1.0)
    print(p2.distance(to: origin))

    // Using Rectangle methods (defined directly in struct)
    let rect = Rectangle { width: 5.0, height: 10.0 }

    // Method from struct body
    print(rect.area())

    // Using standalone function with same struct
    let area1 = rectangle_area(rect)
    print(area1)

    // Method from struct body
    print(rect.perimeter())

    // Methods from impl block
    print(rect.isSquare())

    let square = Rectangle { width: 4.0, height: 4.0 }
    print(square.isSquare())

    let scaled = rect.scaled(by: 2.0)
    print(scaled.area())
}
