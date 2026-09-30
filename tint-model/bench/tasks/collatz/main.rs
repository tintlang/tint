use std::hint::black_box;

fn steps(n: i64) -> i64 {
    let (mut x, mut c) = (n, 0);
    while x != 1 {
        if x % 2 == 0 {
            x /= 2;
        } else {
            x = 3 * x + 1;
        }
        c += 1;
    }
    c
}

fn main() {
    let limit: i64 = black_box(20000);
    let (mut best, mut best_n) = (0, 0);
    for n in 1..limit {
        let s = steps(n);
        if s > best {
            best = s;
            best_n = n;
        }
    }
    println!("{}", best_n);
    println!("{}", best);
}
