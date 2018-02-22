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
//! use log::Level;
//!
//! fn native_activity_create() {
//!     android_logger::init_once(Level::Trace);
//!
//!     debug!("this is a debug {}", "message");
//!     error!("this is printed by default");
//!
//!     if log_enabled!(Level::Info) {
//!         let x = 3 * 4; // expensive computation
//!         info!("the answer was: {}", x);
//!     }
//! }
//! ```

extern crate android_log_sys as log_ffi;
#[macro_use]
extern crate log;

use log_ffi::LogPriority;
use log::{Log,Level,Metadata,Record};
use std::ffi::CStr;
use std::mem;
use std::fmt;
use std::ptr;

/// Output log to android system.
fn android_log(prio: log_ffi::LogPriority, tag: &CStr, msg: &CStr) {
    unsafe { log_ffi::__android_log_write(prio as log_ffi::c_int, tag.as_ptr() as *const log_ffi::c_char, msg.as_ptr() as *const log_ffi::c_char) };
}

/// Underlying android logger, for cases where `init_once` abstraction is not enough.
pub struct AndroidLogger;

const LOGGING_TAG_MAX_LEN: usize = 23;
const LOGGING_MSG_MAX_LEN: usize = 4000;

impl Default for AndroidLogger {
    fn default() -> AndroidLogger {
        AndroidLogger
    }
}

impl Log for AndroidLogger {
    fn enabled(&self, _: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
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

    fn flush(&self) {
    }
}

impl AndroidLogger {
    fn fill_tag_bytes(&self, array: &mut [u8], record: &Record) {
        let tag_bytes_iter = record.module_path().unwrap_or_default().bytes();
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
    pub fn new<'r>(level: Level, tag: &'r CStr) -> PlatformLogWriter<'r> {
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
        let src = unsafe { self.buffer.as_ptr().offset(index as isize) };
        let dst = self.buffer.as_mut_ptr();
        unsafe { ptr::copy(src, dst, len) };
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
pub fn init_once(log_level: Level) {
    log::set_max_level(log_level.to_level_filter());
    if let Err(err) = log::set_logger(&AndroidLogger) {
        debug!("android_logger: log::set_logger failed: {}", err);
    }
}
