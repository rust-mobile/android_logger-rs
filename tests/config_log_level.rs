use android_logger::FilterBuilder;
use log::LevelFilter;

#[test]
fn config_log_level() {
    android_logger::init_once(
        android_logger::Config::default().with_filter(
            FilterBuilder::new()
                .filter_level(LevelFilter::Trace)
                .build(),
        ),
    );

    assert_eq!(log::max_level(), log::LevelFilter::Trace);
}
