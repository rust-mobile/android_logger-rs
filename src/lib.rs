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
//! use log::LevelFilter;
//! use android_logger::Config;
//!
//! /// Android code may not have obvious "main", this is just an example.
//! fn main() {
//!     android_logger::init_once(
//!         Config::default().with_max_level(LevelFilter::Trace),
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
//! use log::LevelFilter;
//! use android_logger::{Config,FilterBuilder};
//!
//! fn main() {
//!     android_logger::init_once(
//!         Config::default()
//!             .with_max_level(LevelFilter::Trace)
//!             .with_tag("mytag")
//!             .with_filter(FilterBuilder::new().parse("debug,hello::crate=trace").build()),
//!     );
//!
//!     // ..
//! }
//! ```
//!
//! ## Example with a custom log formatter
//!
//! ```
//! use android_logger::Config;
//!
//! android_logger::init_once(
//!     Config::default()
//!         .with_max_level(log::LevelFilter::Trace)
//!         .format(|f, record| write!(f, "my_app: {}", record.args()))
//! )
//! ```

#[cfg(target_os = "android")]
extern crate android_log_sys as log_ffi;
extern crate once_cell;
use once_cell::sync::OnceCell;
#[macro_use]
extern crate log;

extern crate env_logger;

use log::{Level, LevelFilter, Log, Metadata, Record};
#[cfg(target_os = "android")]
use log_ffi::LogPriority;
use std::ffi::{CStr, CString};
use std::fmt;
use std::mem::{self, MaybeUninit};
use std::ptr;

pub use env_logger::filter::{Builder as FilterBuilder, Filter};
pub use env_logger::fmt::Formatter;

pub(crate) type FormatFn = Box<dyn Fn(&mut dyn fmt::Write, &Record) -> fmt::Result + Sync + Send>;

/// Possible identifiers of a specific buffer of Android logging system for
/// logging a message.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LogId {
    /// Main log buffer.
    ///
    /// This is the only log buffer available to apps.
    Main,

    /// Radio log buffer.
    Radio,

    /// Event log buffer.
    Events,

    /// System log buffer.
    System,

    /// Crash log buffer.
    Crash,

    /// Kernel log buffer.
    Kernel,

    /// Security log buffer.
    Security,

    /// Statistics log buffer.
    Stats,
}

#[cfg(target_os = "android")]
impl LogId {
    fn to_native(log_id: Option<Self>) -> log_ffi::log_id_t {
        match log_id {
            Some(Self::Main) => log_ffi::log_id_t::MAIN,
            Some(Self::Radio) => log_ffi::log_id_t::RADIO,
            Some(Self::Events) => log_ffi::log_id_t::EVENTS,
            Some(Self::System) => log_ffi::log_id_t::SYSTEM,
            Some(Self::Crash) => log_ffi::log_id_t::CRASH,
            Some(Self::Kernel) => log_ffi::log_id_t::KERNEL,
            Some(Self::Security) => log_ffi::log_id_t::SECURITY,
            Some(Self::Stats) => log_ffi::log_id_t::STATS,
            None => log_ffi::log_id_t::DEFAULT,
        }
    }
}

/// Outputs log to Android system.
#[cfg(target_os = "android")]
fn android_log(buf_id: log_ffi::log_id_t, prio: log_ffi::LogPriority, tag: &CStr, msg: &CStr) {
    unsafe {
        log_ffi::__android_log_buf_write(
            buf_id as log_ffi::c_int,
            prio as log_ffi::c_int,
            tag.as_ptr() as *const log_ffi::c_char,
            msg.as_ptr() as *const log_ffi::c_char,
        );
    };
}

/// Dummy output placeholder for tests.
#[cfg(not(target_os = "android"))]
fn android_log(_buf_id: Option<LogId>, _priority: Level, _tag: &CStr, _msg: &CStr) {}

/// Underlying android logger backend
pub struct AndroidLogger {
    config: OnceCell<Config>,
}

