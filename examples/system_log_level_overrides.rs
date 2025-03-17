//! An utility for testing the behavior of `android_logger` crate.
//!
//! ## Build
//!
//! 1. Setup [`cargo-ndk`](https://github.com/bbqsrc/cargo-ndk)
//!
//!    ```
//!    cargo install cargo-ndk
//!    rustup target add x86_64-linux-android
//!    ```
//!
//! 2. Build with `cargo ndk`:
//!
//!    ```
//!    ANDROID_NDK_HOME=/usr/lib/android-sdk/ndk/27.1.12297006 \
//!        cargo ndk -t x86_64 build --release --features android-api-30 \
//!        --example system_log_level_overrides
//!    ```
//!
//! ## Run on emulator
//!
//! 1. Grab a [Cuttlefish](https://source.android.com/docs/devices/cuttlefish)
//!    virtual device + Android build from [Android
//!    CI](https://ci.android.com/builds/branches/aosp-main/grid?legacy=1). Select
//!    the last green `aosp_cf_x86_64_phone` `trunk_staging-userdebug` build and
//!    open "Artifacts" link, download:
//!
//!    - `aosp_cf_x86_64_phone-img-BUILDNUMBER.zip`
//!    - `cvd-host_package.tar.gz`
//!
//! 2. Unpack both archives & start the emulator.
//!
//!    ```
//!    cd $(mktemp -d)
//!    unzip ~/Downloads/aosp_cf_x86_64_phone-img-*.zip
//!    tar xf ~/Downloads/cvd-host_package.tar.gz
//!    HOME=$PWD bin/launch_cvd
//!    ```
//!
//!    Once emulator launches, `adb` should detect it on `0.0.0.0:6520`
//!    automatically. Shut down the `launch_cvd` command to exit the emulator.
//!
//! 3. Upload & run:
//!
//!    ```
//!    adb push ./target/x86_64-linux-android/release/examples/system_log_level_overrides /data/local/tmp/
//!    adb shell /data/local/tmp/system_log_level_overrides
//!    ```
//!
//! ## Test interaction with Android system properties
//!
//! See [`logd`
//! README](https://cs.android.com/android/platform/superproject/main/+/main:system/logging/logd/README.property)
//! in AOSP for details.
//!
//! ```
//! # default: should print info+ logs in `adb logcat -s log_test`
//! # hint: use `adb logcat -v color` is awesome too
//! adb shell /data/local/tmp/system_log_level_overrides
//!
//! # should print trace+ logs in `adb logcat -s log_test`
//! adb shell setprop log.tag V
//! adb shell /data/local/tmp/system_log_level_overrides
//!
//! # should print warn+ logs in `adb logcat -s log_test`
//! adb shell setprop log.tag.log_test W
//! adb shell /data/local/tmp/system_log_level_overrides
//! ```

fn main() {
    android_logger::init_once(
        android_logger::Config::default()
            .with_tag("log_test")
            // If set, this is the highest level to log unless overriddeby by the system.
            // Note the verbosity can be *increased* through system properties.
            .with_max_level(log::LevelFilter::Info),
    );
    // The log crate applies its filtering before we even get to android_logger.
    // Pass everything down so that Android's liblog can determine the log level instead.
    log::set_max_level(log::LevelFilter::Trace);

    log::trace!("trace");
    log::debug!("debug");
    log::info!("info");
    log::warn!("warn");
    log::error!("error");
}
