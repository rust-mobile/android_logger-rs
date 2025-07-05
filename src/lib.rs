// Copyright 2016 The android_logger Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

#![doc = include_str!("../README.md")]

#[cfg(target_os = "android")]
extern crate android_log_sys as log_ffi;

use config::is_loggable;
use log::{Log, Metadata, Record};
use std::ffi::{CStr, CString};
use std::fmt;
use std::mem::MaybeUninit;
use std::sync::OnceLock;

use crate::arrays::{fill_tag_bytes, uninit_array};
use crate::platform_log_writer::PlatformLogWriter;
pub use config::Config;
pub use env_filter::{Builder as FilterBuilder, Filter};
pub use id::LogId;

pub(crate) type FormatFn = Box<dyn Fn(&mut dyn fmt::Write, &Record) -> fmt::Result + Sync + Send>;

mod arrays;
mod config;
mod id;
mod platform_log_writer;
#[cfg(test)]
mod tests;

/// Outputs log to Android system.
#[cfg(target_os = "android")]
fn android_log(
    buf_id: Option<log_ffi::log_id_t>,
    prio: log_ffi::LogPriority,
    tag: &CStr,
    msg: &CStr,
) {
    if let Some(buf_id) = buf_id {
        unsafe {
            log_ffi::__android_log_buf_write(
                buf_id as log_ffi::c_int,
                prio as log_ffi::c_int,
                tag.as_ptr() as *const log_ffi::c_char,
                msg.as_ptr() as *const log_ffi::c_char,
            );
        };
    } else {
        unsafe {
            log_ffi::__android_log_write(
                prio as log_ffi::c_int,
                tag.as_ptr() as *const log_ffi::c_char,
                msg.as_ptr() as *const log_ffi::c_char,
            );
        };
    }
}

/// Dummy output placeholder for tests.
#[cfg(not(target_os = "android"))]
fn android_log(_buf_id: Option<LogId>, _priority: log::Level, _tag: &CStr, _msg: &CStr) {}

/// Underlying Android logger backend
#[derive(Debug)]
pub struct AndroidLogger {
    filter: env_filter::Filter,
    config: Config,
}

impl Default for AndroidLogger {
    /// Create new logger instance using the [default `Cofig`][Config::default()].
    fn default() -> Self {
        Self::new(Config::default())
    }
}

impl AndroidLogger {
    /// Create new logger instance from config
    pub fn new(mut config: Config) -> AndroidLogger {
        AndroidLogger {
            // TODO: This consumes the filter from the config, disallowing it to be reused...
            filter: config.filter.build(),
            config,
        }
    }
}

static ANDROID_LOGGER: OnceLock<AndroidLogger> = OnceLock::new();

/// Maximum length of a tag that does not require allocation.
///
/// Tags configured explicitly in [`Config`] will not cause an extra allocation. When the tag is
/// derived from the module path, paths longer than this limit will trigger an allocation for each
/// log statement.
///
/// The terminating nullbyte does not count towards this limit.
const LOGGING_TAG_MAX_LEN: usize = 127;
const LOGGING_MSG_MAX_LEN: usize = 4000;

impl Log for AndroidLogger {
    /// # Warning
    /// This method relies on stateful data when `android-api-30` is enabled, including
    /// [`log::max_level()`] which we only initialize when [`init_once()`] is called and which can
    /// be changed by the user at any point via [`log::set_max_level()`].
    fn enabled(&self, metadata: &Metadata) -> bool {
        is_loggable(metadata.target(), metadata.level())
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        if !self.filter.matches(record) {
            return;
        }

        // Temporary storage for null-terminating record.module_path() if it's needed.
        // Tags too long to fit here cause allocation.
        let mut tag_bytes: [MaybeUninit<u8>; LOGGING_TAG_MAX_LEN + 1] = uninit_array();
        // In case we end up allocating, keep the CString alive.
        let _owned_tag;

        let module_path = record.module_path().unwrap_or_default();

        let tag = if let Some(tag) = &self.config.tag {
            tag
        } else if module_path.len() < tag_bytes.len() {
            fill_tag_bytes(&mut tag_bytes, module_path.as_bytes())
        } else {
            // Tag longer than available stack buffer; allocate.
            _owned_tag = CString::new(module_path.as_bytes())
                .expect("record.module_path() shouldn't contain nullbytes");
            _owned_tag.as_ref()
        };

        // message must not exceed LOGGING_MSG_MAX_LEN
        // therefore split log message into multiple log calls
        let mut writer = PlatformLogWriter::new(self.config.buf_id, record.level(), tag);

        // If a custom tag is used, add the module path to the message.
        // Use PlatformLogWriter to output chunks if they exceed max size.
        use std::fmt::Write;
        let _ = match (&self.config.tag, &self.config.custom_format) {
            (_, Some(format)) => format(&mut writer, record),
            (Some(_), _) => write!(&mut writer, "{}: {}", module_path, *record.args()),
            _ => fmt::write(&mut writer, *record.args()),
        };

        // output the remaining message (this would usually be the most common case)
        writer.flush();
    }

    fn flush(&self) {}
}

/// Send a log record to the Android logging backend.
///
/// This action does not require initialization, and does not initialize the [`mod@log`] framework
/// to redirect all logs to [`AndroidLogger`].  If not otherwise configured earlier using
/// [`init_once()`] this uses the default [`Config`] with [`log::LevelFilter::Error`].
pub fn log(record: &Record) {
    let logger = ANDROID_LOGGER.get_or_init(|| AndroidLogger::new(Default::default()));
    logger.log(record);
}

/// Initializes the global logger with an Android logger.
///
/// This can be called many times, but will only initialize logging once,
/// and will not replace any other previously initialized logger.
///
/// It is ok to call this at the activity creation, and it will be
/// repeatedly called on every lifecycle restart (i.e. screen rotation).
///
/// # Warning
/// `config` is ignored on subsequent calls to either [`init_once()`] or [`log()`].
pub fn init_once(config: Config) {
    let logger = ANDROID_LOGGER.get_or_init(|| AndroidLogger::new(config));

    // TODO: Only continue if ANDROID_LOGGER was None?

    let log_level = logger.filter.filter();

    if let Err(err) = log::set_logger(logger) {
        // TODO: Bubble up the error (try_init()) or panic (init()), as suggested
        // by the `log` crate and as implemented by `env_logger`.
        log::debug!("android_logger: log::set_logger failed: {err}");
    } else {
        log::set_max_level(log_level);
    }
}
