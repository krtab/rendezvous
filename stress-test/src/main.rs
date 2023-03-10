fn f(id: String, n_child: usize, rem_depth: usize, b : rendezvous::Rendezvous) {
    println!("{id}");
    if rem_depth == 0 {
        b.wait();
        return;
    }
    for i in 0..n_child {
        let id_child = format!("{id}-{i}");
        let b = b.clone();
        let _h = std::thread::spawn(move || f(id_child, n_child, rem_depth - 1, b));
        // handles.push(h);
    }
    std::mem::drop(b.clone());
    b.wait();
}

fn main() {
    let b = rendezvous::Rendezvous::new();
    f("".into(),2,5, b);
}
