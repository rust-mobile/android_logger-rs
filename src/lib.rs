// Copyright 2016 The android_logger Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

//! A logger which writes to android output.
//!
//! ## Example
//!
//! ```ignore
//! #[macro_use] extern crate log;
//! extern crate android_logger;
//!
//! use log::LogLevel;
//!
//! fn native_activity_create() {
//!     android_logger::init_once(LogLevel::Trace);
//!
//!     debug!("this is a debug {}", "message");
//!     error!("this is printed by default");
//!
//!     if log_enabled!(LogLevel::Info) {
//!         let x = 3 * 4; // expensive computation
//!         info!("the answer was: {}", x);
//!     }
//! }
//! ```

extern crate android_log_sys as log_ffi;
#[macro_use] extern crate log;

use log_ffi::LogPriority;
use log::{Log,LogLevel,LogMetadata,LogRecord};
use std::ffi::{ CStr, CString };

/// Output log to android system.
fn android_log(prio: log_ffi::LogPriority, tag: &CStr, msg: &CStr) {
    unsafe { log_ffi::__android_log_write(prio as log_ffi::c_int, tag.as_ptr(), msg.as_ptr()) };
}

struct PlatformLogger;

impl Log for PlatformLogger {
    fn enabled(&self, _: &LogMetadata) -> bool {
        true
    }

    fn log(&self, record: &LogRecord) {
        let tag = CString::new(record.location().module_path()).unwrap();
        let msg = CString::new(format!("{}", record.args())).unwrap();
        match record.level() {
            LogLevel::Warn => android_log(LogPriority::WARN, &tag, &msg),
            LogLevel::Info => android_log(LogPriority::INFO, &tag, &msg),
            LogLevel::Debug => android_log(LogPriority::DEBUG, &tag, &msg),
            LogLevel::Error => android_log(LogPriority::ERROR, &tag, &msg),
            LogLevel::Trace => android_log(LogPriority::VERBOSE, &tag, &msg),
        }
    }
}

/// Initializes the global logger with an android logger.
///
/// This can be called many times, but will only initialize logging once,
/// and will not replace any other previously initialized logger.
///
/// It is ok to call this at the activity creation, and it will be
/// repeatedly called on every lifecycle restart (i.e. screen rotation).
pub fn init_once(log_level: LogLevel) {
    match log::set_logger(|max_log_level| {
        max_log_level.set(log_level.to_log_level_filter());
        return Box::new(PlatformLogger);
    }) {
        Err(e) => debug!("{}", e),
        _ => (),
    }
}
