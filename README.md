# crap4rust

Native Rust CRAP analysis. The executable is written in Rust and parses Rust with `syn`. It reads LCOV output from `cargo-llvm-cov` and requires complete function coverage by default.

## Requirements

- Rust 1.82 or later.
- `llvm-tools-preview` and `cargo-llvm-cov` for coverage generation.
- No Python, Node, JVM, or other language runtime.

## Install

```bash
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov --locked
cargo install --git https://github.com/lukasa1993/crap4rust --locked --force
```

The repository commits `Cargo.lock`. Use `--locked` so installation uses the dependency graph tested by CI.

## Run

```bash
crap4rust --fail-over 6
```

The default test command is:

```bash
cargo llvm-cov --workspace --all-features --lcov --output-path target/coverage/lcov.info
```

Use `--test-command` when the workspace has a different supported feature matrix. Use `--no-test` to read an existing LCOV report.

Exit status: `0` pass, `1` execution/parse/coverage error, `2` CRAP limit failure.
