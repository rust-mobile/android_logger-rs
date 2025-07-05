use log::LevelFilter;

#[test]
fn config_log_level() {
    android_logger::init_once(android_logger::Config::default().filter_level(LevelFilter::Trace));

    assert_eq!(log::max_level(), log::LevelFilter::Trace);
}
