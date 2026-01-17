// Structs - Demonstrates struct definitions and usage

struct Point {
    x: Float,
    y: Float,
}

struct Rectangle {
    width: Float,
    height: Float,
}

fn point_distance(p1: Point, p2: Point) -> Float {
    let dx = p2.x - p1.x
    let dy = p2.y - p1.y
    ((dx * dx) + (dy * dy)).sqrt()
}

fn rectangle_area(r: Rectangle) -> Float {
    r.width * r.height
}

fn main() {
    let origin = Point { x: 0.0, y: 0.0 }
    let p = Point { x: 3.0, y: 4.0 }

    let dist = point_distance(origin, p)
    print(dist)

    let rect = Rectangle { width: 5.0, height: 10.0 }
    let area = rectangle_area(rect)
    print(area)
}
