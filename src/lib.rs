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
//! ```
//! #[macro_use] extern crate log;
//! extern crate android_logger;
//!
//! use log::Level;
//! use android_logger::Filter;
//!
//! /// Android code may not have obvious "main", this is just an example.
//! fn main() {
//!     android_logger::init_once(
//!         Filter::default().with_min_level(Level::Trace),
//!         None
//!     );
//!
//!     debug!("this is a debug {}", "message");
//!     error!("this is printed by default");
//! }
//! ```
//!
//! ## Example with module path filter
//!
//! It is possible to limit log messages to output from a specific crate,
//! and override the logcat tag name (by default, the crate name is used):
//!
//! ```
//! #[macro_use] extern crate log;
//! extern crate android_logger;
//!
//! use log::Level;
//! use android_logger::Filter;
//!
//! fn main() {
//!     android_logger::init_once(
//!         Filter::default()
//!             .with_min_level(Level::Trace)
//!             .with_allowed_module_path("hello::crate"),
//!         Some("mytag")
//!     );
//!
//!     // ..
//! }
//! ```

#[cfg(target_os = "android")]
extern crate android_log_sys as log_ffi;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;

use std::sync::RwLock;

#[cfg(target_os = "android")]
use log_ffi::LogPriority;
use log::{Level, Log, Metadata, Record};
use std::ffi::{CStr, CString};
use std::mem;
use std::fmt;
use std::ptr;

/// Output log to android system.
#[cfg(target_os = "android")]
fn android_log(prio: log_ffi::LogPriority, tag: &CStr, msg: &CStr) {
    unsafe {
        log_ffi::__android_log_write(
            prio as log_ffi::c_int,
            tag.as_ptr() as *const log_ffi::c_char,
            msg.as_ptr() as *const log_ffi::c_char,
        )
    };
}

/// Dummy output placeholder for tests.
#[cfg(not(target_os = "android"))]
fn android_log(_priority: Level, _tag: &CStr, _msg: &CStr) {}

/// Underlying android logger backend
pub struct AndroidLogger {
    filter: RwLock<Filter>,
    tag: RwLock<Option<CString>>,
}

lazy_static! {
   static ref ANDROID_LOGGER: AndroidLogger = AndroidLogger::default();
}

const LOGGING_TAG_MAX_LEN: usize = 23;
const LOGGING_MSG_MAX_LEN: usize = 4000;

impl Default for AndroidLogger {
    /// Create a new logger with default filter
    fn default() -> AndroidLogger {
        AndroidLogger {
            tag: RwLock::new(None),
            filter: RwLock::new(Filter::default()),
        }
    }
}

impl Log for AndroidLogger {
    fn enabled(&self, _: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        if let Some(module_path) = record.module_path() {
            let filter = self.filter
                .read()
                .expect("failed to acquire android_log filter lock for read");

            if !filter.is_module_path_allowed(module_path) {
                return;
            }
        }

        // tag must not exceed LOGGING_TAG_MAX_LEN
        let mut tag_bytes: [u8; LOGGING_TAG_MAX_LEN + 1] = unsafe { mem::uninitialized() };

        let module_path = record.module_path().unwrap_or_default().to_owned();

        // If no tag was specified, use module name
        let custom_tag = self.tag.read().expect("failed to acquire android_log tag lock for read");
        let tag = custom_tag.as_ref().map(|s| s.as_bytes()).unwrap_or(module_path.as_bytes());

        // truncate the tag here to fit into LOGGING_TAG_MAX_LEN
        self.fill_tag_bytes(&mut tag_bytes, tag);
        // use stack array as C string
        let tag: &CStr = unsafe { CStr::from_ptr(mem::transmute(tag_bytes.as_ptr())) };

        // message must not exceed LOGGING_MSG_MAX_LEN
        // therefore split log message into multiple log calls
        let mut writer = PlatformLogWriter::new(record.level(), tag);

        // If a custom tag is used, add the module path to the message.
        // Use PlatformLogWriter to output chunks if they exceed max size.
        let _ = if custom_tag.is_some() {
            fmt::write(&mut writer, format_args!("{}: {}", module_path, *record.args()))
        } else {
            fmt::write(&mut writer, *record.args())
        };

        // output the remaining message (this would usually be the most common case)
        writer.flush();
    }

    fn flush(&self) {}
}

