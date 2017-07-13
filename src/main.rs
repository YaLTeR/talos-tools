#[macro_use]
extern crate lazy_static;
extern crate libc;
extern crate livesplit_core;
extern crate notify;
extern crate pancurses;
extern crate regex;
extern crate x11;

use std::thread;

mod center_mouse;
mod timer;

fn main() {
    thread::spawn(center_mouse::run);
    thread::spawn(timer::run).join().unwrap();
}
