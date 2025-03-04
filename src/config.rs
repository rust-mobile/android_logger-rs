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
            .field("custom_format", match &self.custom_format {
                Some(_) => &"Some(_)",
                None => &"None",
            })
            .finish()
    }
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

    pub(crate) fn is_loggable(&self, level: Level) -> bool {
        // todo: consider __android_log_is_loggable.
        level <= self.log_level.unwrap_or_else(log::max_level)
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
