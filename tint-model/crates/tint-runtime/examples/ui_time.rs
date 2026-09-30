//! `cargo run --release -p tint-runtime --example ui_time -- app-render.tn`
use std::time::Instant;
use tint_runtime::ui_session::UiSession;
#[inline(never)]
fn profiled_dispatch(s: &mut UiSession, name: &str) -> usize {
    s.dispatch(name).unwrap().len()
}

fn main() {
    let path = std::env::args().nth(1).unwrap();
    let src = std::fs::read_to_string(path).unwrap();
    let mut s = UiSession::new(&src, "App").unwrap();
    s.set_reuse(std::env::var("NOREUSE").is_err());
    let t = Instant::now();
    s.render().unwrap();
    println!("initial render {:?}", t.elapsed());
    let mut prev: Vec<std::rc::Rc<tint_runtime::ui::render::UiRenderNode>> = Vec::new();
    let names: Vec<String> = std::env::args().skip(2).collect();
    let list: Vec<(&str, u32)> = if names.is_empty() { vec![("la", 1), ("lb", 1), ("lb_upd", 1), ("lb_swap", 1), ("lb_rm", 1), ("l_append", 1)] } else { names.iter().map(|n| (n.as_str(), 1)).collect() };
    for (name, n) in list {
        let t = Instant::now();
        let mut last = 0;
        for _ in 0..n {
            let last_one = name == names.last().map(String::as_str).unwrap_or("") && std::env::var_os("PROFILE_LAST").is_some();
            let tree = if last_one { let _ = profiled_dispatch(&mut s, name); s.render().unwrap() } else { s.dispatch(name).unwrap() };
            last = tree.len();
            fn table(n: &[std::rc::Rc<tint_runtime::ui::render::UiRenderNode>]) -> Vec<std::rc::Rc<tint_runtime::ui::render::UiRenderNode>> {
                let mut stack: Vec<_> = n.to_vec();
                while let Some(x) = stack.pop() { if x.tag == "Table" { return x.children.clone(); } stack.extend(x.children.iter().cloned()); }
                vec![]
            }
            let t = table(&tree);
            let same = t.iter().filter(|x| prev.iter().any(|p| std::rc::Rc::ptr_eq(p, x))).count();
            eprintln!("  {name}: table rows {} ptr-shared with prev {}", t.len(), same);
            eprintln!("  purple rows: {}", t.iter().filter(|x| x.style.iter().any(|(_, v)| v == "#6c5ce7")).count());
            eprintln!("  keys: {:?}", t.iter().take(2).map(|x| x.key.clone()).collect::<Vec<_>>());
            prev = t;
        }
        println!("{name}: {:?} per dispatch (roots {last})", t.elapsed() / n);
    }
}
