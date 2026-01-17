// Loops - Demonstrates for and while loops

fn sum_range(start: Int, end: Int) -> Int {
    let mut total = 0
    for i in start..end {
        total = total + i
    }
    total
}

fn count_down(n: Int) {
    let mut i = n
    while i > 0 {
        print(i)
        i = i - 1
    }
    print("Liftoff!")
}

fn main() {
    let sum = sum_range(1, 11)
    print("Sum from 1 to 10: " + sum)

    print("Counting down:")
    count_down(5)

    let numbers = [1, 2, 3, 4, 5]
    for num in numbers {
        print(num * 2)
    }
}
