## A logger which uses android logging backend

This, of course, works only under android and requires linking to `log` which
is only available under android.

Example:

```rust
#[macro_use] extern crate log;
extern crate android_logger;

use log::LogLevel;

fn native_activity_create() {
    android_logger::init(LogLevel::Trace).unwrap(); // trace == verbose

    trace!("this is a verbose {}", "message");
    error!("this is printed by default");
}
```

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
