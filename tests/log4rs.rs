#![cfg(feature = "log4rs")]

use log::{debug, error, info, trace, warn, LevelFilter};

#[test]
fn test_log4rs() {
    use log4rs::append::console::ConsoleAppender;
    use log4rs::config::{Appender, Root};
    use log4rs::encode::pattern::PatternEncoder;
    use log4rs::Config;
    use android_logger::Config as AndroidConfig;

    let android_logger = android_logger::get_instance(AndroidConfig::default().with_max_level(LevelFilter::Trace));
    let stdout = ConsoleAppender::builder()
        .encoder(Box::new(PatternEncoder::new("{m}{n}")))
        .build();
    match Config::builder()
        .appender(Appender::builder().build("stdout", Box::new(stdout)))
        .appender(Appender::builder().build("android_logger", Box::new(android_logger)))
        .build(
            Root::builder()
                .appender("stdout")
                .appender("android_logger")
                .build(LevelFilter::Debug),
        ) {
        Ok(config) => {
            let handle = log4rs::init_config(config);
            if let Err(e) = handle {
                println!("ERROR: failed to configure logging for stdout with {e:?}");
            }
        }
        Err(e) => {
            println!("ERROR: failed to prepare default logging configuration with {e:?}");
        }
    }
    // This will not be logged to the Console because of its category's custom level filter.
    info!(target: "Settings", "Info");

    warn!(target: "Settings", "Warn");
    error!(target: "Settings", "Error");

    trace!("Trace");
    debug!("Debug");
    info!("Info");
    warn!(target: "Database", "Warn");
    error!("Error");
}