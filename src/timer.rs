use std::{env, thread};
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::sync::mpsc::channel;
use std::time::Duration;

use errors::*;
use livesplit_core::{Color, SharedTimer, TimeSpan, Timer, TimerPhase, TimingMethod, component,
                     parser};
use notify::{RawEvent, RecursiveMode, Watcher, op, raw_watcher};
use pancurses;
use regex::Regex;

const LOG_FILE: &str = "/mnt/hdd/Games/SteamLibraryLinux/steamapps/common/The Talos Principle/Log/Talos.log";

struct GameState {
    current_world: Option<String>,
}

impl GameState {
    fn new() -> Self {
        Self {
            current_world: None,
        }
    }
}

fn process_line(timer: &SharedTimer, state: &mut GameState, line: &str) {
    lazy_static! {
        static ref STARTED_LOADING_WORLD: Regex =
            Regex::new(r#"Started loading world "([^"]+)""#).unwrap();
        static ref STARTED_SIMULATION: Regex =
            Regex::new(r"Started simulation on '([^']+)' in ([\d.]+) seconds\.").unwrap();
        static ref STOPPING_SIMULATION: Regex =
            Regex::new(r"Stopping simulation \(duration: [\d.]+\)\.").unwrap();
        static ref PLAYER_PROFILE_SAVED: Regex =
            Regex::new(r"Player profile saved").unwrap();
    }

    if let Some(caps) = STARTED_LOADING_WORLD.captures(line) {
        let world_name = caps.get(1).unwrap().as_str();

        let mut timer = timer.write();
        if timer.current_phase() == TimerPhase::Running {
            timer.pause_game_time();

            if state.current_world.as_ref().unwrap() != world_name {
                timer.split();
            }
        }

        state.current_world = Some(world_name.to_string());
    } else if let Some(caps) = STARTED_SIMULATION.captures(line) {
        let load_time = caps.get(2).unwrap().as_str().parse::<f64>().unwrap();

        let mut timer = timer.write();
        if timer.current_phase() == TimerPhase::Running {
            // Unpause the game time and update the loading times with the precise value.
            let loading_times = timer.loading_times();
            timer.unpause_game_time();
            timer.set_loading_times(loading_times + TimeSpan::from_seconds(load_time));
        }
    } else if PLAYER_PROFILE_SAVED.is_match(line) {
        // Handle autostart.
        if let Some(current_world) = state.current_world.as_ref() {
            if current_world == "Content/Talos/Levels/Cloud_1_01.wld" {
                let mut timer = timer.write();
                if timer.current_phase() == TimerPhase::NotRunning {
                    timer.split();
                }
            }
        }
    } else if STOPPING_SIMULATION.is_match(line) {
        // Handle autoreset.
        timer.write().reset(true);
        state.current_world = None;
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

    // TODO: perhaps update the state from the existing log?
    let mut state = GameState::new();

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

                    process_line(&timer, &mut state, &line);
                }
            }
            _ => {
                panic!("error receiving event");
            }
        }
    }
}

fn create_timer() -> Result<Timer> {
    let splits_filename = env::args()
        .nth(1)
        .ok_or("the splits filename argument is missing")?;
    let splits = File::open(splits_filename)
        .chain_err(|| "could not open the splits file")?;
    let run = parser::livesplit::parse(splits, None)
        .chain_err(|| "could not parse the splits file")?;

    let mut timer = Timer::new(run);
    timer.set_current_timing_method(TimingMethod::GameTime);

    Ok(timer)
}

fn init_curses_colors() {
    pancurses::init_pair(Color::Default as i16, -1, -1);
    pancurses::init_pair(Color::AheadGainingTime as i16, pancurses::COLOR_GREEN, -1);
    pancurses::init_pair(Color::AheadLosingTime as i16, pancurses::COLOR_GREEN, -1);
    pancurses::init_pair(Color::BehindGainingTime as i16, pancurses::COLOR_RED, -1);
    pancurses::init_pair(Color::BehindLosingTime as i16, pancurses::COLOR_RED, -1);
    pancurses::init_pair(Color::BestSegment as i16, pancurses::COLOR_YELLOW, -1);
    pancurses::init_pair(Color::NotRunning as i16, -1, -1);
    pancurses::init_pair(Color::Paused as i16, -1, -1);
    pancurses::init_pair(Color::PersonalBest as i16, pancurses::COLOR_BLUE, -1);
}

pub fn run() -> Result<()> {
    let timer = create_timer()?.into_shared();
    {
        let timer = timer.clone();
        thread::spawn(move || watch_log(timer));
    }

    let window = pancurses::initscr();
    window.nodelay(true);
    pancurses::start_color();
    pancurses::use_default_colors();
    init_curses_colors();

    let timer_component = component::timer::Component::new();

    loop {
        if let Some(_) = window.getch() {
            break;
        }

        let state = timer_component.state(&timer.read());

        let y = window.get_cur_y();
        window.mv(y, 0);
        window.deleteln();
        window.color_set(state.color as i16);
        window.printw(&format!("{}{}", state.time, state.fraction));
        window.color_set(Color::Default as i16);
        window.refresh();

        thread::sleep(Duration::from_millis(10));
    }

    pancurses::endwin();

    Ok(())
}
