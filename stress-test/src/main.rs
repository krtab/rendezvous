use std::time::Instant;

trait BarrierLike: Clone + Send {
    fn wait(self);
}

impl BarrierLike for rendezvous::Rendezvous {
    fn wait(self) {
        rendezvous::Rendezvous::wait(self);
    }
}

impl BarrierLike for adaptive_barrier::Barrier {
    fn wait(mut self) {
        adaptive_barrier::Barrier::wait(&mut self);
    }
}

fn f(id: String, n_child: usize, rem_depth: usize, b: impl BarrierLike + 'static) {
    println!("{id}");
    if rem_depth == 0 {
        b.wait();
        return;
    }
    for i in 0..n_child {
        let id_child = format!("{id}-{i}");
        // let id_child = String::new();
        let b = b.clone();
        let _h = std::thread::spawn(move || f(id_child, n_child, rem_depth - 1, b));
    }
    drop(b)
}

fn g(id: String, n_child: usize, rem_depth: usize) {
    // println!("{id}");
    if rem_depth == 0 {
        return;
    }
    let mut handles = Vec::new();
    for i in 0..n_child {
        let id_child = format!("{id}-{i}");
        // let id_child = String::new();
        let h = std::thread::spawn(move || g(id_child, n_child, rem_depth - 1));
        handles.push(h);
    }
    for h in handles {
        h.join().unwrap();
    }
}

fn main() {
    const N_CHILD: usize = 2;
    const DEPTH: usize = 10;
    //
    let b = rendezvous::Rendezvous::new();
    let start = Instant::now();
    f("".into(), N_CHILD, DEPTH, b.clone());
    b.wait();
    let end = start.elapsed();
    eprintln!("rendez-vous: {}ms", end.as_millis());
    //
    let b = adaptive_barrier::Barrier::new(adaptive_barrier::PanicMode::Decrement);
    let start = Instant::now();
    f("".into(), N_CHILD, DEPTH, b.clone());
    b.wait();
    let end = start.elapsed();
    eprintln!("adaptive: {}ms", end.as_millis());
    //
    let start = Instant::now();
    g("".into(), N_CHILD, DEPTH);
    let end = start.elapsed();
    eprintln!("join: {}ms", end.as_millis());
}
