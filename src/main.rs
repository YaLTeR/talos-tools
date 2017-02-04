#![macro_use]
extern crate lazy_static;
extern crate libc;
extern crate x11;

use std::ffi::CString;
use std::{mem, ptr};
use x11::keysym;
use x11::xlib::*;

struct XlibStuff {
    pub display: *mut Display,
    pub root: Window,
    pub net_active_window: Atom,
    _h: ()
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

            let net_active_window = XInternAtom(display,
                                                CString::new("_NET_ACTIVE_WINDOW").unwrap().as_ptr(),
                                                True);

            Ok(XlibStuff {
                display: display,
                root: root,
                net_active_window: net_active_window,
                _h: ()
            })
        }
    }

    fn get_active_window(&self) -> Result<Option<Window>, String> {
        unsafe {
            let mut actual_type_return = mem::uninitialized::<Atom>();
            let mut actual_format_return = mem::uninitialized::<libc::c_int>();
            let mut nitems_return = mem::uninitialized::<libc::c_ulong>();
            let mut bytes_after_return = mem::uninitialized::<libc::c_ulong>();
            let mut data = mem::uninitialized::<*mut libc::c_uchar>();
            if XGetWindowProperty(self.display,
                                  self.root,
                                  self.net_active_window,
                                  0,
                                  !0,
                                  False,
                                  AnyPropertyType as u64,
                                  &mut actual_type_return,
                                  &mut actual_format_return,
                                  &mut nitems_return,
                                  &mut bytes_after_return,
                                  &mut data) != Success as i32 {
                return Err("XGetWindowProperty() did not return Success.".to_owned());
            }

            if data.is_null() {
                return Err("XGetWindowProperty() returned NULL as data.".to_owned());
            }

            let active_window = *(data as *mut Window);

            XFree(data as *mut std::os::raw::c_void);

            match active_window {
                0 => Ok(None),
                w => Ok(Some(w)),
            }
        }
    }

    fn warp_cursor_to_center(&self, window: Window) -> Result<(), String> {
        unsafe {
            let mut window_attributes = mem::uninitialized::<XWindowAttributes>();
            if XGetWindowAttributes(self.display, window, &mut window_attributes) == 0 {
                return Err("XGetWindowAttributes() returned 0.".to_owned());
            }

            //println!("Window w, h: {} {}",
            //         window_attributes.width,
            //         window_attributes.height);

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
}

impl Drop for XlibStuff {
    fn drop(&mut self) {
        unsafe {
            XCloseDisplay(self.display);
        }
    }
}

fn warp_cursor(x: &XlibStuff) -> Result<(), String> {
    let active_window = match try!(x.get_active_window()) {
        Some(w) => w,
        None => return Ok(()),
    };

    try!(x.warp_cursor_to_center(active_window));

    Ok(())
}

fn run_event_loop(x: &XlibStuff) -> Result<(), String> {
    unsafe {
        // This approach works but it blocks programs from using Escape for the most part.
        // Most annoyingly, it blocks usage of Escape in the Talos menu.
        // Also it doesn't work with modifiers (you have to have NumLock off for example).

        //if XGrabKey(x.display,
        //            XKeysymToKeycode(x.display, keysym::XK_Escape as u64) as i32,
        //            0,
        //            x.root,
        //            False,
        //            GrabModeAsync,
        //            GrabModeAsync) == BadAccess as i32 {
        //    return Err("XGrabKey() returned BadAccess.".to_owned());
        //}

        //XSelectInput(x.display, x.root, KeyPressMask);

        //loop {
        //    let mut ev = mem::uninitialized::<XEvent>();
        //    XNextEvent(x.display, &mut ev);

        //    match ev.get_type() {
        //        t if t == KeyPress => {
        //            try!(warp_cursor(&x));
        //        },
        //        _ => {}
        //    };
        //}

        // This sometimes gives BadWindow?
        // Also doesn't work with all windows (but does work with Talos).

        let mut current_focus_window = mem::uninitialized::<Window>();
        let mut revert_to_return = mem::uninitialized::<libc::c_int>();
        XGetInputFocus(x.display, &mut current_focus_window, &mut revert_to_return);
        XSelectInput(x.display, current_focus_window, KeyPressMask | FocusChangeMask);

        loop {
            let mut ev = mem::uninitialized::<XEvent>();
            XNextEvent(x.display, &mut ev);

            match ev.get_type() {
                t if t == FocusOut => {
                    if current_focus_window != x.root {
                        XSelectInput(x.display, current_focus_window, 0);
                    }

                    XGetInputFocus(x.display, &mut current_focus_window, &mut revert_to_return);

                    if current_focus_window == PointerRoot as u64 {
                        current_focus_window = x.root;
                    }

                    XSelectInput(x.display, current_focus_window, KeyPressMask | FocusChangeMask);
                },

                t if t == KeyPress => {
                    if XKeyEvent::from(ev).keycode == XKeysymToKeycode(x.display,
                                                                       keysym::XK_Escape as u64) as u32 {
                        try!(warp_cursor(&x));
                    }
                },

                _ => {}
            };
        }
    }
}

fn main() {
    let x = match XlibStuff::init() {
        Ok(x) => x,
        Err(err) => {
            println!("{}", err);
            return;
        }
    };

    if let Err(err) = run_event_loop(&x) {
        println!("{}", err);
    }
}
