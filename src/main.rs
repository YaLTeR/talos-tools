#[macro_use]
extern crate error_chain;
#[macro_use]
extern crate lazy_static;
extern crate libc;
extern crate livesplit_core;
extern crate notify;
extern crate pancurses;
extern crate regex;
extern crate x11;

use std::{env, thread};

use error_chain::ChainedError;

mod errors {
    error_chain!{}
}

mod center_mouse;
mod timer;

fn usage() {
    println!("Usage: {} <path/to/Talos.log> <path/to/splits.lss>",
             env::args().nth(0).unwrap());
}

fn main() {
    let center_mouse_thread = thread::spawn(center_mouse::run);

    if let Err(ref e) = timer::run() {
        println!("{}", e.display());
        usage();

        println!("\nContinuing in center mouse mode.");
        center_mouse_thread.join().unwrap();
    }
}
