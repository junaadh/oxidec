// Advanced - Demonstrates advanced features combined

protocol Comparable {
    fn cmp(&self, other: Self) -> Int
}

enum BinaryTree<T> {
    Leaf,
    Node { value: T, left: Box<BinaryTree<T>>, right: Box<BinaryTree<T>> },
}

impl<T: Comparable> BinaryTree<T> {
    fn new() -> Self {
        BinaryTree::Leaf
    }

    fn insert(&self, value: T) -> Self {
        match self {
            BinaryTree::Leaf => {
                BinaryTree::Node {
                    value,
                    left: Box::new(BinaryTree::Leaf),
                    right: Box::new(BinaryTree::Leaf),
                }
            },
            BinaryTree::Node { value: v, left, right } => {
                let cmp = value.cmp(*v)
                if cmp < 0 {
                    BinaryTree::Node {
                        value: *v,
                        left: Box::new(left.insert(value)),
                        right: right,
                    }
                } else {
                    BinaryTree::Node {
                        value: *v,
                        left: left,
                        right: Box::new(right.insert(value)),
                    }
                }
            },
        }
    }
}

struct IntWrapper {
    value: Int,
}

impl Comparable for IntWrapper {
    fn cmp(&self, other: Self) -> Int {
        if self.value < other.value {
            -1
        } else if self.value > other.value {
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
