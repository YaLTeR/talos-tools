use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;

use livesplit_core::{Run, Segment, SharedTimer, Timer, TimingMethod};
use livesplit_core::time_formatter::{self, Accuracy, TimeFormatter};
use notify::{RawEvent, RecursiveMode, Watcher, op, raw_watcher};
use pancurses::{endwin, initscr};
use regex::Regex;

const LOG_FILE: &str = "/mnt/hdd/Games/SteamLibraryLinux/steamapps/common/The Talos Principle/Log/Talos.log";

fn process_line(timer: &SharedTimer, line: &str) {
    lazy_static! {
        static ref STARTED_SIMULATION: Regex =
            Regex::new(r"Started simulation on '[^']+' in [\d.]+ seconds\.").unwrap();
        static ref STOPPING_SIMULATION: Regex =
            Regex::new(r"Stopping simulation \(duration: [\d.]+\)\.").unwrap();
    }

    if let Some(_) = STARTED_SIMULATION.find(line) {
        let mut timer = timer.write();
        timer.split();
        timer.initialize_game_time();
    } else if let Some(_) = STOPPING_SIMULATION.find(line) {
        timer.write().split();
    }
}

fn watch_log(timer: SharedTimer) {
    let log = File::open(LOG_FILE).unwrap();
    let mut log = BufReader::new(log);
    let mut line = String::new();
    {
        let mut temp = Vec::new();
        log.read_to_end(&mut temp).unwrap();
    }

    let (tx, rx) = channel();
    let mut watcher = raw_watcher(tx).unwrap();
    watcher.watch(LOG_FILE, RecursiveMode::NonRecursive)
           .unwrap();

    loop {
        match rx.recv() {
            Ok(RawEvent { op: Ok(op), .. }) => {
                if op != op::WRITE {
                    continue;
                }

                loop {
                    line.clear();
                    let length = log.read_line(&mut line).unwrap();
                    if length == 0 {
                        break;
                    }

                    process_line(&timer, &line);
                }
            }
            _ => {
                panic!("error receiving event");
            }
        }
    }
}

fn create_timer() -> Timer {
    let mut run = Run::new();
    run.set_game_name("The Talos Principle");
    run.set_category_name("Any% (60 FPS)");
    run.push_segment(Segment::new("The only split"));

    let mut timer = Timer::new(run);
    timer.set_current_timing_method(TimingMethod::GameTime);

    timer
}

pub fn run() {
    let timer = create_timer().into_shared();
    {
        let timer = timer.clone();
        thread::spawn(move || watch_log(timer));
    }

    let window = initscr();
    window.nodelay(true);

    let formatter = time_formatter::Regular::with_accuracy(Accuracy::Tenths);

    loop {
        if let Some(_) = window.getch() {
            break;
        }

        let time = timer.read().current_time();

        let y = window.get_cur_y();
        window.mv(y, 0);
        window.deleteln();
        window.printw(&format!("{}, phase: {:?}",
                               formatter.format(time.game_time),
                               timer.read().current_phase()));
        window.refresh();

        thread::sleep(Duration::from_millis(100));
    }

    endwin();
}
