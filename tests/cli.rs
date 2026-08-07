//! Integration tests against the real binary.
//!
//! The unit tests cover argument parsing and the chain invariants in-process.
//! These run the shipped executable instead, because a usage error exiting 0 is
//! invisible to an in-process test.
//!
//! What these tests do NOT cover, despite the obvious guess: the optimiser
//! deleting the chase. Cargo points `CARGO_BIN_EXE_tachyon` at the binary built
//! in the test's own profile, so `cargo test` runs the *debug* build, where no
//! elision happens at all. Only the CI `probe` job builds `--release`.
//!
//! And the symptom is the opposite of the intuitive one. `accesses` counts
//! completed batches (`accesses += BATCH_STEPS`), not completed loads, so an
//! elided chase makes the outer loop nearly free and `accesses` *inflates* —
//! measured at ~2.4e12 for a 0.4 s run. An `accesses > 0` assertion therefore
//! cannot detect elision. The plausibility of `ns_per_access` is what detects
//! it, which is why the bounds below are asserted on both sides.

use std::process::{Command, Output};

/// Run the built binary. Cargo sets `CARGO_BIN_EXE_<name>` for integration
/// tests, so this exercises the same artifact a user would install.
fn tachyon(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tachyon")).args(args).output().expect("failed to run tachyon")
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Pull one numeric field out of the flat JSON, without a JSON dependency.
fn json_number(json: &str, key: &str) -> f64 {
    let needle = format!("\"{key}\":");
    let start = json.find(&needle).unwrap_or_else(|| panic!("no {key} in {json}")) + needle.len();
    let rest = &json[start..];
    let end = rest.find([',', '}']).unwrap_or(rest.len());
    rest[..end].trim().parse().unwrap_or_else(|_| panic!("{key} not numeric in {json}"))
}

/// A probe short enough to keep the suite fast, with a working set small enough
/// not to thrash a CI runner but still far past any last-level cache.
fn fast<'a>(extra: &[&'a str]) -> Vec<&'a str> {
    let mut args = vec!["--seconds", "0.4", "--working-set-mb", "16"];
    args.extend_from_slice(extra);
    args
}

#[test]
fn a_short_probe_actually_measures_something() {
    let output = tachyon(&fast(&["--json"]));
    assert!(output.status.success(), "exited {:?}", output.status.code());
    let json = stdout_of(&output);

    assert!(json_number(&json, "accesses") > 0.0, "no accesses recorded: {json}");
    assert!(json_number(&json, "million_accesses_per_sec") > 0.0, "{json}");

    // The load-bearing assertion, and it is this one rather than `accesses > 0`
    // -- see the module docs. A chase that did not really touch memory reports a
    // latency far below any DRAM round trip; the bound is deliberately loose
    // because a real reading is host-dependent and a busy runner is allowed to
    // be slow. Asserting `> 0.0` alone would rest on `{:.2}` rounding a
    // sub-nanosecond value to exactly "0.00", which a formatting change would
    // silently disarm.
    let ns = json_number(&json, "ns_per_access");
    assert!(ns > 1.0, "latency implausibly low, did the chase get elided? {json}");
    assert!(ns < 10_000.0, "latency implausibly high: {json}");
}

#[test]
fn json_is_a_single_flat_object() {
    // Shaped to be merged into a run's metadata beside the host identity, so it
    // must stay one line with no nesting.
    let output = tachyon(&fast(&["--json"]));
    let json = stdout_of(&output);
    let trimmed = json.trim();
    assert!(trimmed.starts_with('{') && trimmed.ends_with('}'), "{json}");
    assert_eq!(trimmed.lines().count(), 1, "JSON must be one line: {json}");
    // Unnested, not merely one line: a second `{` would be a nested value, and
    // a flat merge into meta.json cannot carry one.
    assert_eq!(trimmed.matches('{').count(), 1, "JSON must not nest: {json}");
    assert!(trimmed.contains("\"probe\":\"memory-chase\""), "{json}");
}

