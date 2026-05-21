# Changelog

All notable changes to this project are documented here. The format is loosely
based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this
project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [2.0.0] - 2025-08-30

- Bump Rust edition to 2024; minimum supported Rust version raised accordingly.
- Replace `lazy_static!` with `std::sync::LazyLock` (standard-library only).
- Use new `proc_macro` span APIs to get the next token with no gap.
- New `build-print` feature: enable `build.rs` logging.
- Add non-global ways to set debug and pipefail modes (thread-local overrides).
- Fix thread unsafety caused by internal `set_var()` use; work around
  `set_pipefail()` thread-safety bug.
- Replace `wait_with_pipe()` with `wait_with_borrowed_pipe()`; fix hang when
  called with built-ins.
- Fix handling of `ignore` in `wait()` and `wait_with_all()`.
- Inherit tracing spans when the program uses `tracing`.
- Make stderr logging handle CR-separated progress output.
- Support command environment variable values that contain `=`.
- Include license text in the packaged macros crate.
- Large refactor and new `test_ignore_and_pipefail()` test suite covering most
  entry points.

## [1.9.6] - 2025-06-14

- Support more formats for interpolation strings.
- Support `${variable:?}` in interpolation strings.
- Examples: port from `structopt` to `clap`/derive.

## [1.9.5] - 2024-10-01

- Fix `pipefail` issue for the `run_fun!()` macro.
- Report full commands in errors to give more context.
- Switch from `proc-macro-error` to `proc-macro-error2`.
- Consolidate pipe-related test cases.

## [1.9.4] - 2024-05-12

- Skip empty arguments.

## [1.9.3] - 2023-11-29

- Remove `MainResult` and `MainError`.

## [1.9.2] - 2023-11-29

- Update `FunChildren` APIs.

## [1.9.1] - 2023-11-24

- Append command location information into errors.
- Update custom-command API signature.
- Hide `CmdIn` / `CmdOut` implementation details.
- Fix unit testing of macros.

## [1.9.0] - 2023-11-23

- Refactor custom-command registration.

## [1.8.1] - 2023-11-19

- New API to catch all output and return code.
- Documentation updates.

## [1.8.0] - 2023-11-02

- Add `cmd_lib::main` attribute macro to log `main()` errors by default.
- Support empty commands and empty commands in pipelines.

## [1.6.1] - 2023-10-29

- Support `-n` option for the builtin `echo` command.

## [1.6.0] - 2023-10-24

- Refactor logging mechanism for better readability.
- Cleaner printing of `ignore`, `cd`, and redirect operations.

## [1.5.0] - 2023-10-20

- Better error reporting when running commands.

## [1.4.0] - 2023-10-18

- Use `env_logger` as the default logger.
- Allow nested `cd` commands inside group commands.
- Support iterators that produce `OsStr`.
- Use arrays (instead of vectors) for command arguments.
- Add API to get PIDs of pipeline processes; support killing a pipeline.
- Improved error reporting when spawning processes fails.
- Import `main_error` crate for nicer top-level error reporting.

## [1.3.0] - 2021-10-16

- Allow `ignore` to be used for builtin commands; allow ignoring spawn errors.
- New flag to control command error reporting; broader error-reporting
  refactor.
- Move stderr polling into a separate structure.
- Low-level API cleanup.

## [1.2.4] - 2021-09-27

- Allow `ignore` to ignore all errors inside pipelines.

## [1.2.3] - 2021-09-27

- Fix issue #36; cleanup around log waiting.

## [1.2.2] - 2021-09-20

- Fix `ignore` behavior for builtin commands.
- Always report thread-join errors.
- Warn-level messages only emitted when debug flag is enabled.

## [1.2.1] - 2021-09-19

- Fix builtin-command redirection bug.

## [1.2.0] - 2021-09-19

