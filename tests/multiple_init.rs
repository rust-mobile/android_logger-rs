use android_logger::FilterBuilder;
use log::LevelFilter;

#[test]
fn multiple_init() {
    android_logger::init_once(
        android_logger::Config::default().with_filter(
            FilterBuilder::new()
                .filter_level(LevelFilter::Trace)
                .build(),
        ),
    );

    // Second initialization should be silently ignored
    android_logger::init_once(
        android_logger::Config::default().with_filter(
            FilterBuilder::new()
                .filter_level(LevelFilter::Error)
                .build(),
        ),
    );

    assert_eq!(log::max_level(), LevelFilter::Trace);
}
