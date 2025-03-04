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
    pub(crate) const fn to_native(log_id: Option<Self>) -> Option<log_ffi::log_id_t> {
        match log_id {
            Some(Self::Main) => Some(log_ffi::log_id_t::MAIN),
            Some(Self::Radio) => Some(log_ffi::log_id_t::RADIO),
            Some(Self::Events) => Some(log_ffi::log_id_t::EVENTS),
            Some(Self::System) => Some(log_ffi::log_id_t::SYSTEM),
            Some(Self::Crash) => Some(log_ffi::log_id_t::CRASH),
            Some(Self::Kernel) => Some(log_ffi::log_id_t::KERNEL),
            Some(Self::Security) => Some(log_ffi::log_id_t::SECURITY),
            Some(Self::Stats) => Some(log_ffi::log_id_t::STATS),
            None => None,
        }
    }
}