#[test]
fn json_carries_the_version_that_produced_the_reading() {
    // A stored score is interpreted months later against whatever the tool
    // measured at the time; without the version there is no way to know.
    let output = tachyon(&fast(&["--json"]));
    let json = stdout_of(&output);
    let expected = format!("\"version\":\"{}\"", env!("CARGO_PKG_VERSION"));
    assert!(json.contains(&expected), "{json}");
}

#[test]
fn version_exits_zero_and_prints_the_crate_version() {
    for flag in ["--version", "-V"] {
        let output = tachyon(&[flag]);
        assert!(output.status.success(), "{flag} should exit 0");
        let text = stdout_of(&output);
        assert!(text.contains(env!("CARGO_PKG_VERSION")), "{flag} printed {text}");
    }
}

#[test]
fn human_output_is_the_default() {
    let output = tachyon(&fast(&[]));
    assert!(output.status.success());
    let text = stdout_of(&output);
    assert!(text.contains("M accesses/s"), "{text}");
    assert!(text.contains("ns/access"), "{text}");
    assert!(!text.contains('{'), "default output should not be JSON: {text}");
}

#[test]
fn requested_thread_count_is_honoured() {
    let output = tachyon(&["--seconds", "0.4", "--working-set-mb", "8", "-t", "2", "--json"]);
    let json = stdout_of(&output);
    assert!((json_number(&json, "threads") - 2.0).abs() < f64::EPSILON, "{json}");
}

#[test]
fn help_exits_zero_and_documents_every_flag() {
    for flag in ["--help", "-h"] {
        let output = tachyon(&[flag]);
        assert!(output.status.success(), "{flag} should exit 0");
        let text = stdout_of(&output);
        for expected in
            ["--seconds", "--working-set-mb", "--threads", "--seed", "--json", "--version"]
        {
            assert!(text.contains(expected), "{flag} output omits {expected}: {text}");
        }
    }
}

#[test]
fn a_usage_error_exits_nonzero_and_explains_itself() {
    // A probe that silently accepts nonsense and reports a number is worse than
    // one that refuses: the number would look like a measurement.
    for args in [
        vec!["--nope"],
        vec!["--seconds"],
        vec!["--seconds", "0"],
        vec!["--seconds", "later"],
        // Large but finite: this used to reach Duration::from_secs_f64 and
        // abort with a panic, which is neither an error line nor a usage text.
        vec!["--seconds", "1e20"],
        vec!["--threads", "0"],
        vec!["--working-set-mb", "0"],
        // Overflows the MB-to-bytes multiply, which wrapped silently in release
        // and reported a cache-resident chain as a very fast host.
        vec!["--working-set-mb", "17592186044416"],
        vec!["--seed", "lucky"],
    ] {
        let output = tachyon(&args);
        assert!(!output.status.success(), "{args:?} should have failed");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("error:"), "{args:?} gave no error line: {stderr}");
        // The usage text follows the error, so the fix is on screen.
        assert!(stderr.contains("USAGE:"), "{args:?} printed no usage: {stderr}");
    }
}

#[test]
fn no_seed_produces_a_degenerate_chain() {
    // Every seed must yield a chain the chase can walk -- including 0, which
    // `XorShift64::new` substitutes a constant for because zero is a fixed point
    // of xorshift. That the same seed reproduces the same chain is pinned by
    // `same_seed_builds_the_same_chain` in src/lib.rs; this covers the other
    // half, that no seed collapses the probe.
    for seed in ["1", "0", "12345"] {
        let output =
            tachyon(&["--seconds", "0.3", "--working-set-mb", "8", "--seed", seed, "--json"]);
        assert!(output.status.success(), "seed {seed} failed");
        let json = stdout_of(&output);
        assert!(json_number(&json, "accesses") > 0.0, "seed {seed}: {json}");
    }
}