impl AndroidLogger {
    fn fill_tag_bytes(&self, array: &mut [u8], tag: &[u8]) {
        if tag.len() > LOGGING_TAG_MAX_LEN {
            for (input, output) in tag.iter()
                .take(LOGGING_TAG_MAX_LEN - 2)
                .chain(b"..\0".iter())
                .zip(array.iter_mut())
            {
                *output = *input;
            }
        } else {
            for (input, output) in tag.iter()
                .chain(b"\0".iter())
                .zip(array.iter_mut())
            {
                *output = *input;
            }
        }
    }
}

/// Filter for android logger.
pub struct Filter {
    log_level: Option<Level>,
    allow_module_paths: Vec<String>,
}

impl Default for Filter {
    fn default() -> Self {
        Filter {
            log_level: None,
            allow_module_paths: Vec::new(),
        }
    }
}

impl Filter {
    /// Change the minimum log level.
    ///
    /// All values above the set level are logged. For example, if
    /// `Warn` is set, the `Error` is logged too, but `Info` isn't.
    pub fn with_min_level(mut self, level: Level) -> Self {
        self.log_level = Some(level);
        self
    }

    /// Set allowed module path.
    ///
    /// Allow log entry only if module path matches specified path exactly.
    ///
    /// ## Example:
    ///
    /// ```
    /// use android_logger::Filter;
    ///
    /// let filter = Filter::default().with_allowed_module_path("crate");
    ///
    /// assert!(filter.is_module_path_allowed("crate"));
    /// assert!(!filter.is_module_path_allowed("other_crate"));
    /// assert!(!filter.is_module_path_allowed("crate::subcrate"));
    /// ```
    ///
    /// ## Multiple rules example:
    ///
    /// ```
    /// use android_logger::Filter;
    ///
    /// let filter = Filter::default()
    ///     .with_allowed_module_path("A")
    ///     .with_allowed_module_path("B");
    ///
    /// assert!(filter.is_module_path_allowed("A"));
    /// assert!(filter.is_module_path_allowed("B"));
    /// assert!(!filter.is_module_path_allowed("C"));
    /// assert!(!filter.is_module_path_allowed("A::B"));
    /// ```
    pub fn with_allowed_module_path<S: Into<String>>(mut self, path: S) -> Self {
        self.allow_module_paths.push(path.into());
        self
    }

    /// Set multiple allowed module paths.
    ///
    /// Same as `with_allowed_module_path`, but accepts list of paths.
    ///
    /// ## Example:
    ///
    /// ```
    /// use android_logger::Filter;
    ///
    /// let filter = Filter::default()
    ///     .with_allowed_module_paths(["A", "B"].iter().map(|i| i.to_string()));
    ///
    /// assert!(filter.is_module_path_allowed("A"));
    /// assert!(filter.is_module_path_allowed("B"));
    /// assert!(!filter.is_module_path_allowed("C"));
    /// assert!(!filter.is_module_path_allowed("A::B"));
    /// ```
    pub fn with_allowed_module_paths<I: IntoIterator<Item = String>>(mut self, paths: I) -> Self {
        self.allow_module_paths.extend(paths.into_iter());
        self
    }

    /// Check if module path is allowed by filter rules.
    pub fn is_module_path_allowed(&self, path: &str) -> bool {
        if self.allow_module_paths.is_empty() {
            return true;
        }

        self.allow_module_paths
            .iter()
            .any(|allowed_path| path.contains(allowed_path))
    }
}

#[cfg(test)]
mod tests {
    use super::Filter;

    #[test]
    fn with_allowed_module_path() {
        assert!(Filter::default().is_module_path_allowed("random"));
    }
}

struct PlatformLogWriter<'a> {
    #[cfg(target_os = "android")] priority: LogPriority,
    #[cfg(not(target_os = "android"))] priority: Level,
    len: usize,
    last_newline_index: usize,
    tag: &'a CStr,
    buffer: [u8; LOGGING_MSG_MAX_LEN + 1],
}

impl<'a> PlatformLogWriter<'a> {
    #[cfg(target_os = "android")]
    pub fn new(level: Level, tag: &CStr) -> PlatformLogWriter {
        PlatformLogWriter {
            priority: match level {
                Level::Warn => LogPriority::WARN,
                Level::Info => LogPriority::INFO,
                Level::Debug => LogPriority::DEBUG,
                Level::Error => LogPriority::ERROR,
                Level::Trace => LogPriority::VERBOSE,
            },
            len: 0,
            last_newline_index: 0,
            tag: tag,
            buffer: unsafe { mem::uninitialized() },
        }
    }

