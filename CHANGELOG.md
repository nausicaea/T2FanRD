## v0.4.10 (2026-10-17)

### Fix

- **deps**: update rust crate signal-hook to v0.4.5 (#86)
- **deps**: update rust crate libc to v0.2.190 (#85)
- **deps**: update rust crate sentry to v0.49.3 (#80)
- **deps**: update rust crate thiserror to v2.0.21 (#81)
- **deps**: update rust crate clap to v4.6.7 (#73)

## v0.4.9 (2026-09-14)

### Fix

- silent exit of the fan daemon (#69)

## v0.4.8 (2026-09-13)

### Fix

- **observability**: reduce the sentry boilerplate to the minimum (#66)

## v0.4.7 (2026-09-12)

### Fix

- **deps**: update sentry-rust monorepo to v0.49.2 (#63)
- **deps**: update rust crate log to v0.4.34 (#62)
- **deps**: update rust crate clap to v4.6.6 (#53)
- **deps**: update rust crate thiserror to v2.0.20 (#54)
- **deps**: pin dependencies (#50)
- **deps**: update sentry-rust monorepo to 0.49.0 (#43)
- remove abandoned dependencies (#47)
- pin the actions/checkout version to a specific hash (#44)

## v0.4.6 (2026-07-30)

## v0.4.5 (2026-07-17)

### Fix

- Extension Service Woes (#7)

## v0.4.4 (2026-06-07)

## v0.4.3 (2026-06-06)

## v0.4.1 (2026-06-05)

## v0.4.0 (2026-06-05)

### Refactor

- hide the compile error for non-linux systems behind a negative off-by-default feature
- add the observability feature by default for the docker builds
- removed a few features from sentry, and switched around the features
- move the root check and the flock(2) acquisition behind a default feature gate

## v0.3.3 (2026-06-05)

### Refactor

- strip down the t2fanrd container manifest, and bump the version

## v0.3.2 (2026-06-05)

### Refactor

- optimize the binary for size and reduce duplication in the docker image

## v0.3.1 (2026-06-05)

### Refactor

- add Display to FanComponent, and fix a few mostly infallible errors
- add fan component information to the errors (i.e. input or manual, etc)
- remove the need for MinSpeed and MaxSpeed error variants
- introduce a nested error type for fan operations
- introduce a nested error type for config parsing
- replace the debug logging on FanController with a more readable log
- reorder the main() and real_main() functions to the top of src/main.rs
- parametrize the config file loading
- track the current speed and log only if the speed was changed
- replace control flow with clamp()
- unify error reporting in `main()` into a single call to `log::error!()`
- rustfmt
- replace the `FanController::set_manual` loop in `real_main` with RAII via `Drop` impl on `FanController`.
- rustfmt
- use the more idiomatic max method over explicit control flow
- since file trunctation is performed on every write now with write_trunc!(), I don't need the truncate(true) open options
- rustfmt
- we don't need to explicitly drop the process lock
- elide the needless infallible conversion from usize to u32 and add a comment why
- add a comment why we don't specify truncation info for the process lock
- fix a typo in Error::TempParse
- add explicit try_from for u32 and u8 when calculating the mean temperature
- be explicit and specific about where I allow truncation, precision loss, and sign loss
- replace `as` conversions with explicit From usage
- QA remove the global clippy exceptions
- move temperature buffer clearing to before reading from the temperature file in /sys, and elide a temporary binding
- remove the Fan newtype again (code-review said it adds too much noise)
- loosen the lifetime restrictions on FanController::write_property
- rustfmt
- replace manual PID-file management with a flock(2) process lock
- make the empty tuple argument to a closure more explicit
- replace control flow with a type conversion from bool to usize
- remove the unnecessary stdout locking and flushing in favor of a bare logging call
- rustfmt
- import std::fs::File
- rustfmt
- simplify the PID-File removal at the end
- parametrize check_pid_file

## v0.2.0 (2026-06-03)

### Refactor

- rustfmt

## v0.1.1 (2026-06-02)

### Refactor

- remove some of Claude's comments
- replace the manual bind mount of the config file with a dependency on ExtensionServiceConfig

## v0.1.0 (2026-06-01)

### Fix

- Latest temperature may be insignificant in average computation (#7)
