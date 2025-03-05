use crate::{FormatFn, LogId};
use log::{Level, LevelFilter, Record};
use std::ffi::CString;
use std::fmt;

/// Filter for android logger.
#[derive(Default)]
pub struct Config {
    pub(crate) log_level: Option<LevelFilter>,
    pub(crate) buf_id: Option<LogId>,
    filter: Option<env_filter::Filter>,
    pub(crate) tag: Option<CString>,
    pub(crate) custom_format: Option<FormatFn>,
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("log_level", &self.log_level)
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
fn default_is_loggable(_tag: &str, record_level: Level, config_level: Option<LevelFilter>) -> bool {
    record_level <= config_level.unwrap_or_else(log::max_level)
}

#[cfg(all(target_os = "android", feature = "android-api-30"))]
fn android_is_loggable(tag: &str, record_level: Level, config_level: Option<LevelFilter>) -> bool {
    let prio = android_log_priority_from_level(record_level);
    // Priority to use in case no system-wide or process-wide overrides are set.
    let default_prio = match config_level {
        Some(level_filter) => match level_filter.to_level() {
            Some(level) => android_log_priority_from_level(level),
            // LevelFilter::to_level() returns None only for LevelFilter::Off
            None => android_log_sys::LogPriority::SILENT,
        },
        None => android_log_sys::LogPriority::INFO,
    };
    android_is_loggable_len(prio, tag, default_prio)
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

    pub(crate) fn filter_matches(&self, record: &Record) -> bool {
        if let Some(ref filter) = self.filter {
            filter.matches(record)
        } else {
            true
        }
    }

    pub(crate) fn is_loggable(&self, tag: &str, level: Level) -> bool {
        #[cfg(all(target_os = "android", feature = "android-api-30"))]
        use android_is_loggable as is_loggable;
        #[cfg(not(all(target_os = "android", feature = "android-api-30")))]
        use default_is_loggable as is_loggable;

        is_loggable(tag, level, self.log_level)
    }

    pub fn with_filter(mut self, filter: env_filter::Filter) -> Self {
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
