# crap4rust

`crap4rust` calculates the Change Risk Anti-Pattern metric for Rust functions and methods.

```text
CRAP = CC² × (1 - coverage)³ + CC
```

The analyzer is implemented in Python. It does not modify the target project.

## Install

```bash
pipx install git+https://github.com/lukasa1993/crap4rust.git
```

## Run

```bash
crap4rust --fail-over 6
```

The default test command is:

```bash
cargo llvm-cov --json --output-path target/coverage/coverage.json
```

`cargo llvm-cov --json` and LLVM coverage export JSON are supported.

Use a project-specific command when required:

```bash
crap4rust --test-command "<command that runs tests and exports coverage>"
```

Analyze an existing report:

```bash
crap4rust --no-test --coverage target/coverage/coverage.json
```

Use `--require-coverage` to fail when a function has no coverage data. Use `--json` for machine-readable output. Positional path fragments limit the analyzed files.

## Exit status

- `0`: analysis completed and the quality gate passed.
- `1`: the command or analysis failed.
- `2`: coverage is required but missing, or a score is above `--fail-over`.

## Development

```bash
python -m pip install -e . pytest
pytest -q
```
