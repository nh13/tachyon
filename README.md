[![Check and Test](https://github.com/nh13/tachyon/actions/workflows/check.yml/badge.svg)](https://github.com/nh13/tachyon/actions/workflows/check.yml)
[![Version at crates.io](https://img.shields.io/crates/v/tachyon)](https://crates.io/crates/tachyon)
[![Documentation at docs.rs](https://img.shields.io/docsrs/tachyon)](https://docs.rs/tachyon)
[![License](http://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/nh13/tachyon/blob/main/LICENSE)

# tachyon

**Detect the cloaked.** Measure how fast *this* host's memory subsystem is **right now**, so you can tell a slow machine apart from slow code.

> "A tachyon detection grid. Any cloaked ship passing through it would disrupt the beam."
> — Data, *Redemption II*

On a shared cloud host your noisy neighbours are cloaked: you cannot see them, you cannot name them, and they are stealing your throughput. This is the detection grid.

Zero dependencies. One binary. `forbid(unsafe_code)`.

## The problem

Benchmark wall times on shared cloud hosts are not comparable across runs.

Measured on a real AWS Batch fleet — `m7i.4xlarge`, one binary, one input, 644 measurements across 156 distinct instances:

| | spread |
| --- | --- |
| repeat a measurement on the **same** instance | **2.0 %** |
| let each measurement pick a **fresh** instance | **9.4 %** |

The same unchanged binary measured **18.89 s** and **25.01 s** depending only on which machine it landed on. That was enough to fail a release gate for a regression that did not exist.

**It is not lost CPU.** Across every replicate `cpu_time / wall` held at 14.1–14.7 of 16 vCPUs — the scheduler handed over the cores. What changed is that identical work burned about **20 % more CPU-seconds**: cores stalling on memory, because co-tenants on the same socket were consuming last-level cache and DRAM bandwidth.

A `4xlarge` is roughly one twelfth of a physical server. Your vCPUs are guaranteed. Your share of the memory system is not, and never was.

## Highlights

- **Sensitive to the right thing.** A dependent-load pointer chase over a working set far past last-level cache — the access pattern of an FM-index search, a hash join, a graph traversal. Not an arithmetic loop, which reads the same on a contended host as on an idle one.
- **Responds, and recovers.** 42 % drop with a memory neighbour present, back to baseline when it leaves. Measures conditions *now*, not a running average.
- **Independent of your code by construction.** Shares nothing with whatever you are benchmarking, so a real regression in your code cannot move the probe. That is what makes it usable as a control.
- **Zero dependencies.** Readings get compared across months; nothing outside this repo can change what they mean.
- **Machine-readable.** `--json` emits one flat line, shaped to merge into a run's metadata beside the host identity.

## Installation

```console
# From crates.io:
cargo install tachyon

# From source:
git clone https://github.com/nh13/tachyon && cd tachyon
cargo build --release
# Binary is at target/release/tachyon
```

Record `tachyon --version` alongside any score you keep. A future release may change what the probe measures, and the version is the only thing that tells two readings apart.

## Usage

```console
$ tachyon
score  92.424 M accesses/s   (higher = faster host right now)
latency 129.84 ns/access      (expect ~60-120 ns uncontended DRAM)
12 threads x 64 MB chain, 924467200 accesses in 10.00 s

$ tachyon --seconds 2 --json
{"probe":"memory-chase","version":"0.1.0","million_accesses_per_sec":82.522,"ns_per_access":145.42,"accesses":165445632,"elapsed_s":2.005,"threads":12,"working_set_bytes_per_thread":67108864}
```

| flag | default | meaning |
| --- | --- | --- |
| `-s, --seconds <F>` | 10 | wall-clock budget (max 86400) |
| `-w, --working-set-mb <N>` | 64 | chain size **per thread** |
| `-t, --threads <N>` | available parallelism | concurrent chains |
| `--seed <N>` | 1 | chain permutation seed |
| `--json` | off | flat JSON instead of a human summary |
| `-V, --version` | | version, recorded in `--json` so stored scores stay interpretable |
| `-h, --help` | | usage |

### Reading the output

**`million_accesses_per_sec` is the score.** Higher means a faster host right now. This is the number to compare between runs, or to record alongside a timing.

**`ns_per_access` is the sanity check.** An uncontended DRAM round trip is roughly 60–120 ns. A reading far below that means the working set fit in cache and the probe measured the wrong thing — raise `--working-set-mb`. It is also the field that catches a probe which did not really touch memory at all: `accesses` counts completed *batches*, so it goes **up**, not to zero, if the chase is ever optimised away.

**`version` says which semantics produced the number.** A change to what the tool measures makes old readings incomparable with new ones, and nothing else in the record distinguishes them — so keep it with the score.

### Choosing a working set

The default 64 MB per thread clears the last-level cache of every mainstream server CPU with headroom. Raise it for a host with a very large L3. Lower it only after checking `ns_per_access` still looks like DRAM.

## Does it work?

A 4-thread probe on an otherwise quiet 12-core host:

| condition | score | latency |
| --- | --- | --- |
| alone | 28.4 / 30.3 / 31.7 M/s | 126–141 ns |
| **8-thread memory neighbour** | **18.0 / 17.2 / 17.9 M/s** | **222–233 ns** |
| alone again | 29.8 / 29.7 M/s | 134 ns |

A **42 % response**, tight within each condition, and it **recovers** — so the score tracks present conditions rather than drifting. The response is larger than the ~20 % wall-time effect it exists to detect, which is the right way round for a detector.

Single-threaded latency reads 87.8 ns on the same host, squarely in the DRAM range: the check that the working set really is clearing cache.

## Design

Each choice exists to keep the probe honest about one thing.

- **Pointer chase, not arithmetic.** Every step's address comes from the previous step's *value*, so nothing prefetches and nothing overlaps within a thread. A register-bound arithmetic loop is immune to a memory neighbour and would report an idle host and a contended one identically.
- **One chain per thread.** A single-threaded chase measures latency; a real multi-threaded workload also consumes memory *parallelism*, and that is what a neighbour degrades. Match `--threads` to the workload you care about.
- **Sattolo's algorithm, not Fisher-Yates.** Sattolo draws `j` from `[0, i)` rather than `[0, i]`, which guarantees a **single cycle of full length**. An arbitrary permutation decomposes into several short cycles, and a walker caught in a short one would revisit a handful of lines that then sit in cache — quietly turning a memory probe into a cache probe. Pinned by a test.
- **A dense `u32` chain covering the whole working set.** Sixteen slots share each 64-byte line, so the array is exactly the size you asked for. Spreading hops one-per-line is unnecessary: the full-length cycle means the neighbours of any line are revisited millions of hops later, long after eviction.
- **No shared code with the thing being measured.** If the probe shared code with your workload, dividing your measurement by the probe score would cancel real regressions along with host noise. `cpu_time` is the cautionary example: it is already a contention signal, but a genuinely slower binary also burns more of it, so normalising by it flattens everything — including what you were looking for.

## Two caveats

**It is not a pure memory probe.** Eight CPU-only spinning threads with no memory traffic still cost about 18 %, so the score conflates memory contention with CPU availability. For deciding *"is this host degraded"* that is harmless — either cause means it is. It matters if you use the score as a divisor.

**It needs a genuinely quiet baseline.** An early measurement here read 12–22 M/s instead of ~30 purely because a build was finishing in the background, which *inverted* the result until the machine was actually idle. Calibrate on a host you know is idle, and record enough context to tell later.

## Filter first, normalise only once validated

Treating the score as a **scale factor** assumes probe and workload degrade proportionally. Plausible; unverified.

Treating it as a **filter** only assumes a bad score means a bad host. Far weaker, and already useful.

So the recommended path is:

1. Record the score next to the host identity and the timing, for a while.
2. Check how much of the between-host variance the score actually explains.
3. If most of it, normalise. If not, you still have a principled way to flag or discard measurements from bad hosts — needing no calibration at all.

Step 2 is the one people skip. It is also the one that decides whether the number means anything.

## Use as a library

```rust
use std::time::Duration;
use tachyon::{ProbeConfig, run};

let result = run(&ProbeConfig {
    duration: Duration::from_secs(2),
    working_set_bytes: 64 << 20,
    threads: 4,
    seed: 1,
});
println!("{:.3} M accesses/s", result.million_accesses_per_sec());
```

`run` does not police `working_set_bytes` the way the CLI does — a working set that fits in cache yields a well-formed result reporting a very fast host. Check `ns_per_access()` against DRAM latency to confirm you sized it correctly.

## Motivation

This came out of a false alarm. A release benchmark reported a 13 % performance regression on one cell, with tight non-overlapping ranges across eight replicates — about as convincing as such evidence gets. Re-running the *previous, unchanged* binary on the same day measured it 19 % slower than its own recorded number, which meant the release was actually **faster**, and the gate had failed a good release.

No amount of extra replication fixes that, because the machine and the release changed together: a significance test can establish that a difference is real, never what caused it. The options are to control the machine — run both versions on one host, alternating — or to measure the machine and account for it.

This is the second option, and it is much cheaper than the first.

## License

MIT
