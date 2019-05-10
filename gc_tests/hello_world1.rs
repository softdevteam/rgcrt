#![feature(gc)]

/// run-time output:
/// Hello world

extern crate core;
use core::gc::Scan;

struct S {}

impl Scan for S {
    fn scan(&self) {
        println!("Hello world")
    }
}

fn main() {
    let s = S{};
    s.scan();
}
