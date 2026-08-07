# Contributing to tachyon

## The one rule that is specific to this project

**A change to `src/` is a change to what past readings mean.**

Scores from this tool get recorded next to benchmark timings and compared weeks
or months later. If a change alters the measurement — the access pattern, the
default working set, the chain construction, the units — then every previously
recorded score becomes incomparable with every new one, silently. There is no
error message for that.

So any PR touching **what determines a reading** must state, in the description,
either:

- **No change to what is measured** (refactor, docs, CI, tests), or
- **Changes what is measured**, with before/after scores from the *same machine*
  and a note on how to interpret old readings.

"What determines a reading" is not only `src/`. It also covers `[profile.release]`
in `Cargo.toml` — the optimised build *is* the measurement — and
`rust-toolchain.toml`, since a different compiler can generate a different chase.
A PR that touches only those files still needs the classification.

The PR template asks for this. It is the review question that matters most here.

## Development setup

### Prerequisites

- Rust stable (the toolchain is pinned in `rust-toolchain.toml`; rustup will
  fetch it automatically). MSRV is 1.85.
- Nothing else. The crate has no dependencies, and that is deliberate — see
  below.

### Install git hooks

```bash
./scripts/install-hooks.sh
```

Runs `cargo ci-fmt` and `cargo ci-lint` before each commit. Skip once with
`git commit --no-verify`.

### The three commands CI runs

```bash
cargo ci-fmt    # fmt --all -- --check
cargo ci-lint   # clippy --all-features --all-targets -- -D warnings
cargo ci-test   # test --locked --all-features
```

`--locked` is not decoration: it makes CI fail if the committed lockfile is
stale, rather than quietly resolving something else.

## Testing a measurement tool

Unit tests cover argument parsing and the chain invariants. `tests/cli.rs` runs
the built binary, because a **usage error exiting 0** is invisible in-process: a
probe that accepts nonsense and prints a number is worse than one that refuses,
because the number looks like a measurement.

**The optimiser deleting the chase** is the other failure mode, and it needs
care, because two intuitive things about it are false:

1. `cargo test` cannot catch it. `CARGO_BIN_EXE_tachyon` points at the binary
   built in the test's own profile, so the integration tests run the *debug*
   build, where nothing is elided. Only the CI `probe` job builds `--release`.
2. The symptom is not `accesses: 0`. `accesses` counts completed batches, not
   completed loads, so eliding the inner chase makes the outer loop nearly free
   and the count *inflates* — measured at ~2.4e12 for a 0.4 s run. An
   `accesses > 0` assertion cannot detect it. The **plausibility bounds on
   `ns_per_access`** are what detect it, in `tests/cli.rs` and again in CI.

If you add a flag, add a case to `a_usage_error_exits_nonzero_and_explains_itself`
for its invalid values; `usage_documents_every_flag_the_parser_accepts` and
`help_exits_zero_and_documents_every_flag` will also need it, and they are what
stop a flag shipping undocumented. If you change the chain, the cycle-property
tests in `src/lib.rs` are the ones that must still hold. If you change a JSON
field name, `json_field_names_are_pinned` fails on purpose — that rename breaks
every stored reading downstream.

**What is deliberately not tested:** absolute scores. They depend on the host, so
asserting a number would make the suite fail on a busy CI runner — which is the
tool working, not a bug. Tests assert *shape* and *plausible range* only; the
range check in CI (`20 < ns_per_access < 2000`) is wide on purpose.

## Why there are no dependencies

The probe gets baked into benchmark images and its readings are compared across
months. Every dependency is one more thing that can change what it measures
between two releases without anyone noticing. A dependency PR needs to argue that
the benefit outweighs that.

This is why the CLI is hand-rolled rather than using `clap`, and why the JSON is
`format!`ed rather than using `serde_json`. Both are small enough to own.

## Style

- `rustfmt` with the committed `rustfmt.toml`; clippy pedantic, warnings denied.
- `forbid(unsafe_code)` at crate level. A probe that could have undefined
  behaviour cannot be a trustworthy instrument.
- Comments explain *why*, not *what*. In this crate the "why" is usually "because
  otherwise the probe would measure the wrong thing" — say which thing.

## Reporting a low score

Not a bug by default. A contended machine reads slower **by design**; that is the
entire purpose. The bug report template asks what else was running, because the
answer usually explains the reading.
