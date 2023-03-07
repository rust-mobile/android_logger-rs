`android_logger` changelog
==========================

All user visible changes to this project will be documented in this file. This project uses [Semantic Versioning 2.0.0].




## [0.13.1] · 2023-03-07
[0.13.1]: /../../tree/v0.13.1

[Diff](/../../compare/v0.13.0...v0.13.1)

### Fixed

- Missing logs on [Android] API 26 and earlier. ([#66], [#67])

[#66]: /../../issues/66
[#67]: /../../pull/67




## [0.13.0] · 2023-02-14
[0.13.0]: /../../tree/v0.13.0

[Diff](/../../compare/v0.12.0...v0.13.0)

### BC Breaks

- Added `buf_id` argument to `PlatformLogWriter::new()` method allowing to specify concrete Android logging system buffer. ([#50], [#64])
- Removed deprecated `Config::with_min_level()` method accepting `log::Level`. ([#65])

### Added

- `Config::with_log_buffer()` method to specify concrete Android logging system buffer. ([#50], [#64])

[#50]: /../../pull/50
[#64]: /../../pull/64
[#65]: /../../pull/65




## [0.12.0] · 2023-01-19
[0.12.0]: /../../tree/v0.12.0

[Diff](/../../compare/v0.11.3...v0.12.0)

### Added

- `Config::with_max_level()` method to filters logs via `log::LevelFilter`. ([#62])

### Deprecated

- `Config::with_min_level()` method accepting `log::Level`. ([#62])

### Fixed

- Incorrect logs level filtering. ([#62])

[#62]: /../../pull/62




## [0.11.3] · 2022-12-20
[0.11.3]: /../../tree/v0.11.3

[Diff](/../../compare/38186ece1056d90b8f75fd2a5eb5c860e0a1704e...v0.11.3)

### Fixed 

- Broken compilation on [Android] targets. ([#59], [#58])

[#58]: /../../issues/58
[#59]: /../../pull/59




## Previous releases

See [Git log](/../../commits/master?after=1a5a07ec6742f0069acc2be223c1bb3b6a9d15f8+0).




[Android]: https://www.android.com
[Semantic Versioning 2.0.0]: https://semver.org
