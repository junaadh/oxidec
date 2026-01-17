// Protocols - Demonstrates protocol definitions and conformance

protocol Display {
    fn to_string(&self) -> String
}

protocol Clone {
    fn clone(&self) -> Self
}

struct Point {
    x: Float,
    y: Float,
}

impl Display for Point {
    fn to_string(&self) -> String {
        "Point(" + self.x + ", " + self.y + ")"
    }
}

struct Counter {
    count: Int,
}

impl Clone for Counter {
    fn clone(&self) -> Self {
        Self { count: self.count }
    }
}

fn print_item<T: Display>(item: T) {
    print(item.to_string())
}

fn main() {
    let p = Point { x: 3.0, y: 4.0 }
    print_item(p)

    let c1 = Counter { count: 5 }
    let c2 = c1.clone()
    print(c2.count)
}
