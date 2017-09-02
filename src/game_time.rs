use chrono::Duration;
use livesplit_core::{SharedTimer, TimeSpan, TimerPhase, TimingMethod};
use process_list::get_process_list;
use read_process_memory::{CopyAddress, Pid, ProcessHandle, TryIntoProcessHandle};
use timer_crate;

pub struct GameTime {
    talos_pid: Option<Pid>,
    scheduler: timer_crate::Timer,
    thread: Option<timer_crate::Guard>,
}

impl GameTime {
    pub fn new() -> Self {
        Self {
            talos_pid: None,
            scheduler: timer_crate::Timer::new(),
            thread: None,
        }
    }

    pub fn start(&mut self, timer: SharedTimer) {
        let mut talos_process = self.talos_pid
                                    .and_then(|x| x.try_into_process_handle().ok());
        if talos_process.is_none() {
            let pid = get_talos_pid();
            if self.talos_pid != pid {
                self.talos_pid = pid;
                talos_process = self.talos_pid
                                    .and_then(|x| x.try_into_process_handle().ok());
            }
        }

        let mut was_loading;

        {
            let mut timer_ = timer.write();

            if talos_process.is_none() {
                timer_.set_current_timing_method(TimingMethod::RealTime);
                return;
            }

            let loading = is_loading(talos_process.as_ref().unwrap());
            if loading.is_none() {
                timer_.set_current_timing_method(TimingMethod::RealTime);
                return;
            }
            was_loading = loading.unwrap();

            timer_.set_current_timing_method(TimingMethod::GameTime);
            timer_.initialize_game_time();
            timer_.pause_game_time();
            timer_.set_game_time(TimeSpan::zero());
        }

        // Needed for correct handling of the intro cutscene.
        let mut first = true;

        self.thread = Some(self.scheduler.schedule_repeating(
            Duration::milliseconds(15),
            move || {
                // TODO: figure out a way to stop the repeating if an error occurrs.
                if talos_process.is_none() {
                    return;
                }

                if let Some(loading) = is_loading(talos_process.as_ref().unwrap()) {
                    if was_loading != loading {
                        was_loading = loading;

                        if loading {
                            let mut timer = timer.write();
                            if timer.current_phase() == TimerPhase::Running &&
                                timer.is_game_time_initialized()
                            {
                                timer.pause_game_time();
                            } else {
                                talos_process = None;
                            }
                        } else {
                            if !first {
                                let mut timer = timer.write();
                                if timer.current_phase() == TimerPhase::Running &&
                                    timer.is_game_time_initialized()
                                {
                                    timer.unpause_game_time();
                                } else {
                                    talos_process = None;
                                }
                            }

                            first = false;
                        }
                    }
                } else {
                    talos_process = None;
                }
            },
        ));
    }
}

fn get_talos_pid() -> Option<Pid> {
    let mut iter = get_process_list().into_iter()
                                     .filter(|&(_, ref v)| v.ends_with("/x64/Talos"));

    if let Some((pid, _)) = iter.next() {
        if iter.next().is_some() {
            None
        } else {
            Some(pid)
        }
    } else {
        None
    }
}

#[cfg(all(not(windows), not(target_os = "macos")))]
fn is_loading(process: &ProcessHandle) -> Option<bool> {
    const LOADING_POPUP_PTR: u64 = 0x2B2CAC0;
    const LOADING_WORLD_PTR: u64 = 0x2B961C0;
    const OFF: u64 = 0x58;

    let mut buf = [0u8; 8];
    if process.copy_address(LOADING_POPUP_PTR as usize, &mut buf)
              .is_err()
    {
        return None;
    }
    let ptr = unsafe { *(buf.as_ptr() as *const u64) };
    let mut buf = [0u8; 1];
    if process.copy_address((ptr + OFF) as usize, &mut buf)
              .is_err()
    {
        return None;
    }
    let load_popup = unsafe { *(buf.as_ptr() as *const u8) };
    if load_popup & 1 != 0 {
        return Some(true);
    }

    let mut buf = [0u8; 8];
    if process.copy_address(LOADING_WORLD_PTR as usize, &mut buf)
              .is_err()
    {
        return None;
    }
    let ptr = unsafe { *(buf.as_ptr() as *const u64) };
    let mut buf = [0u8; 1];
    if process.copy_address((ptr + OFF) as usize, &mut buf)
              .is_err()
    {
        return None;
    }
    let load_world = unsafe { *(buf.as_ptr() as *const u8) };
    if load_world & 1 != 0 {
        return Some(true);
    }

    Some(false)
}

#[cfg(not(all(not(windows), not(target_os = "macos"))))]
fn is_loading(_: &ProcessHandle) -> Option<bool> {
    None
}