- Launch a separate thread to print stderr log (fix #35).
- Read stdout/stderr buffers before and after `wait`.
- Fix pipe-full issue for builtin commands.
- Check thread-join return values.

## [1.1.0] - 2021-06-12

- Add builtin `ignore` command.
- `run_cmd!()` and `spawn!()` no longer capture output.
- Remove support for the `||` command operator.
- Update `wait_with_pipe()` API.
- Fix pipe race condition in tests.

## [1.0.13] - 2021-05-05

- Allow `Path` / `OsString` variables to be used directly in commands.
- Convert variables to `OsString` via `into_os_string()`; replace
  `IntoOsString` trait with `AsOsStr`.
- Print `OsString` values without `unwrap()`.
- Fix `dd_test` display regression.

## [1.0.12] - 2021-04-27

- Support stream API with `wait_with_pipe()`.
- Process cleanup: simplify wait structure; move `CmdChildren` to `child.rs`.
- Rename `wait_cmd` / `wait_fun` functions; remove `Child` postfix from APIs.
- Print messages so that `cargo test` is happy.

## [1.0.11] - 2021-04-23

- Update `cmd_die!()` macro to return the `!` (never) type.
- Use iterators for parser; embed iterator into `Lexer` struct.
- Return size 0 for `/dev/null` reads.
- Fix stderr logging (#23).
- Large internal cleanup of lexer, parser, child, and process modules.

## [1.0.10] - 2021-04-05

- Optimize handling of `/dev/null`.
- Reject the same redirection being set multiple times.
- Spawn a separate thread for builtin/custom pipe-out threads, and check its
  status.

## [1.0.9] - 2021-04-03

- Fix stderr piping bug.
- Fix macOS testing error.

## [1.0.8] - 2021-03-31

- Continued work on the `log` crate dependency.

## [1.0.7] - 2021-03-31

- Add `log` crate dependency for `cmd_lib_macros`.

## [1.0.6] - 2021-03-31

- Support `|&` operation; tighter checks around `&` formats.
- Optimize for `/dev/null`; fix redirection-all-to-a-file.
- Simplify debug strings.

## [1.0.5] - 2021-03-30

- Log builtin/custom command's stderr content.
- Fix builtin command stderr redirection.
- Report `cd` command errors.

## [1.0.4] - 2021-03-29

- Ensure command-running errors are logged across all code paths.
- Suppress errors when `or_cmd` can run.
- Log errors for `wait_raw_result()`.

## [1.0.3] - 2021-03-29

- Add `cmd_echo!` support and print all log levels by default.
- Default logging level set to debug.
- Capture all stderr messages into logs.

## [1.0.2] - 2021-03-29

- Add log levels for `warn!()` and `error!()`.
- Log error messages when running commands fails.

## [1.0.1] - 2021-03-28

- Add logging support.
- Fix empty command with variable settings.
- Only take child's stdin for previous builtin/custom commands.
- Lexer cleanup.

## [1.0.0] - 2021-03-28

- First stable release.
- Update builtin/custom command registration API.
- Remove `vars()` API from `CmdEnv`.

## [0.15.1] - 2021-03-27

- Refactor `process` module; tidier `std_cmd` matching.

## [0.15.0] - 2021-03-27

- Move command vector into the lower-level `Cmd` struct; carry more
  information on `Cmd` itself.
- Update `CmdStdio` APIs.
- Fix `wait_raw_result()` wait bug.
- Simplify `current_dir` setting and debug-string printing.

## [0.14.6] - 2021-03-26

- Don't panic when opening files fails; always wait for children even on
  failure.
- Parser cleanup.

## [0.14.5] - 2021-03-26

- Support null command.
- Refactor `RedirectFd` struct and redirection parsing.
- Apply Rust 1.51 clippy suggestions.
- Update crate keywords.

## [0.14.4] - 2021-03-24

- Support pipe in / stdin for builtin and custom commands.
- Lexer cleanup.

## [0.14.3] - 2021-03-24

- Release global `CMD_MAP` lock during execution.
- Fix redirection-format check; lexer refactor.

## [0.14.2] - 2021-03-24

- Update `ProcHandle` enum.
- Fix string-literal parsing.

## [0.14.1] - 2021-03-23

- Cleanup of builtin-command implementation.

## [0.14.0] - 2021-03-23

- Replace `die!()` macro with `cmd_die!()`.
- Update builtin-command API interface.
- Support basic stdout/stderr redirection for builtin commands.
- Update docs on registering custom commands.

## [0.12.6] - 2021-03-23

- Add `cmd_info!()` macro.
- Allow more valid tokens after pipe; check more invalid redirection formats.
- Fix raw fd redirection issues.
- Fix redirection bug for `run_fun!`.
- Lexer refactor.

## [0.12.5] - 2021-03-21

- Fix redirect bug; fail more invalid redirections.

## [0.12.4] - 2021-03-21

- Show envs and redirects when running in debug mode.

## [0.12.3] - 2021-03-21

- Clean up `current_dir` setting.
- Move children out of `Cmds` struct.

## [0.12.2] - 2021-03-21

- Simplify argument parsing in examples (`clap` / `structopt`).
- Re-revert "Removed lazy_static package dependency".
- Fix `current_dir` setting issues.

## [0.12.1] - 2021-03-19

- Better error messages for macro compilation; use `proc_macro_error` crate.
- Add support for escaped characters.
- Parse variables in a single pass; cleaner peekable iterator.
- CI: introduce `rust.yml` workflow.

## [0.12.0] - 2021-03-09

- Add `spawn_with_output!()` macro.
- Refactor to allow builtin commands in pipes.
- Rename `proc_var*` to `tls_*`.
- Add `set_pipefail()` API; replace `try_wait()` with `wait()`.
- Drop `tempfile` dependency.
- Many examples/test fixes (tetris, pipes, pipefail on macOS).

## [0.12.0-rc1] - 2021-03-08

- Release candidate for 0.12.0.

## [0.11.6] - 2021-03-03

- Update logging macro/functions.

## [0.11.5] - 2021-03-03

- Make `pipefail` the default.
- Add `die!` macro and `info()` builtin.
- More logging-related builtin commands; more examples.
- Drop `lazy_static` package dependency.

## [0.11.4] - 2021-03-02

- Add `spawn` macro with compile-time check.

## [0.11.3] - 2021-03-02

- Hide builtin functions from public API.

## [0.11.2] - 2021-03-02

- Rename `use_cmd` to `use_custom_cmd`.
- Support builtin commands.

## [0.11.1] - 2021-03-01

- Macro library cleanup; doc typo fix.

## [0.11.0] - 2021-03-01

- Move parser from runtime into compile time (proc-macro parsing).
- Refactor `process` implementation.

## [0.10.5] - 2021-03-01

- Fix cargo-readme code-highlighting issue.
- Fix doctest failure.

## [0.10.4] - 2021-03-01

- Documentation updates; tokenstream cleanup.

## [0.10.3] - 2021-02-28

- New compile-time pipeline implementation: pipes, string literals,
  semicolons, variable passing, `Or` command, vector variables, environment
  variable passing, stdin/stdout/stderr/fd redirection.
- Introduce new lexer with `SepToken` and `MarkerToken`.
- Unify `run_cmd` and `run_fun` macros.

## [0.10.2] - 2021-02-24

- Add variable-substitution rules.
- Add `config_cmd` / `export_cmd` / `use_cmd` APIs; `set_debug` API.
- Hide internal APIs.

## [0.10.1] - 2021-02-21

- Better error reporting for proc-macros.
- Skip invalid variable names.
- Clean up `Cargo.toml` for `proc-macro2`.

## [0.10.0] - 2021-02-21

- First release published to crates.io.
