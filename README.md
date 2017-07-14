# talos-tools

Some speedrun-related stuff for the Linux version of The Talos Principle:

- Resetting the mouse to the center of the current window whenever Escape is pressed. This is to compensate for mouse not resetting to center upon opening the menu in Linux or OSX as it does on Windows. This is pretty much essential.
- A LiveSplit-like console timer with autosplitting and load removal.

### Running
Get Rust and do `cargo run --release`.

For the timer to work, prepare the splits file in LiveSplit `.lss` format, and run the program like this: `cargo run --release <path/to/Talos.log> <path/to/splits.lss>`.

### Usage
In center mouse-only mode Ctrl-C exits the program.

In the timer mode pressing Delete resets the splits without saving golds, and pressing any other key exits the program.
