use std::{env, thread};
use std::borrow::Cow;
use std::cmp::{max, min};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read};
use std::sync::Arc;
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::time::Duration;
use std::thread::JoinHandle;

use errors::*;
use config::*;
use game_time::GameTime;
use livesplit_core::{component, GeneralLayoutSettings, SharedTimer, TimeSpan, Timer, TimerPhase};
use livesplit_core::run::{parser, saver};
use livesplit_core::settings::SemanticColor;
use notify::{op, raw_watcher, RawEvent, RecursiveMode, Watcher};
use pancurses;
use regex::Regex;

enum ArgumentPosition {
    TalosLogFilename = 1,
    SplitsFilename = 2,
}

struct GameState {
    game_time: GameTime,
    current_world: Option<String>,
}

impl GameState {
    fn new() -> Self {
        Self {
            game_time: GameTime::new(),
            current_world: None,
        }
    }
}

fn save_splits(timer: &Timer) -> Result<()> {
    let splits_filename = env::args().nth(ArgumentPosition::SplitsFilename as usize)
                                     .ok_or("the splits filename argument is missing")?;

    saver::livesplit::save(timer.run(),
                           File::create(splits_filename)
                               .chain_err(|| "could not open the splits file for writing")?)
    .chain_err(|| "could not save the splits")?;

    Ok(())
}

fn process_line(config: &Config,
                timer: &SharedTimer,
                state: &mut GameState,
                line: &str)
                -> Result<()> {
    lazy_static! {
        static ref CHANGING_OVER_TO: Regex =
            Regex::new(r"Changing over to (.+)").unwrap();
        static ref PUZZLE_SOLVED: Regex =
            Regex::new(r#"Puzzle "[^"]+" solved"#).unwrap();
    }

    if let Some(caps) = CHANGING_OVER_TO.captures(line) {
        // Splitting on returning to Nexus.
        let world_name = caps.get(1).unwrap().as_str();
        if config.split_on_return_to_nexus && world_name == "Content/Talos/Levels/Nexus.wld" {
            let mut timer = timer.write();
            if timer.current_phase() == TimerPhase::Running {
                if state.current_world.as_ref().unwrap() != world_name {
                    timer.split();
                }
            }
        }

        state.current_world = Some(world_name.to_string());
    } else if line.contains("Picked:") {
        // Splitting on tetromino and star pickups.
        if config.split_on_sigil_collection.in_general {
            let world_name = state.current_world.as_ref().unwrap();

            let mut split = true;
            if world_name == "Content/Talos/Levels/Cloud_1_06.wld" {
                if !config.split_on_sigil_collection.in_a6 {
                    split = false;
                }
            } else if world_name == "Content/Talos/Levels/Cloud_2_04.wld" {
                if !config.split_on_sigil_collection.in_b4 {
                    split = false;
                }
            }

            if split {
                timer.write().split();
            }
        }
    } else if PUZZLE_SOLVED.is_match(line) {
        // Splitting on tetromino puzzles.
        if config.split_on_tetromino_doors {
            timer.write().split();
        }
    } else if line.contains("Started simulation on 'Content/Talos/Levels/Cloud_1_01.wld'") {
        // Starting the timer.
        let mut timer_ = timer.write();
        if timer_.current_phase() == TimerPhase::NotRunning {
            timer_.start();

            // Try starting the game time.
            drop(timer_);
            state.game_time.start(timer.clone());
        }

        state.current_world = Some("Content/Talos/Levels/Cloud_1_01.wld".to_string());
    } else if line.contains("Save Talos Progress: delayed request") {
        // Resuming the game time on intro cutscene finish.
        let mut timer = timer.write();
        if timer.current_phase() == TimerPhase::Running && timer.is_game_time_initialized() &&
            timer.current_time().game_time.unwrap() == TimeSpan::zero()
        {
            timer.unpause_game_time();
        }
    } else if line.contains("Stopping simulation (duration: ") {
        // Resetting.
        let mut timer = timer.write();
        timer.reset(true);
        save_splits(&timer)?;

        state.current_world = None;
    } else if line.contains("USER: /eternalize") || line.contains("USER: /transcend") {
        // Splitting on game end.
        if state.current_world
                .as_ref()
                .map(|world| world == "Content/Talos/Levels/Nexus.wld")
                .unwrap_or(false)
        {
            timer.write().split();
        }
    } else if line.contains("USER:") {
        // Splitting on game end.
        if state.current_world
                .as_ref()
                .map(|world| world == "Content/Talos/Levels/Islands_03.wld")
                .unwrap_or(false)
        {
            timer.write().split();
        }
    }

    Ok(())
}

fn watch_log(config: &Config, timer: SharedTimer) -> Result<()> {
    let log_filename = env::args().nth(ArgumentPosition::TalosLogFilename as usize)
                                  .ok_or("the log filename argument is missing")?;
    let log = OpenOptions::new().read(true)
                                .write(true)
                                .create(true)
                                .truncate(true)
                                .open(&log_filename)
                                .chain_err(|| "could not open the Talos log file")?;
    let mut log = BufReader::new(log);
    let mut line = String::new();
    {
        let mut temp = Vec::new();
        log.read_to_end(&mut temp)
           .chain_err(|| "error reading the Talos log file")?;
    }

    let (tx, rx) = channel();
    let mut watcher = raw_watcher(tx).chain_err(|| "could not create a filesystem watcher")?;
    watcher.watch(&log_filename, RecursiveMode::NonRecursive)
           .chain_err(|| "could not set up the filesystem watcher on the Talos log file")?;

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
                    let length = log.read_line(&mut line)
                                    .chain_err(|| "error reading the Talos log file")?;
                    if length == 0 {
                        break;
                    }

                    process_line(config, &timer, &mut state, &line)?;
                }
            }
            _ => {
                panic!("error receiving event");
            }
        }
    }
}