impl AndroidLogger {
    /// Create new logger instance from config
    pub fn new(config: Config) -> AndroidLogger {
        AndroidLogger {
            config: OnceCell::from(config),
        }
    }

    fn config(&self) -> &Config {
        self.config.get_or_init(Config::default)
    }
}

static ANDROID_LOGGER: OnceCell<AndroidLogger> = OnceCell::new();

const LOGGING_TAG_MAX_LEN: usize = 23;
const LOGGING_MSG_MAX_LEN: usize = 4000;

impl Default for AndroidLogger {
    /// Create a new logger with default config
    fn default() -> AndroidLogger {
        AndroidLogger {
            config: OnceCell::from(Config::default()),
        }
    }
}

impl Log for AndroidLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        let config = self.config();
        // todo: consider __android_log_is_loggable.
        metadata.level() <= config.log_level.unwrap_or_else(log::max_level)
    }

    fn log(&self, record: &Record) {
        let config = self.config();

        if !self.enabled(record.metadata()) {
            return;
        }

        // this also checks the level, but only if a filter was
        // installed.
        if !config.filter_matches(record) {
            return;
        }

        // tag must not exceed LOGGING_TAG_MAX_LEN
        let mut tag_bytes: [MaybeUninit<u8>; LOGGING_TAG_MAX_LEN + 1] = uninit_array();

        let module_path = record.module_path().unwrap_or_default().to_owned();

        // If no tag was specified, use module name
        let custom_tag = &config.tag;
        let tag = custom_tag
            .as_ref()
            .map(|s| s.as_bytes())
            .unwrap_or_else(|| module_path.as_bytes());

        // truncate the tag here to fit into LOGGING_TAG_MAX_LEN
        self.fill_tag_bytes(&mut tag_bytes, tag);
        // use stack array as C string
        let tag: &CStr = unsafe { CStr::from_ptr(mem::transmute(tag_bytes.as_ptr())) };

        // message must not exceed LOGGING_MSG_MAX_LEN
        // therefore split log message into multiple log calls
        let mut writer = PlatformLogWriter::new(config.buf_id, record.level(), tag);

        // If a custom tag is used, add the module path to the message.
        // Use PlatformLogWriter to output chunks if they exceed max size.
        let _ = match (custom_tag, &config.custom_format) {
            (_, Some(format)) => format(&mut writer, record),
            (Some(_), _) => fmt::write(
                &mut writer,
                format_args!("{}: {}", module_path, *record.args()),
            ),
            _ => fmt::write(&mut writer, *record.args()),
        };

        // output the remaining message (this would usually be the most common case)
        writer.flush();
    }

    fn flush(&self) {}
}

impl AndroidLogger {
    fn fill_tag_bytes(&self, array: &mut [MaybeUninit<u8>], tag: &[u8]) {
        if tag.len() > LOGGING_TAG_MAX_LEN {
            for (input, output) in tag
                .iter()
                .take(LOGGING_TAG_MAX_LEN - 2)
                .chain(b"..\0".iter())
                .zip(array.iter_mut())
            {
                output.write(*input);
            }
        } else {
            for (input, output) in tag.iter().chain(b"\0".iter()).zip(array.iter_mut()) {
                output.write(*input);
            }
        }
    }
}

/// Filter for android logger.
#[derive(Default)]
pub struct Config {
    log_level: Option<LevelFilter>,
    buf_id: Option<LogId>,
    filter: Option<env_logger::filter::Filter>,
    tag: Option<CString>,
    custom_format: Option<FormatFn>,
}

impl Config {
    /// Changes the maximum log level.
    ///
    /// Note, that `Trace` is the maximum level, because it provides the
    /// maximum amount of detail in the emitted logs.
    ///
    /// If `Off` level is provided, then nothing is logged at all.
    ///
    /// [`log::max_level()`] is considered as the default level.
    pub fn with_max_level(mut self, level: LevelFilter) -> Self {
        self.log_level = Some(level);
        self
    }

