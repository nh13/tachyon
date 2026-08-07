# Changelog

All notable changes to this project are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Because this tool's readings are compared across months and across machines, any
change that alters **what is measured** is a breaking change to the numbers even
when it is not a breaking change to the API. Such changes are called out
explicitly under a `Measurement` heading.

## [Unreleased]

Initial release, not yet tagged.

### Added

- Multi-threaded dependent-load pointer chase over a per-thread working set,
  reporting `million_accesses_per_sec` (the score) and `ns_per_access` (the
  sanity check against DRAM latency).
- `--seconds`, `--working-set-mb`, `--threads`, `--seed`, `--json`, `--version`,
  `--help`.
- Flat single-line JSON output, shaped to merge into a benchmark run's metadata
  beside the host identity. It carries a `version` field, because a change to
  what the tool measures makes old readings incomparable with new ones and
  nothing else in the record distinguishes them.
- Library API (`ProbeConfig`, `ProbeResult`, `run`, `slots_for`) for embedding
  the probe.
- A CI job that builds against the MSRV declared in `Cargo.toml`, so the claim
  cannot rot unnoticed.

### Measurement

- Chain built with Sattolo's algorithm, guaranteeing a single full-length cycle.
  A multi-cycle permutation would let a walker orbit a small subset that then
  lives in cache, turning a memory probe into a cache probe.
- A dense `u32` chain covering the whole working set — sixteen slots per 64-byte
  line. Spreading hops one-per-line is unnecessary: the full-length cycle means
  the other slots in a line are revisited millions of hops later, long after the
  line has been evicted.
- Default working set 64 MB per thread, chosen to clear the last-level cache of
  every mainstream server CPU with headroom.
- `working_set_bytes` on `ProbeResult` reports the memory actually walked, which
  is the requested size rounded down to whole cache lines.

### Fixed

Pre-release, so these never shipped in a reading; recorded because each one
could have produced a number that looked like a measurement.

- `--seconds` values too large for `Duration` panicked instead of being rejected
  as a usage error. Now bounded at 86400.
- `--working-set-mb` overflowed the MB-to-bytes multiply silently in the release
  profile, wrapping to a tiny cache-resident chain and reporting thousands of
  M accesses/s at exit 0. Now uses checked arithmetic.
- A panicking worker thread was counted as zero accesses, depressing the score
  by exactly `1/threads` — indistinguishable from the host contention this tool
  exists to report. The panic now propagates.
- Documentation across `README.md`, `CONTRIBUTING.md`, `tests/cli.rs` and CI
  claimed an optimised-away chase would show `accesses: 0`, and that
  `cargo test` would catch it. Both are false: `accesses` counts batches so it
  *inflates* (~2.4e12 for a 0.4 s run), and `cargo test` links the debug binary.
  The plausibility bounds on `ns_per_access` are the assertion that catches it.

[Unreleased]: https://github.com/nh13/tachyon/commits/main
