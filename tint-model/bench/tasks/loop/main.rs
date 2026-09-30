use std::hint::black_box;

fn main() {
    let n: i64 = black_box(3_000_000);
    let (mut i, mut sum) = (0i64, 0i64);
    while i < n {
        sum += i % 7;
        i += 1;
    }
    println!("{}", sum);
}
