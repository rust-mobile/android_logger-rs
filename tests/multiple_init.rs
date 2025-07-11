use log::LevelFilter;

#[test]
fn multiple_init() {
    android_logger::init_once(android_logger::Config::default().filter_level(LevelFilter::Trace));

    // Second initialization should be silently ignored
    android_logger::init_once(android_logger::Config::default().filter_level(LevelFilter::Error));

    assert_eq!(log::max_level(), LevelFilter::Trace);
}
