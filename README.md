# talos-tools

Some speedrun-related stuff for the Linux version of The Talos Principle:

- Resetting the mouse to the center of the current window whenever Escape is pressed. This is to compensate for mouse not resetting to center upon opening the menu in Linux or OSX as it does on Windows. This is pretty much essential.
- A LiveSplit-like console timer with autosplitting and load removal.

### Running
Get Rust and do `cargo run --release`.

For the timer to work, prepare the splits file in LiveSplit `.lss` format, and run the program like this: `cargo run --release <path/to/Talos.log> <path/to/splits.lss>`.

The load removal is currently supported only on **64-bit Linux Talos**, and requires elevated permissions for reading memory of the Talos process. Run `cargo build --release`, followed by `sudo target/release/talos-tools <path/to/Talos.log> <path/to/splits.lss>`. If the load removal fails to work the timer will fall back to RTA timing.

### Usage
In center mouse-only mode Ctrl-C exits the program.

In the timer mode pressing Delete resets the splits without saving golds, and pressing any other key exits the program.
