use android_logger::AndroidLogger;
use android_logger::Config;
use log::LevelFilter;
use std::sync::OnceLock;

#[test]
fn test_debug() {
    static ANDROID_LOGGER: OnceLock<AndroidLogger> = OnceLock::new();
    let android_logger = ANDROID_LOGGER
        .get_or_init(|| AndroidLogger::new(Config::default().with_max_level(LevelFilter::Trace)));
    assert_eq!("AndroidLogger", format!("{:?}", android_logger));
}
