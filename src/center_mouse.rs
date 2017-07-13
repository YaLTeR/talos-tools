use std::ffi::CStr;
use std::{self, mem, ptr, thread, time};

use libc;
use x11::keysym;
use x11::xlib::*;

struct XlibStuff {
    pub display: *mut Display,
    pub root: Window,
    _h: (),
}

impl XlibStuff {
    fn init() -> Result<Self, String> {
        unsafe {
            let display = XOpenDisplay(ptr::null());
            if display.is_null() {
                return Err("XOpenDisplay() returned NULL.".to_owned());
            }

            let screen = XDefaultScreen(display);
            let root = XRootWindow(display, screen);

            Ok(XlibStuff {
                   display: display,
                   root: root,
                   _h: (),
               })
        }
    }

    fn warp_cursor_to_center(&self, window: Window) -> Result<(), String> {
        unsafe {
            let mut window_attributes = mem::uninitialized::<XWindowAttributes>();
            if XGetWindowAttributes(self.display, window, &mut window_attributes) == 0 {
                return Err("XGetWindowAttributes() returned 0.".to_owned());
            }

            XWarpPointer(self.display,
                         window,
                         window,
                         0,
                         0,
                         0,
                         0,
                         window_attributes.width / 2,
                         window_attributes.height / 2);
        }

        Ok(())
    }

    fn get_window_by_name(&self, name: &str, current: Window) -> Option<Window> {
        unsafe {
            let mut window_name = mem::uninitialized::<*mut libc::c_char>();

            if XFetchName(self.display, current, &mut window_name) != 0 {
                if let Ok(window_name_str) = CStr::from_ptr(window_name).to_str() {
                    if window_name_str.starts_with("Talos - Linux") {
                        XFree(window_name as *mut std::os::raw::c_void);

                        return Some(current);
                    }
                }

                XFree(window_name as *mut std::os::raw::c_void);
            }

            let mut root_return = mem::uninitialized::<Window>();
            let mut parent_return = mem::uninitialized::<Window>();
            let mut children = mem::uninitialized::<*mut Window>();
            let mut children_count = mem::uninitialized::<libc::c_uint>();

            if XQueryTree(self.display,
                          current,
                          &mut root_return,
                          &mut parent_return,
                          &mut children,
                          &mut children_count) != 0
            {
                for i in 0..children_count {
                    if let Some(window) =
                        self.get_window_by_name(name, *children.offset(i as isize))
                    {
                        XFree(children as *mut std::os::raw::c_void);

                        return Some(window);
                    }
                }

                XFree(children as *mut std::os::raw::c_void);
            }

            None
        }
    }
}

impl Drop for XlibStuff {
    fn drop(&mut self) {
        unsafe {
            XCloseDisplay(self.display);
        }
    }
}

fn run_event_loop(x: &XlibStuff) {
    unsafe {
        loop {
            if let Some(talos_window) = x.get_window_by_name("Talos - Linux", x.root) {
                // println!("Found the Talos window.");

                XSelectInput(x.display, talos_window, KeyPressMask | StructureNotifyMask);
                loop {
                    let mut ev = mem::uninitialized::<XEvent>();
                    XNextEvent(x.display, &mut ev);

                    match ev.get_type() {
                        t if t == KeyPress => {
                            if XKeyEvent::from(ev).keycode ==
                                XKeysymToKeycode(x.display, keysym::XK_Escape as u64) as u32
                            {
                                if x.warp_cursor_to_center(talos_window).is_err() {
                                    break;
                                }
                            }
                        }

                        t if t == DestroyNotify => {
                            // println!("The Talos window was closed.");
                            break;
                        }

                        _ => {}
                    }
                }
            }

            // Wait for Talos to be opened.
            thread::sleep(time::Duration::from_secs(1));
        }
    }
}

pub fn run() {
    let x = match XlibStuff::init() {
        Ok(x) => x,
        Err(err) => {
            // TODO: handle this error better.
            panic!("{}", err);
            // return;
        }
    };

    run_event_loop(&x);
}