    #[cfg(not(target_os = "android"))]
    pub fn new(level: Level, tag: &CStr) -> PlatformLogWriter {
        PlatformLogWriter {
            priority: level,
            len: 0,
            last_newline_index: 0,
            tag: tag,
            buffer: unsafe { mem::uninitialized() },
        }
    }

    /// Flush some bytes to android logger.
    ///
    /// If there is a newline, flush up to it.
    /// If ther was no newline, flush all.
    ///
    /// Not guaranteed to flush everything.
    fn temporal_flush(&mut self) {
        let total_len = self.len;

        if total_len == 0 {
            return;
        }

        if self.last_newline_index > 0 {
            let copy_from_index = self.last_newline_index;
            let remaining_chunk_len = total_len - copy_from_index;

            self.output_specified_len(copy_from_index);
            self.copy_bytes_to_start(copy_from_index, remaining_chunk_len);
            self.len = remaining_chunk_len;
        } else {
            self.output_specified_len(total_len);
            self.len = 0;
        }
        self.last_newline_index = 0;
    }

    /// Flush everything remaining to android logger.
    fn flush(&mut self) {
        let total_len = self.len;

        if total_len == 0 {
            return;
        }

        self.output_specified_len(total_len);
        self.len = 0;
        self.last_newline_index = 0;
    }

    /// Output buffer up until the \0 which will be placed at `len` position.
    fn output_specified_len(&mut self, len: usize) {
        let mut last_byte: u8 = b'\0';
        mem::swap(&mut last_byte, unsafe {
            self.buffer.get_unchecked_mut(len)
        });

        let msg: &CStr = unsafe { CStr::from_ptr(mem::transmute(self.buffer.as_ptr())) };
        android_log(self.priority, self.tag, msg);

        *unsafe { self.buffer.get_unchecked_mut(len) } = last_byte;
    }

    /// Copy `len` bytes from `index` position to starting position.
    fn copy_bytes_to_start(&mut self, index: usize, len: usize) {
        let src = unsafe { self.buffer.as_ptr().offset(index as isize) };
        let dst = self.buffer.as_mut_ptr();
        unsafe { ptr::copy(src, dst, len) };
    }
}

impl<'a> fmt::Write for PlatformLogWriter<'a> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let mut incomming_bytes = s.as_bytes();

        while !incomming_bytes.is_empty() {
            let len = self.len;

            // write everything possible to buffer and mark last \n
            let new_len = len + incomming_bytes.len();
            let last_newline = self.buffer[len..LOGGING_MSG_MAX_LEN]
                .iter_mut()
                .zip(incomming_bytes)
                .enumerate()
                .fold(None, |acc, (i, (output, input))| {
                    *output = *input;
                    if *input == b'\n' {
                        Some(i)
                    } else {
                        acc
                    }
                });

            // update last \n index
            if let Some(newline) = last_newline {
                self.last_newline_index = len + newline;
            }

            // calculate how many bytes were written
            let written_len = if new_len <= LOGGING_MSG_MAX_LEN {
                // if the len was not exceeded
                self.len = new_len;
                new_len - len // written len
            } else {
                // if new length was exceeded
                self.len = LOGGING_MSG_MAX_LEN;
                self.temporal_flush();

                LOGGING_MSG_MAX_LEN - len // written len
            };

            incomming_bytes = &incomming_bytes[written_len..];
        }

        Ok(())
    }
}

/// Send a log record to Android logging backend.
///
/// This action does not require initialization. However, without initialization it
/// will use the default filter, which allows all logs.
pub fn log(record: &Record) {
    ANDROID_LOGGER.log(record)
}

/// Initializes the global logger with an android logger.
///
/// This can be called many times, but will only initialize logging once,
/// and will not replace any other previously initialized logger.
///
/// It is ok to call this at the activity creation, and it will be
/// repeatedly called on every lifecycle restart (i.e. screen rotation).
pub fn init_once(filter: Filter, tag: Option<&str>) {
    if let Err(err) = log::set_logger(&*ANDROID_LOGGER) {
        debug!("android_logger: log::set_logger failed: {}", err);
    } else {
        if let Some(level) = filter.log_level {
            log::set_max_level(level.to_level_filter());
        }
        *ANDROID_LOGGER
            .filter
            .write()
            .expect("failed to acquire android_log filter lock for write") = filter;
        *ANDROID_LOGGER
            .tag
            .write()
            .expect("failed to acquire android_log filter lock for write") = tag.map(|s| CString::new(s).expect("Can't convert tag to CString"))
    }
}
