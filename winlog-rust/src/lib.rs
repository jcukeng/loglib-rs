//! # winlog-rs
//!
//! Простая библиотека для записи в Windows Event Log.
//! Если кастомный источник не зарегистрирован — использует "Application" с префиксом.

use windows_sys::core::PCSTR;
use windows_sys::Win32::Foundation::{HANDLE, HMODULE, PSID};
use windows_sys::Win32::System::EventLog::{
    DeregisterEventSource,
    RegisterEventSourceA,
    ReportEventA,
    EVENTLOG_ERROR_TYPE,
    EVENTLOG_WARNING_TYPE,
    EVENTLOG_INFORMATION_TYPE,
};
use std::ffi::CString;

#[derive(Debug, Clone, Copy)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warning,
    Error,
    Fatal,
}

impl LogLevel {
    fn to_event_type(self) -> u16 {
        match self {
            LogLevel::Trace | LogLevel::Debug | LogLevel::Info => EVENTLOG_INFORMATION_TYPE,
            LogLevel::Warning => EVENTLOG_WARNING_TYPE,
            LogLevel::Error | LogLevel::Fatal => EVENTLOG_ERROR_TYPE,
        }
    }
}
#[derive(Debug, Clone)] // Добавили Clone
pub struct WinEventLogger {
    preferred_source: String,
    fallback_source: &'static str,
}

impl WinEventLogger {
    pub fn new(preferred_source: &str) -> Self {
        Self {
            preferred_source: preferred_source.to_owned(),
            fallback_source: "Application",
        }
    }

    pub fn report(&self, level: LogLevel, message: &str) {
        if self.try_report(&self.preferred_source, level, message) {
            return;
        }

        let prefixed = format!("[{}] {}", self.preferred_source, message);
        let _ = self.try_report(self.fallback_source, level, &prefixed);
    }

    fn try_report(&self, source: &str, level: LogLevel, message: &str) -> bool {
        let c_source = match to_cstring(source) {
            Some(s) => s,
            None => return false,
        };
        let c_message = match to_cstring(message) {
            Some(s) => s,
            None => return false,
        };

        let source_ptr: PCSTR = c_source.as_ptr() as _;
        let msg_ptr: PCSTR = c_message.as_ptr() as _;

        let h_source = unsafe { RegisterEventSourceA(std::ptr::null(), source_ptr) };
        if h_source == 0 {
            return false;
        }

        let success: i32 = unsafe {
            ReportEventA(
                h_source,
                level.to_event_type(),
                0,
                1000,
                0 as PSID,
                1,
                0,
                &msg_ptr,
                std::ptr::null_mut(),
            )
        };

        let _ = unsafe { DeregisterEventSource(h_source) };

        success != 0
    }
}

fn to_cstring(s: &str) -> Option<CString> {
    CString::new(s).ok()
}