struct Tree {
    l: Option<Box<Tree>>,
    r: Option<Box<Tree>>,
}

fn make(d: u32) -> Option<Box<Tree>> {
    if d == 0 {
        return None;
    }
    Some(Box::new(Tree { l: make(d - 1), r: make(d - 1) }))
}

fn check(t: &Option<Box<Tree>>) -> u64 {
    match t {
        None => 1,
        Some(n) => 1 + check(&n.l) + check(&n.r),
    }
}

fn main() {
    let mut total = 0u64;
    for _ in 0..std::hint::black_box(20) {
        total += check(&make(16));
    }
    println!("{total}");
}
