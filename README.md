[![Crates.io badge](https://img.shields.io/crates/v/rendezvous?style=flat-square)](https://crates.io/crates/rendezvous)
[![github release badge badge](https://img.shields.io/github/v/release/krtab/rendezvous?style=flat-square)](https://github.com/krtab/rendezvous/releases/latest)

Rendezvous is a futex and atomics based implementation of an adaptive barrier, also known as a wait group.

It is primarily intended as a pedagogical resource and training exercise, as benchmarks show that is does not outperform mutexes and condvar based implementations.

![Benchmark results](resources/lines.svg)