//! Integration tests against the real binary.
//!
//! The unit tests cover argument parsing and the chain invariants in-process.
//! These run the shipped executable instead, because two of the failure modes
//! that matter most are invisible to an in-process test: the release profile
//! optimising the chase away entirely (`accesses` would read 0 while every unit
//! test still passed), and a usage error exiting 0.

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
const FAST: [&str; 4] = ["--seconds", "0.4", "--working-set-mb", "16"];

#[test]
fn a_short_probe_actually_measures_something() {
    let output = tachyon(&[FAST[0], FAST[1], FAST[2], FAST[3], "--json"]);
    assert!(output.status.success(), "exited {:?}", output.status.code());
    let json = stdout_of(&output);

    // The load-bearing assertion. If the optimiser eliminates the pointer chase
    // -- it is pure computation whose result is otherwise unused -- this reads 0
    // while the binary still exits 0 and prints a well-formed report.
    assert!(json_number(&json, "accesses") > 0.0, "no accesses recorded: {json}");
    assert!(json_number(&json, "million_accesses_per_sec") > 0.0, "{json}");
    assert!(json_number(&json, "ns_per_access") > 0.0, "{json}");
}

#[test]
fn json_is_a_single_flat_object() {
    // Shaped to be merged into a run's metadata beside the host identity, so it
    // must stay one line with no nesting.
    let output = tachyon(&[FAST[0], FAST[1], FAST[2], FAST[3], "--json"]);
    let json = stdout_of(&output);
    let trimmed = json.trim();
    assert!(trimmed.starts_with('{') && trimmed.ends_with('}'), "{json}");
    assert_eq!(trimmed.lines().count(), 1, "JSON must be one line: {json}");
    assert!(trimmed.contains("\"probe\":\"memory-chase\""), "{json}");
}

#[test]
fn human_output_is_the_default() {
    let output = tachyon(&[FAST[0], FAST[1], FAST[2], FAST[3]]);
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
        for expected in ["--seconds", "--working-set-mb", "--threads", "--seed", "--json"] {
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
        vec!["--threads", "0"],
        vec!["--working-set-mb", "0"],
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
fn the_same_seed_is_reproducible_in_shape() {
    // Two runs cannot produce identical scores -- that is the point, they measure
    // conditions -- but both must produce a usable reading, and neither may
    // collapse to zero because the seed happened to build a degenerate chain.
    for seed in ["1", "0", "12345"] {
        let output =
            tachyon(&["--seconds", "0.3", "--working-set-mb", "8", "--seed", seed, "--json"]);
        assert!(output.status.success(), "seed {seed} failed");
        let json = stdout_of(&output);
        assert!(json_number(&json, "accesses") > 0.0, "seed {seed}: {json}");
    }
}
