// Classes - Demonstrates class definitions, methods, init, and static methods

class Counter {
    count: Int,

    // Swift-style initializer - allows Counter() construction
    init() {
        count = 0
    }

    init(startingAt count: Int) {
        self.count = count
    }

    // Immutable method - self is implicit
    fn get() -> Int {
        count
    }

    // Mutable method - uses mut fn keyword
    mut fn increment() {
        count = count + 1
    }

    mut fn decrement() {
        count = count - 1
    }

    mut fn reset() {
        count = 0
    }
}

// impl block for static methods
impl Counter {
    // Factory method pattern - requires Counter::new()
    static fn new() -> Self {
        Self { count: 0 }
    }

    static fn withCount(count: Int) -> Self {
        Self { count }
    }
}

class Rectangle {
    width: Float,
    height: Float,

    init(width: Float, height: Float) {
        self.width = width
        self.height = height
    }

    fn area() -> Float {
        width * height
    }

    fn perimeter() -> Float {
        2.0 * (width + height)
    }
}

impl Rectangle {
    static fn square(size: Float) -> Self {
        Self { width: size, height: size }
    }
}

fn main() {
    // Using init() - Swift-style construction
    let mut counter1 = Counter()
    counter1.increment()
    counter1.increment()
    print(counter1.get())

    // Using init with parameters
    let counter2 = Counter(startingAt: 10)
    print(counter2.get())

    // Using static fn new() - factory method pattern
    let counter3 = Counter::new()
    let counter4 = Counter::withCount(count: 15)

    print(counter3.get())
    print(counter4.get())

    // Using Rectangle init
    let rect = Rectangle(width: 5.0, height: 3.0)
    print(rect.area())
    print(rect.perimeter())

    // Using static factory method
    let square = Rectangle::square(size: 4.0)
    print(square.area())
}
