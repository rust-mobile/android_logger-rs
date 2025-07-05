use log::LevelFilter;

use super::*;
use std::sync::atomic::{AtomicBool, Ordering};

#[test]
fn check_config_values() {
    // Filter is checked in config_filter_match below.
    let config = Config::default()
        .with_log_buffer(LogId::System)
        .with_tag("my_app");

    assert_eq!(config.buf_id, Some(LogId::System));
    assert_eq!(config.tag, Some(CString::new("my_app").unwrap()));
}

#[test]
fn log_calls_formatter() {
    static FORMAT_FN_WAS_CALLED: AtomicBool = AtomicBool::new(false);
    let config = Config::default().format(|_, _| {
        FORMAT_FN_WAS_CALLED.store(true, Ordering::SeqCst);
        Ok(())
    });
    let logger = AndroidLogger::new(config);

    logger.log(&Record::builder().level(log::Level::Error).build());

    assert!(FORMAT_FN_WAS_CALLED.load(Ordering::SeqCst));
}

// TODO: How about deleting `fn enabled()` pretty much entirely?  It's only useful for implementing
// the new `android-api-30` `__android_log_is_loggable_len()` and without tests mocking that,
// bogus to call.  All filtering based on `log::max_level()` happens inside the `log::log!()`
// macros already, and otherwise relies on the tests to call `init_once()` to ensure we call
// `log::set_max_level()`.
//
// #[test]
// fn logger_enabled_threshold() {
//     let logger = AndroidLogger::new(
//         Config::default().with_filter(FilterBuilder::new().filter_level(LevelFilter::Info).build()),
//     );

//     assert!(logger.enabled(&log::MetadataBuilder::new().level(log::Level::Warn).build()));
//     assert!(logger.enabled(&log::MetadataBuilder::new().level(log::Level::Info).build()));
//     assert!(!logger.enabled(&log::MetadataBuilder::new().level(log::Level::Debug).build()));
// }

// Test whether the filter gets called correctly. Not meant to be exhaustive for all filter
// options, as these are handled directly by the filter itself.
#[test]
fn config_filter_match() {
    let info_record = Record::builder().level(log::Level::Info).build();
    let debug_record = Record::builder().level(log::Level::Debug).build();

    let info_all_config = Config::default().filter_level(LevelFilter::Info);
    let info_all_config = AndroidLogger::new(info_all_config);

    assert!(info_all_config.filter.matches(&info_record));
    assert!(!info_all_config.filter.matches(&debug_record));
}

#[test]
fn fill_tag_bytes_truncates_long_tag() {
    let too_long_tag = [b'a'; LOGGING_TAG_MAX_LEN + 20];

    let mut result = uninit_array();
    let tag = fill_tag_bytes(&mut result, &too_long_tag);

    let mut expected_result = vec![b'a'; LOGGING_TAG_MAX_LEN - 2];
    expected_result.extend("..\0".as_bytes());
    assert_eq!(tag.to_bytes_with_nul(), expected_result);
}

#[test]
fn fill_tag_bytes_keeps_short_tag() {
    let short_tag = [b'a'; 3];

    let mut result = uninit_array();
    let tag = fill_tag_bytes(&mut result, &short_tag);

    let mut expected_result = short_tag.to_vec();
    expected_result.push(0);
    assert_eq!(tag.to_bytes_with_nul(), expected_result);
}