fn watch_log_thread(watch_to_main_tx: Sender<Error>, config: Arc<Config>, timer: SharedTimer) {
    if let Err(e) = watch_log(&config, timer) {
        watch_to_main_tx.send(e).unwrap();
    }
}

fn create_timer() -> Result<Timer> {
    let splits_filename = env::args().nth(ArgumentPosition::SplitsFilename as usize)
                                     .ok_or("the splits filename argument is missing")?;
    let splits = File::open(splits_filename).chain_err(|| "could not open the splits file")?;
    let run =
        parser::livesplit::parse(splits, None).chain_err(|| "could not parse the splits file")?;

    Timer::new(run).chain_err(|| "could not create the Timer")
}

fn init_curses_colors() {
    pancurses::init_pair(SemanticColor::Default as i16, -1, -1);
    pancurses::init_pair(SemanticColor::AheadGainingTime as i16,
                         pancurses::COLOR_GREEN,
                         -1);
    pancurses::init_pair(SemanticColor::AheadLosingTime as i16,
                         pancurses::COLOR_GREEN,
                         -1);
    pancurses::init_pair(SemanticColor::BehindGainingTime as i16,
                         pancurses::COLOR_RED,
                         -1);
    pancurses::init_pair(SemanticColor::BehindLosingTime as i16,
                         pancurses::COLOR_RED,
                         -1);
    pancurses::init_pair(SemanticColor::BestSegment as i16,
                         pancurses::COLOR_YELLOW,
                         -1);
    pancurses::init_pair(SemanticColor::NotRunning as i16, -1, -1);
    pancurses::init_pair(SemanticColor::Paused as i16, -1, -1);
    pancurses::init_pair(SemanticColor::PersonalBest as i16,
                         pancurses::COLOR_BLUE,
                         -1);
}

fn truncate_string(string: &str, length: usize) -> Cow<str> {
    if length < 4 {
        panic!("length must be >= 4");
    }

    if string.len() <= length {
        Cow::Borrowed(string)
    } else {
        Cow::Owned(format!("{:.1$}...", string, length - 3))
    }
}

fn draw_title(window: &pancurses::Window, width: usize, title_state: component::title::State) {
    window.color_set(SemanticColor::Default as i16);
    window.printw(&format!("{:^1$.1$}", title_state.line1, width));
    window.printw(&format!("{:^1$.1$}", title_state.line2.unwrap(), width));
    let attempts = format!("{}", title_state.attempts.unwrap());
    window.mv(1, (width - attempts.len()) as i32);
    window.printw(&attempts);
}

fn draw_split(window: &pancurses::Window, width: usize, split: &component::splits::SplitState) {
    const TIME_WIDTH: usize = 6;
    let width = max(width, TIME_WIDTH + 4);
    let split_name_width = width - TIME_WIDTH * 2 - 2;

    let split_name_truncated = truncate_string(&split.name, split_name_width);
    window.color_set(SemanticColor::Default as i16);
    window.printw(&format!("{:1$.1$} ", split_name_truncated, split_name_width));

    window.color_set(split.semantic_color as i16);
    window.printw(&format!("{:>1$.1$} ", split.delta, TIME_WIDTH));

    window.color_set(SemanticColor::Default as i16);
    window.printw(&format!("{:>1$.1$}", split.time, TIME_WIDTH));
}

