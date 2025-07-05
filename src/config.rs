use crate::{FormatFn, LogId};
use log::{Level, LevelFilter, Record};
use std::ffi::CString;
use std::fmt;

/// Filter for android logger.
// #[derive(Default)]
// TODO: Rename to Builder.
pub struct Config {
    pub(crate) buf_id: Option<LogId>,
    pub(crate) filter: env_filter::Builder,
    pub(crate) tag: Option<CString>,
    pub(crate) custom_format: Option<FormatFn>,
}

impl Default for Config {
    /// Creates a default config that logs all modules at the [`LevelFilter::Error`] level by
    /// default, when no other filters are set.
    // TODO: Parse from env?
    fn default() -> Self {
        Self {
            buf_id: None,
            // TODO: This doesn't read from an env var like RUST_LOG...
            filter: env_filter::Builder::new(),
            tag: None,
            custom_format: None,
        }
    }
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("buf_id", &self.buf_id)
            .field("filter", &self.filter)
            .field("tag", &self.tag)
            .field(
                "custom_format",
                match &self.custom_format {
                    Some(_) => &"Some(_)",
                    None => &"None",
                },
            )
            .finish()
    }
}

#[cfg(all(target_os = "android", feature = "android-api-30"))]
fn android_log_priority_from_level(level: Level) -> android_log_sys::LogPriority {
    match level {
        Level::Warn => android_log_sys::LogPriority::WARN,
        Level::Info => android_log_sys::LogPriority::INFO,
        Level::Debug => android_log_sys::LogPriority::DEBUG,
        Level::Error => android_log_sys::LogPriority::ERROR,
        Level::Trace => android_log_sys::LogPriority::VERBOSE,
    }
}

/// Asks Android liblog if a message with given `tag` and `priority` should be logged, using
/// `default_prio` as the level filter in case no system- or process-wide overrides are set.
#[cfg(all(target_os = "android", feature = "android-api-30"))]
fn android_is_loggable_len(
    prio: log_ffi::LogPriority,
    tag: &str,
    default_prio: log_ffi::LogPriority,
) -> bool {
    // SAFETY: tag points to a valid string tag.len() bytes long.
    unsafe {
        log_ffi::__android_log_is_loggable_len(
            prio as log_ffi::c_int,
            tag.as_ptr() as *const log_ffi::c_char,
            tag.len() as log_ffi::c_size_t,
            default_prio as log_ffi::c_int,
        ) != 0
    }
}

#[cfg(not(all(target_os = "android", feature = "android-api-30")))]
pub(crate) fn is_loggable(_tag: &str, _record_level: Level) -> bool {
    // There is nothing to test here, the `log` macros already checked the variable
    // `log::max_level()` before calling into the implementation.
    // The tests ensure this by creating and calling into `AndroidLogger::log()` without
    // `set_max_level()` from `init_once()`, and expect the message to be logged.
    true
}

#[cfg(all(target_os = "android", feature = "android-api-30"))]
pub(crate) fn is_loggable(tag: &str, record_level: Level) -> bool {
    let prio = android_log_priority_from_level(record_level);
    // Priority to use in case no system-wide or process-wide overrides are set.
    // WARNING: Reading live `log::max_level()` state here would break tests, for example when
    // `AndroidLogger` is constructed and `AndroidLogger::log()` is called _without_ going through
    // `init_once()` which would call `log::set_max_level()`, leaving this at `Off`.  Currently no
    // tests exist that run on live Android devices and/or mock `__android_log_is_loggable_len()`
    // such that this function can be called.
    let default_prio = match log::max_level().to_level() {
        Some(level) => android_log_priority_from_level(level),
        // LevelFilter::to_level() returns None only for LevelFilter::Off
        None => android_log_sys::LogPriority::SILENT,
    };
    android_is_loggable_len(prio, tag, default_prio)
}

impl Config {
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

    /// Adds a directive to the filter for a specific module.
    ///
    /// Note that this replaces the default [`LevelFilter::Error`] for all global modules.
    pub fn filter_module(mut self, module: &str, level: LevelFilter) -> Self {
        self.filter.filter_module(module, level);
        self
    }

    /// Adds a directive to the filter for all modules.
    pub fn filter_level(mut self, level: LevelFilter) -> Self {
        self.filter.filter_level(level);
        self
    }

    /// Parses the directives string in the same form as the `RUST_LOG`
    /// environment variable.
    ///
    /// See the `env_logger` module documentation for more details.
    pub fn parse_filters(mut self, filters: &str) -> Self {
        self.filter.parse(filters);
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
