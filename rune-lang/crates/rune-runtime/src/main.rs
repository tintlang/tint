use rune_runtime::state::store::StateStore;

fn main() {
    let mut store = StateStore::new();

    // --- STATE ---
    let a = store.create_state(10);
    let b = store.create_state(20);

    println!("a = {}", store.get_state::<i32>(a));
    println!("b = {}", store.get_state::<i32>(b));

    // --- COMPUTED ---
    let sum = store.create_computed(vec![a, b], move |s| {
        let x = s.get_state::<i32>(a);
        let y = s.get_state::<i32>(b);
        x + y
    });

    println!("sum before = {}", store.get_computed::<i32>(sum));

    // --- WATCHER ---
    store.watch(a, Box::new(|| {
        println!("watcher fired");
    }));

    // UPDATE
    store.set_state(a, 15);
    store.flush();

    println!("sum after = {}", store.get_computed::<i32>(sum));
}
