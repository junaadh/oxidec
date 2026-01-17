// Generics - Demonstrates generic types and functions

struct Pair<T, U> {
    first: T,
    second: U,
}

fn swap<T, U>(p: Pair<T, U>) -> Pair<U, T> {
    Pair { first: p.second, second: p.first }
}

fn first<T, U>(p: Pair<T, U>) -> T {
    p.first
}

struct Box<T> {
    value: T,
}

impl<T> Box<T> {
    fn new(value: T) -> Self {
        Self { value }
    }

    fn get(&self) -> T {
        self.value
    }
}

fn main() {
    let pair = Pair { first: 1, second: "hello" }
    print(first(pair))

    let swapped = swap(pair)
    print(first(swapped))

    let boxed = Box::new(42)
    print(boxed.get())
}
