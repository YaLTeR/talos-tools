use std::collections::HashMap;
use std::os::raw::c_int;

#[cfg(all(not(windows), not(target_os = "macos")))]
pub fn get_process_list() -> HashMap<c_int, String> {
    use std::ffi::CStr;
    use procps_sys::readproc::{closeproc, openproc, proc_t, readproc, PROC_FILLCOM};

    let mut rv = HashMap::new();

    unsafe {
        let proctab = openproc(PROC_FILLCOM);

        let mut procinfo = proc_t::default();
        while !readproc(proctab, &mut procinfo).is_null() {
            if let Some(cmdline) = procinfo.cmdline
                                           .as_ref()
                                           .and_then(|&x| CStr::from_ptr(x).to_str().ok())
                                           .map(|x| x.to_owned())
            {
                rv.insert(procinfo.tid, cmdline);
            }
        }

        closeproc(proctab);
    }

    rv
}

#[cfg(not(all(not(windows), not(target_os = "macos"))))]
pub fn get_process_list() -> HashMap<c_int, String> {
    HashMap::new()
}
