## Send Rust logs to Logcat

[![Version](https://img.shields.io/crates/v/android_logger.svg)](https://crates.io/crates/android_logger)
[![CI status](https://github.com/rust-mobile/android_logger-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/rust-mobile/android_logger-rs/actions/workflows/ci.yml/)


This library is a drop-in replacement for `env_logger`. Instead, it outputs messages to
android's logcat.

This only works on Android and requires linking to `liblog` which
is only available under Android. With Cargo, it is possible to conditionally
include this crate and library requirement when targeting Android only:

```toml
[target.'cfg(target_os = "android")'.dependencies]
android_logger = "0.15"
```

### Examples

#### Example of initialization on activity creation, with log configuration

```rust
use android_logger::Config;

fn native_activity_create() {
    android_logger::init_once(
        Config::default()
            .with_tag("mytag") // logs will show under mytag tag
            .parse_filters("debug,hello::crate=error") // Limig log level to Debug, and limit the  hello::crate module further to Error.
    );

    log::debug!("this is a verbose {}", "message");
    log::error!("this is printed by default");
}
```

To allow all logs, use the default configuration with the global module level set to `Trace`

```rust
use android_logger::Config;

fn native_activity_create() {
    android_logger::init_once(
        Config::default().filter_level(log::LevelFilter::Trace),
    );
}
```

#### Example with a custom log formatter

```rust
use android_logger::Config;

android_logger::init_once(
    Config::default()
        .format(|f, record| write!(f, "my_app: {}", record.args()))
)
```

### Single-initialization guarantee

There is a caveat that this library can only be initialized once
(hence the `init_once` function name). However, Android native activity can be
re-created every time the screen is rotated, resulting in multiple initialization calls.
Therefore this library will only log a warning for subsequent `init_once` calls.

###

This library ensures that logged messages do not overflow Android log message limits
by efficiently splitting messages into chunks.

### Consistent log filtering in mixed Rust/C/C++ apps

Android's C logging API determines the effective log level based on [the combination] of a process-wide global variable, [system-wide properties], and call-specific default. `log` + `android_logger` crates add another layer of log filtering on top of that, independent from the C API.

[the combination]: https://cs.android.com/android/platform/superproject/main/+/main:system/logging/liblog/properties.cpp;l=243;drc=b74a506c1b69f5b295a8cdfd7e2da3b16db15934
[system-wide properties]: https://cs.android.com/android/platform/superproject/main/+/main:system/logging/logd/README.property;l=45;drc=99c545d3098018a544cb292e1501daca694bee0f

```text
    .-----.
    | app |
    '-----'     Rust
C/C++ | '--------------.
      |                v
      |          .-----------.   filter by log::STATIC_MAX_LEVEL +
      |          | log crate | - log::MAX_LOG_LEVEL_FILTER,
      |          '-----------'   overrideable via log::set_max_level
      |                |
      |                v
      |     .----------------------.
      |     | android_logger crate | - filter by Config::max_level
      |     '----------------------'
      |                |
      |   .------------'
      v   v
   .--------.
   | liblog | - filter by global state or system-wide properties
   '--------'
```

`liblog` APIs introduced in Android API 30 let `android_logger` delegate log
filtering decisions to `liblog`, making the log level consistent across C, C++
and Rust calls.

If you build `android_logger` with the `android-api-30` feature enabled, the logger
will consider the process-wide global state (set via
[`__android_log_set_minimum_priority`](https://cs.android.com/android/platform/superproject/main/+/main:prebuilts/runtime/mainline/runtime/sdk/common_os/include/system/logging/liblog/include/android/log.h;l=364;drc=4cf460634134d51dba174f8af60dffb10f703f51))
and Android system properties when deciding if a message should be logged or
not. In this case, the effective log level is the _least verbose_ of the levels
set between those and [Rust log facilities].

[Rust log facilities]: https://docs.rs/log/latest/log/fn.set_max_level.html

### License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.
