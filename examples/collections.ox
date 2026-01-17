// Collections - Demonstrates arrays and dictionaries

fn sum_array(numbers: [Int]) -> Int {
    let mut total = 0
    for num in numbers {
        total = total + num
    }
    total
}

fn main() {
    // Arrays
    let numbers = [1, 2, 3, 4, 5]
    let sum = sum_array(numbers)
    print("Sum: " + sum)

    let first = numbers[0]
    let last = numbers[4]
    print("First: " + first)
    print("Last: " + last)

    // Dictionaries
    let scores = ["Alice": 95, "Bob": 87, "Charlie": 92]
    let alice_score = scores["Alice"]
    print("Alice's score: " + alice_score)

    // Mutable operations
    let mut mut_numbers = [10, 20, 30]
    mut_numbers[1] = 25
    print(mut_numbers[1])
}
