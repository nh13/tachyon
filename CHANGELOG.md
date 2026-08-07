# Changelog

All notable changes to this project are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Because this tool's readings are compared across months and across machines, any
change that alters **what is measured** is a breaking change to the numbers even
when it is not a breaking change to the API. Such changes are called out
explicitly under a `Measurement` heading.

## [Unreleased]

## [0.1.0] - 2026-08-07

Initial release.

### Added

- Multi-threaded dependent-load pointer chase over a per-thread working set,
  reporting `million_accesses_per_sec` (the score) and `ns_per_access` (the
  sanity check against DRAM latency).
- `--seconds`, `--working-set-mb`, `--threads`, `--seed`, `--json`, `--help`.
- Flat single-line JSON output, shaped to merge into a benchmark run's metadata
  beside the host identity.
- Library API (`ProbeConfig`, `ProbeResult`, `run`) for embedding the probe.

### Measurement

- Chain built with Sattolo's algorithm, guaranteeing a single full-length cycle.
  A multi-cycle permutation would let a walker orbit a small subset that then
  lives in cache, turning a memory probe into a cache probe.
- One chain slot per 64-byte cache line, so consecutive hops never share a line.
- Default working set 64 MB per thread, chosen to clear the last-level cache of
  every mainstream server CPU with headroom.

[Unreleased]: https://github.com/nh13/tachyon/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/nh13/tachyon/releases/tag/v0.1.0
