## Send Rust logs to Logcat

[![Version](https://img.shields.io/crates/v/android_logger.svg)](https://crates.io/crates/android_logger)
[![Build Status](https://travis-ci.org/Nercury/android_logger-rs.svg?branch=master)](https://travis-ci.org/Nercury/android_logger-rs)

This library is a drop-in replacement for `env_logger`. Instead, it outputs messages to
android's logcat.

This only works on Android and requires linking to `log` which
is only available under android. With Cargo, it is possible to conditionally require
this library:

```toml
[target.'cfg(target_os = "android")'.dependencies]
android_logger = "0.5"
```

Example of initialization on activity creation, with log filters:

```rust
#[macro_use] extern crate log;
extern crate android_logger;

use log::LogLevel;
use android_logger::Filter;

fn native_activity_create() {
    android_logger::init_once(
        Filter::default()
            .with_min_level(Level::Trace) // limit log level
            .with_allowed_module_path("hello::crate") // limit messages to specific crate
    ); 

    trace!("this is a verbose {}", "message");
    error!("this is printed by default");
}
```

To allow all logs, use the default filter with min level Trace:

```rust
#[macro_use] extern crate log;
extern crate android_logger;

use android_logger::Filter;

fn native_activity_create() {
    android_logger::init_once(Filter::default()
                              .with_min_level(Level::Trace));
}
```

There is a caveat that this library can only be initialized once 
(hence the `init_once` function name). However, Android native activity can be
re-created every time the screen is rotated, resulting in multiple initialization calls.
Therefore this library will only log a warning for subsequent `init_once` calls.

This library ensures that logged messages do not overflow Android log message limits
by efficiently splitting messages into chunks.

## License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.