    /// Changes the Android logging system buffer to be used.
    ///
    /// By default, logs are sent to the [`Main`] log. Other logging buffers may
    /// only be accessible to certain processes.
    ///
    /// [`Main`]: LogId::Main
    pub fn with_log_buffer(mut self, buf_id: LogId) -> Self {
        self.buf_id = Some(buf_id);
        self
    }

    fn filter_matches(&self, record: &Record) -> bool {
        if let Some(ref filter) = self.filter {
            filter.matches(record)
        } else {
            true
        }
    }

    pub fn with_filter(mut self, filter: env_logger::filter::Filter) -> Self {
        self.filter = Some(filter);
        self
    }

    pub fn with_tag<S: Into<Vec<u8>>>(mut self, tag: S) -> Self {
        self.tag = Some(CString::new(tag).expect("Can't convert tag to CString"));
        self
    }

    /// Sets the format function for formatting the log output.
    /// ```
    /// # use android_logger::Config;
    /// android_logger::init_once(
    ///     Config::default()
    ///         .with_max_level(log::LevelFilter::Trace)
    ///         .format(|f, record| write!(f, "my_app: {}", record.args()))
    /// )
    /// ```
    pub fn format<F>(mut self, format: F) -> Self
    where
        F: Fn(&mut dyn fmt::Write, &Record) -> fmt::Result + Sync + Send + 'static,
    {
        self.custom_format = Some(Box::new(format));
        self
    }
}

pub struct PlatformLogWriter<'a> {
    #[cfg(target_os = "android")]
    priority: LogPriority,
    #[cfg(not(target_os = "android"))]
    priority: Level,
    #[cfg(target_os = "android")]
    buf_id: log_ffi::log_id_t,
    #[cfg(not(target_os = "android"))]
    buf_id: Option<LogId>,
    len: usize,
    last_newline_index: usize,
    tag: &'a CStr,
    buffer: [MaybeUninit<u8>; LOGGING_MSG_MAX_LEN + 1],
}

