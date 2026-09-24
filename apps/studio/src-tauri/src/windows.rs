use std::io;
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    System::Threading::{GetCurrentProcessId, GetCurrentThreadId},
    UI::{
        Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::{EnumThreadWindows, GetClassNameW, GetWindowThreadProcessId, WM_CLOSE, WM_NCDESTROY},
    },
};

const EVENT_TARGET_CLASS: &str = "Tao Thread Event Target";
const CLOSE_GUARD_ID: usize = 0x4149534c;

struct EventTarget {
    process_id: u32,
    thread_id: u32,
    window: HWND,
    matches: usize,
}

pub fn protect_event_target() -> io::Result<()> {
    let mut target = EventTarget {
        process_id: unsafe { GetCurrentProcessId() },
        thread_id: unsafe { GetCurrentThreadId() },
        window: std::ptr::null_mut(),
        matches: 0,
    };
    if unsafe { EnumThreadWindows(target.thread_id, Some(find_event_target), &mut target as *mut EventTarget as LPARAM) } == 0 {
        return Err(io::Error::last_os_error());
    }
    if target.matches != 1 {
        return Err(io::Error::other("Expected one native event target on the Studio UI thread"));
    }
    if unsafe { SetWindowSubclass(target.window, Some(prevent_auxiliary_close), CLOSE_GUARD_ID, 0) } == 0 {
        return Err(io::Error::other("Could not protect the Studio native event target"));
    }
    Ok(())
}

unsafe extern "system" fn find_event_target(window: HWND, data: LPARAM) -> i32 {
    let target = &mut *(data as *mut EventTarget);
    let mut process_id = 0;
    let thread_id = GetWindowThreadProcessId(window, &mut process_id);
    if process_id != target.process_id || thread_id != target.thread_id {
        return 1;
    }
    let mut class_name = [0u16; 64];
    let length = GetClassNameW(window, class_name.as_mut_ptr(), class_name.len() as i32);
    if length == EVENT_TARGET_CLASS.len() as i32 && EVENT_TARGET_CLASS.encode_utf16().eq(class_name[..length as usize].iter().copied()) {
        target.window = window;
        target.matches = target.matches.saturating_add(1);
    }
    1
}

unsafe extern "system" fn prevent_auxiliary_close(window: HWND, message: u32, word: WPARAM, data: LPARAM, subclass_id: usize, _reference: usize) -> LRESULT {
    if message == WM_CLOSE {
        return 0;
    }
    if message == WM_NCDESTROY {
        RemoveWindowSubclass(window, Some(prevent_auxiliary_close), subclass_id);
    }
    DefSubclassProc(window, message, word, data)
}