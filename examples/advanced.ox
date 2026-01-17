// Advanced - Demonstrates advanced features combined

protocol Comparable {
    fn cmp(other: Self) -> Int
}

enum BinaryTree<T> {
    case leaf,
    case node(value: T, left: Box<BinaryTree<T>>, right: Box<BinaryTree<T>>),
}

impl<T: Comparable> BinaryTree<T> {
    fn new() -> Self {
        .leaf
    }

    fn insert(value: T) -> Self {
        match self {
            .leaf => {
                .node(
                    value: value,
                    left: Box::new(.leaf),
                    right: Box::new(.leaf),
                )
            },
            .node(value: v, left, right) => {
                let cmp = value.cmp(v)
                if cmp < 0 {
                    .node(
                        value: v,
                        left: Box::new(left.insert(value)),
                        right: right,
                    )
                } else {
                    .node(
                        value: v,
                        left: left,
                        right: Box::new(right.insert(value)),
                    )
                }
            },
        }
    }
}

struct IntWrapper {
    value: Int,
}

impl Comparable for IntWrapper {
    fn cmp(other: Self) -> Int {
        if value < other.value {
            -1
        } else if value > other.value {
            1
        } else {
            0
        }
    }
}

fn main() {
    let tree = BinaryTree::new()
    let tree = tree.insert(IntWrapper { value: 5 })
    let tree = tree.insert(IntWrapper { value: 3 })
    let tree = tree.insert(IntWrapper { value: 7 })

    print("Binary tree constructed")
}