impl<'a> PlatformLogWriter<'a> {
    #[cfg(target_os = "android")]
    pub fn new_with_priority(
        buf_id: Option<LogId>,
        priority: log_ffi::LogPriority,
        tag: &CStr,
    ) -> PlatformLogWriter<'_> {
        #[allow(deprecated)] // created an issue #35 for this
        PlatformLogWriter {
            priority,
            buf_id: LogId::to_native(buf_id),
            len: 0,
            last_newline_index: 0,
            tag,
            buffer: uninit_array(),
        }
    }

    #[cfg(target_os = "android")]
    pub fn new(buf_id: Option<LogId>, level: Level, tag: &CStr) -> PlatformLogWriter<'_> {
        PlatformLogWriter::new_with_priority(
            buf_id,
            match level {
                Level::Warn => LogPriority::WARN,
                Level::Info => LogPriority::INFO,
                Level::Debug => LogPriority::DEBUG,
                Level::Error => LogPriority::ERROR,
                Level::Trace => LogPriority::VERBOSE,
            },
            tag,
        )
    }

    #[cfg(not(target_os = "android"))]
    pub fn new(buf_id: Option<LogId>, level: Level, tag: &CStr) -> PlatformLogWriter<'_> {
        #[allow(deprecated)] // created an issue #35 for this
        PlatformLogWriter {
            priority: level,
            buf_id,
            len: 0,
            last_newline_index: 0,
            tag,
            buffer: uninit_array(),
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
    pub fn flush(&mut self) {
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
        let mut last_byte = MaybeUninit::new(b'\0');

        mem::swap(&mut last_byte, unsafe {
            self.buffer.get_unchecked_mut(len)
        });

        let msg: &CStr = unsafe { CStr::from_ptr(self.buffer.as_ptr().cast()) };
        android_log(self.buf_id, self.priority, self.tag, msg);

        unsafe { *self.buffer.get_unchecked_mut(len) = last_byte };
    }

    /// Copy `len` bytes from `index` position to starting position.
    fn copy_bytes_to_start(&mut self, index: usize, len: usize) {
        let dst = self.buffer.as_mut_ptr();
        let src = unsafe { self.buffer.as_ptr().add(index) };
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
                    output.write(*input);
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
    ANDROID_LOGGER
        .get_or_init(AndroidLogger::default)
        .log(record)
}

/// Initializes the global logger with an android logger.
///
/// This can be called many times, but will only initialize logging once,
/// and will not replace any other previously initialized logger.
///
/// It is ok to call this at the activity creation, and it will be
/// repeatedly called on every lifecycle restart (i.e. screen rotation).
pub fn init_once(config: Config) {
    let log_level = config.log_level;
    let logger = ANDROID_LOGGER.get_or_init(|| AndroidLogger::new(config));

    if let Err(err) = log::set_logger(logger) {
        debug!("android_logger: log::set_logger failed: {}", err);
    } else if let Some(level) = log_level {
        log::set_max_level(level);
    }
}

// FIXME: When `maybe_uninit_uninit_array ` is stabilized, use it instead of this helper
fn uninit_array<const N: usize, T>() -> [MaybeUninit<T>; N] {
    // SAFETY: Array contains MaybeUninit, which is fine to be uninit
    unsafe { MaybeUninit::uninit().assume_init() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn check_config_values() {
        // Filter is checked in config_filter_match below.
        let config = Config::default()
            .with_max_level(LevelFilter::Trace)
            .with_log_buffer(LogId::System)
            .with_tag("my_app");

        assert_eq!(config.log_level, Some(LevelFilter::Trace));
        assert_eq!(config.buf_id, Some(LogId::System));
        assert_eq!(config.tag, Some(CString::new("my_app").unwrap()));
    }

    #[test]
    fn log_calls_formatter() {
        static FORMAT_FN_WAS_CALLED: AtomicBool = AtomicBool::new(false);
        let config = Config::default()
            .with_max_level(LevelFilter::Info)
            .format(|_, _| {
                FORMAT_FN_WAS_CALLED.store(true, Ordering::SeqCst);
                Ok(())
            });
        let logger = AndroidLogger::new(config);

        logger.log(&Record::builder().level(Level::Info).build());

        assert!(FORMAT_FN_WAS_CALLED.load(Ordering::SeqCst));
    }

    #[test]
    fn logger_enabled_threshold() {
        let logger = AndroidLogger::new(Config::default().with_max_level(LevelFilter::Info));

        assert!(logger.enabled(&log::MetadataBuilder::new().level(Level::Warn).build()));
        assert!(logger.enabled(&log::MetadataBuilder::new().level(Level::Info).build()));
        assert!(!logger.enabled(&log::MetadataBuilder::new().level(Level::Debug).build()));
    }

    // Test whether the filter gets called correctly. Not meant to be exhaustive for all filter
    // options, as these are handled directly by the filter itself.
    #[test]
    fn config_filter_match() {
        let info_record = Record::builder().level(Level::Info).build();
        let debug_record = Record::builder().level(Level::Debug).build();

        let info_all_filter = env_logger::filter::Builder::new().parse("info").build();
        let info_all_config = Config::default().with_filter(info_all_filter);

        assert!(info_all_config.filter_matches(&info_record));
        assert!(!info_all_config.filter_matches(&debug_record));
    }

    #[test]
    fn fill_tag_bytes_truncates_long_tag() {
        let logger = AndroidLogger::new(Config::default());
        let too_long_tag: [u8; LOGGING_TAG_MAX_LEN + 20] = [b'a'; LOGGING_TAG_MAX_LEN + 20];

        let mut result: [MaybeUninit<u8>; LOGGING_TAG_MAX_LEN + 1] = uninit_array();
        logger.fill_tag_bytes(&mut result, &too_long_tag);

        let mut expected_result = [b'a'; LOGGING_TAG_MAX_LEN - 2].to_vec();
        expected_result.extend("..\0".as_bytes());
        assert_eq!(unsafe { assume_init_slice(&result) }, expected_result);
    }

    #[test]
    fn fill_tag_bytes_keeps_short_tag() {
        let logger = AndroidLogger::new(Config::default());
        let short_tag: [u8; 3] = [b'a'; 3];

        let mut result: [MaybeUninit<u8>; LOGGING_TAG_MAX_LEN + 1] = uninit_array();
        logger.fill_tag_bytes(&mut result, &short_tag);

        let mut expected_result = short_tag.to_vec();
        expected_result.push(0);
        assert_eq!(unsafe { assume_init_slice(&result[..4]) }, expected_result);
    }

    #[test]
    fn platform_log_writer_init_values() {
        let tag = CStr::from_bytes_with_nul(b"tag\0").unwrap();

        let writer = PlatformLogWriter::new(None, Level::Warn, tag);

        assert_eq!(writer.tag, tag);
        // Android uses LogPriority instead, which doesn't implement equality checks
        #[cfg(not(target_os = "android"))]
        assert_eq!(writer.priority, Level::Warn);
    }

    #[test]
    fn temporal_flush() {
        let mut writer = get_tag_writer();

        writer
            .write_str("12\n\n567\n90")
            .expect("Unable to write to PlatformLogWriter");

        assert_eq!(writer.len, 10);
        writer.temporal_flush();
        // Should have flushed up until the last newline.
        assert_eq!(writer.len, 3);
        assert_eq!(writer.last_newline_index, 0);
        assert_eq!(
            unsafe { assume_init_slice(&writer.buffer[..writer.len]) },
            "\n90".as_bytes()
        );

        writer.temporal_flush();
        // Should have flushed all remaining bytes.
        assert_eq!(writer.len, 0);
        assert_eq!(writer.last_newline_index, 0);
    }

    #[test]
    fn flush() {
        let mut writer = get_tag_writer();
        writer
            .write_str("abcdefghij\n\nklm\nnopqr\nstuvwxyz")
            .expect("Unable to write to PlatformLogWriter");

        writer.flush();

        assert_eq!(writer.last_newline_index, 0);
        assert_eq!(writer.len, 0);
    }

    #[test]
    fn last_newline_index() {
        let mut writer = get_tag_writer();

        writer
            .write_str("12\n\n567\n90")
            .expect("Unable to write to PlatformLogWriter");

        assert_eq!(writer.last_newline_index, 7);
    }

    #[test]
    fn output_specified_len_leaves_buffer_unchanged() {
        let mut writer = get_tag_writer();
        let log_string = "abcdefghij\n\nklm\nnopqr\nstuvwxyz";
        writer
            .write_str(log_string)
            .expect("Unable to write to PlatformLogWriter");

        writer.output_specified_len(5);

        assert_eq!(
            unsafe { assume_init_slice(&writer.buffer[..log_string.len()]) },
            log_string.as_bytes()
        );
    }

    #[test]
    fn copy_bytes_to_start() {
        let mut writer = get_tag_writer();
        writer
            .write_str("0123456789")
            .expect("Unable to write to PlatformLogWriter");

        writer.copy_bytes_to_start(3, 2);

        assert_eq!(
            unsafe { assume_init_slice(&writer.buffer[..10]) },
            "3423456789".as_bytes()
        );
    }

    #[test]
    fn copy_bytes_to_start_nop() {
        let test_string = "Test_string_with\n\n\n\nnewlines\n";
        let mut writer = get_tag_writer();
        writer
            .write_str(test_string)
            .expect("Unable to write to PlatformLogWriter");

        writer.copy_bytes_to_start(0, 20);
        writer.copy_bytes_to_start(10, 0);

        assert_eq!(
            unsafe { assume_init_slice(&writer.buffer[..test_string.len()]) },
            test_string.as_bytes()
        );
    }

    fn get_tag_writer() -> PlatformLogWriter<'static> {
        PlatformLogWriter::new(
            None,
            Level::Warn,
            CStr::from_bytes_with_nul(b"tag\0").unwrap(),
        )
    }

    unsafe fn assume_init_slice<T>(slice: &[MaybeUninit<T>]) -> &[T] {
        &*(slice as *const [MaybeUninit<T>] as *const [T])
    }
}
