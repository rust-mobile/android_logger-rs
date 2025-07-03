#[test]
fn default_init() {
    android_logger::init_once(Default::default());

    // android_logger has default log level of Error
    // TODO: env_logger/env_filter have this too, but I cannot find it in the source code
    assert_eq!(log::max_level(), log::LevelFilter::Error);
}