fn draw_splits(window: &pancurses::Window, width: usize, splits_state: component::splits::State) {
    for split in &splits_state.splits[0..splits_state.splits.len() - 1] {
        draw_split(window, width, split);
    }

    window.color_set(SemanticColor::Default as i16);
    window.printw(&format!("{:-<1$}", "", width));

    draw_split(window,
               width,
               &splits_state.splits[splits_state.splits.len() - 1]);
}

fn draw_timer(window: &pancurses::Window, width: usize, timer_state: component::timer::State) {
    window.color_set(timer_state.semantic_color as i16);
    window.printw(&format!("{:^1$.1$}",
                           &format!("{}{}", timer_state.time, timer_state.fraction),
                           width));
}

fn draw_prev_segment(window: &pancurses::Window,
                     width: usize,
                     prev_seg_state: component::previous_segment::State) {
    window.color_set(SemanticColor::Default as i16);
    window.printw(&prev_seg_state.text);

    window.color_set(prev_seg_state.semantic_color as i16);
    window.printw(&format!("{:>1$.1$}",
                           &prev_seg_state.time,
                           width - min(width, window.get_cur_x() as usize)));
}

fn draw_sum_of_best(window: &pancurses::Window,
                    width: usize,
                    sob_state: component::sum_of_best::State) {
    let y = window.get_cur_y();

    window.color_set(SemanticColor::Default as i16);
    window.printw("Sum of Best");
    window.mv(y, (width - sob_state.time.len()) as i32);
    window.printw(&sob_state.time);
}

fn main_loop(timer: SharedTimer,
             window: &pancurses::Window,
             watch_to_main_rx: Receiver<Error>,
             watch_thread: JoinHandle<()>)
             -> Result<()> {
    let mut title_component = component::title::Component::new();
    let timer_component = component::timer::Component::new();
    let sob_component = component::sum_of_best::Component::new();
    let prev_seg_component = component::previous_segment::Component::new();
    let mut splits_component = component::splits::Component::new();
    {
        let settings = splits_component.settings_mut();
        settings.always_show_last_split = true;
        settings.separator_last_split = true;
        settings.split_preview_count = 1;
    }

    loop {
        match watch_to_main_rx.try_recv() {
            Ok(e) => return Err(e),
            Err(TryRecvError::Disconnected) => {
                bail!("watch thread channel disconnected: {:?}",
                      watch_thread.join()
                                  .err()
                                  .and_then(|x| x.downcast_ref::<String>().cloned()))
            }
            Err(TryRecvError::Empty) => {}
        }

        if let Some(input) = window.getch() {
            match input {
                pancurses::Input::KeyResize => {}
                pancurses::Input::KeyDC => {
                    // Delete key: reset without saving golds.
                    let mut timer = timer.write();
                    timer.reset(false);
                    save_splits(&timer)?;
                }
                _ => break,
            }
        }

        splits_component.settings_mut().visual_split_count =
            max(window.get_max_y() as usize, 7) - 6;

        let timer = timer.read();
        let title_state = title_component.state(&timer);
        let timer_state = timer_component.state(&timer, &GeneralLayoutSettings::default());
        let splits_state = splits_component.state(&timer, &GeneralLayoutSettings::default());
        let sob_state = sob_component.state(&timer);
        let prev_seg_state = prev_seg_component.state(&timer, &GeneralLayoutSettings::default());
        drop(timer);

        let width = window.get_max_x() as usize;

        // Clear the contents.
        window.clear();
        window.mv(0, 0);

        // Draw the title.
        draw_title(window, width, title_state);

        // Draw the splits.
        draw_splits(window, width, splits_state);

        // Draw the timer.
        draw_timer(window, width, timer_state);

        // Draw previous segment.
        draw_prev_segment(window, width, prev_seg_state);

        // Draw sum of best.
        draw_sum_of_best(window, width, sob_state);

        window.refresh();

        thread::sleep(Duration::from_millis(10));
    }

    Ok(())
}

pub fn run() -> Result<()> {
    let config = read_config().chain_err(|| "failed to read config")
                              .map(Arc::new)?;

    let (watch_to_main_tx, watch_to_main_rx) = channel();

    let timer = create_timer()?.into_shared();
    let watch_thread = {
        let config = config.clone();
        let timer = timer.clone();
        thread::spawn(move || watch_log_thread(watch_to_main_tx, config, timer))
    };

    let window = pancurses::initscr();
    window.nodelay(true);
    window.keypad(true);
    pancurses::curs_set(0);
    pancurses::start_color();
    pancurses::use_default_colors();
    init_curses_colors();

    let result = main_loop(timer, &window, watch_to_main_rx, watch_thread);

    pancurses::endwin();

    result
}
