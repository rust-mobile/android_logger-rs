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
use std::ffi::CStr;
use std::mem;
use std::fmt;

/// Output log to android system.
fn android_log(prio: log_ffi::LogPriority, tag: &CStr, msg: &CStr) {
    unsafe { log_ffi::__android_log_write(prio as log_ffi::c_int, tag.as_ptr(), msg.as_ptr()) };
}

struct PlatformLogger;

const LOGGING_TAG_MAX_LEN: usize = 23;
const LOGGING_MSG_MAX_LEN: usize = 4000;

impl Log for PlatformLogger {
    fn enabled(&self, _: &LogMetadata) -> bool {
        true
    }

    fn log(&self, record: &LogRecord) {
        // tag must not exceed LOGGING_TAG_MAX_LEN
        let mut tag_bytes: [u8; LOGGING_TAG_MAX_LEN + 1] = unsafe { mem::uninitialized() };
        // truncate the tag here to fit into LOGGING_TAG_MAX_LEN
        self.fill_tag_bytes(&mut tag_bytes, record);
        // use stack array as C string
        let tag: &CStr = unsafe { CStr::from_ptr(mem::transmute(tag_bytes.as_ptr())) };

        // message must not exceed LOGGING_MSG_MAX_LEN
        // therefore split log message into multiple log calls
        let mut writer = PlatformLogWriter::new(
            record.level(),
            tag
        );

        // use PlatformLogWriter to output chunks if they exceed max size
        let _ = fmt::write(&mut writer, *record.args());

        // output the remaining message (this would usually be the most common case)
        writer.flush();
    }
}

impl PlatformLogger {
    fn fill_tag_bytes(&self, array: &mut [u8], record: &LogRecord) {
        let tag_bytes_iter = record.location().module_path().bytes();
        if tag_bytes_iter.len() > LOGGING_TAG_MAX_LEN {
            for (input, output) in tag_bytes_iter
                .take(LOGGING_TAG_MAX_LEN - 2)
                .chain(b"..\0".iter().cloned())
                .zip(array.iter_mut())
            {
                *output = input;
            }
        } else {
            for (input, output) in tag_bytes_iter
                .chain(b"\0".iter().cloned())
                .zip(array.iter_mut())
            {
                *output = input;
            }
        }
    }
}

struct PlatformLogWriter<'a> {
    priority: LogPriority,
    len: usize,
    last_newline_index: usize,
    tag: &'a CStr,
    buffer: [u8; LOGGING_MSG_MAX_LEN + 1],
}

impl<'a> PlatformLogWriter<'a> {
    pub fn new<'r>(level: LogLevel, tag: &'r CStr) -> PlatformLogWriter<'r> {
        PlatformLogWriter {
            priority: match level {
                LogLevel::Warn => LogPriority::WARN,
                LogLevel::Info => LogPriority::INFO,
                LogLevel::Debug => LogPriority::DEBUG,
                LogLevel::Error => LogPriority::ERROR,
                LogLevel::Trace => LogPriority::VERBOSE,
            },
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
        mem::swap(&mut last_byte, unsafe { self.buffer.get_unchecked_mut(len) });

        let msg: &CStr = unsafe { CStr::from_ptr(mem::transmute(self.buffer.as_ptr())) };
        android_log(self.priority, self.tag, msg);

        *unsafe { self.buffer.get_unchecked_mut(len) } = last_byte;
    }

    /// Copy `len` bytes from `index` position to starting position.
    fn copy_bytes_to_start(&mut self, index: usize, len: usize) {
        for i in 0..len {
            *unsafe { self.buffer.get_unchecked_mut(i) } =
                *unsafe { self.buffer.get_unchecked_mut(i + index) };
        }
    }
}

impl<'a> fmt::Write for PlatformLogWriter<'a> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let mut incomming_bytes = s.as_bytes();

        while incomming_bytes.len() > 0 {
            let len = self.len;

            // write everything possible to buffer and mark last \n
            let new_len = len + incomming_bytes.len();
            let last_newline = self.buffer[len..LOGGING_MSG_MAX_LEN].iter_mut()
                .zip(incomming_bytes)
                .enumerate()
                .fold(None, |acc, (i, (output, input))| {
                    *output = *input;
                    if *input == b'\n' { Some(i) } else { acc }
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
