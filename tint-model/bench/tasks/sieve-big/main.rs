use std::hint::black_box;

fn main() {
    let n: usize = black_box(3000000);
    let mut flags: Vec<bool> = Vec::new();
    for _ in 0..=n {
        flags.push(true);
    }
    let mut p = 2;
    while p * p <= n {
        if flags[p] {
            let mut j = p * p;
            while j <= n {
                flags[j] = false;
                j += p;
            }
        }
        p += 1;
    }
    let mut count = 0;
    for k in 2..=n {
        if flags[k] {
            count += 1;
        }
    }
    println!("{}", count);
}
