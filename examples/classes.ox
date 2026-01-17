// Classes - Demonstrates class definitions and methods

class Counter {
    count: Int,

    fn new() -> Self {
        Self { count: 0 }
    }

    fn increment(&mut self) {
        self.count = self.count + 1
    }

    fn decrement(&mut self) {
        self.count = self.count - 1
    }

    fn get(&self) -> Int {
        self.count
    }

    fn reset(&mut self) {
        self.count = 0
    }
}

class Rectangle {
    width: Float,
    height: Float,

    fn new(width: Float, height: Float) -> Self {
        Self { width, height }
    }

    fn area(&self) -> Float {
        self.width * self.height
    }

    fn perimeter(&self) -> Float {
        2.0 * (self.width + self.height)
    }
}

fn main() {
    let mut counter = Counter::new()
    counter.increment()
    counter.increment()
    print(counter.get())

    let rect = Rectangle::new(5.0, 3.0)
    print(rect.area())
    print(rect.perimeter())
}
