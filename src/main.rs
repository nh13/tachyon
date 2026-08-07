//! CLI for the contention probe. See `lib.rs` for what it measures and why.

use std::process::ExitCode;
use std::time::Duration;

use tachyon::{ProbeConfig, ProbeResult, run};

const DEFAULT_SECONDS: f64 = 10.0;
const DEFAULT_WORKING_SET_MB: usize = 64;
const BYTES_PER_MB: usize = 1024 * 1024;

/// Upper bound on `--seconds`. `Duration::from_secs_f64` panics above roughly
/// `u64::MAX` seconds, and a probe asked to run longer than a day is a typo
/// rather than a request — so reject it as a usage error instead of aborting
/// with a panic the operator cannot act on.
const MAX_SECONDS: f64 = 86_400.0;

const USAGE: &str = "\
tachyon — measure this host's memory access rate under current contention

USAGE:
    tachyon [OPTIONS]

OPTIONS:
    -s, --seconds <F>            wall-clock budget (default 10, max 86400)
    -w, --working-set-mb <N>     chain size PER THREAD (default 64)
    -t, --threads <N>            concurrent chains (default: available parallelism)
        --seed <N>               chain permutation seed (default 1)
        --json                   emit JSON instead of a human summary
    -V, --version                print the version and exit
    -h, --help                   print this help

WHY 64 MB PER THREAD BY DEFAULT
    The chain must not fit in last-level cache, or the probe measures cache
    instead of memory. 64 MB per thread clears the LLC of every instance type
    this project benchmarks, with headroom. Raise it if you add a host with a
    very large L3; lower it only if you have checked ns_per_access still looks
    like DRAM (roughly 60-120 ns uncontended).

INTERPRETING THE OUTPUT
    million_accesses_per_sec is the score: higher means a faster host RIGHT NOW.
    Compare it between runs, and record it next to the instance id so host
    quality can be separated from code change after the fact.

    Use it as a FILTER before using it as a divisor. Treating it as a scale
    factor assumes probe and workload degrade proportionally, which is plausible
    but unverified; treating it as a filter only assumes a bad score means a bad
    host.

    Record the version alongside the score. A change to what this tool measures
    makes old readings incomparable with new ones, and the version is the only
    thing in the output that says which semantics produced a number.
";

/// Parsed CLI arguments, or a request to print help or the version.
#[derive(Debug)]
enum Args {
    Help,
    Version,
    Run { config: ProbeConfig, json: bool },
}

fn parse_args<I: Iterator<Item = String>>(args: I) -> Result<Args, String> {
    let mut seconds = DEFAULT_SECONDS;
    let mut working_set_mb = DEFAULT_WORKING_SET_MB;
    let mut threads = std::thread::available_parallelism().map_or(1, Into::into);
    let mut seed = 1u64;
    let mut json = false;

    let mut args = args;
    while let Some(arg) = args.next() {
        // Every value-taking flag needs the same "was a value actually there?"
        // check; without it `--threads` at the end of argv silently keeps the
        // default and the operator gets a probe they did not ask for.
        let mut value = || -> Result<String, String> {
            args.next().ok_or_else(|| format!("{arg} requires a value"))
        };
        match arg.as_str() {
            "-h" | "--help" => return Ok(Args::Help),
            "-V" | "--version" => return Ok(Args::Version),
            "--json" => json = true,
            "-s" | "--seconds" => {
                let raw = value()?;
                seconds = raw.parse().map_err(|_| format!("--seconds: not a number: {raw}"))?;
                // The upper bound is not decoration: Duration::from_secs_f64
                // panics on values it cannot represent, and a panic gives the
                // operator a backtrace where they need a usage message.
                if !(seconds.is_finite() && seconds > 0.0 && seconds <= MAX_SECONDS) {
                    return Err(format!(
                        "--seconds must be positive and at most {MAX_SECONDS}, got {seconds}"
                    ));
                }
            }
            "-w" | "--working-set-mb" => {
                let raw = value()?;
                working_set_mb =
                    raw.parse().map_err(|_| format!("--working-set-mb: not an integer: {raw}"))?;
                if working_set_mb == 0 {
                    return Err("--working-set-mb must be at least 1".to_string());
                }
            }
            "-t" | "--threads" => {
                let raw = value()?;
                threads = raw.parse().map_err(|_| format!("--threads: not an integer: {raw}"))?;
                if threads == 0 {
                    return Err("--threads must be at least 1".to_string());
                }
            }
            "--seed" => {
                let raw = value()?;
                seed = raw.parse().map_err(|_| format!("--seed: not an integer: {raw}"))?;
            }
            other => return Err(format!("unrecognised argument: {other}")),
        }
    }

    // Unchecked, this wraps in the release profile (where overflow checks are
    // off) and the probe goes on to measure a tiny cache-resident chain and
    // report it as an absurdly fast host — a fabricated number that looks like
    // a measurement, which is worse than refusing.
    let working_set_bytes = working_set_mb
        .checked_mul(BYTES_PER_MB)
        .ok_or_else(|| format!("--working-set-mb is too large: {working_set_mb}"))?;

    Ok(Args::Run {
        config: ProbeConfig {
            duration: Duration::from_secs_f64(seconds),
            working_set_bytes,
            threads,
            seed,
        },
        json,
    })
}

/// JSON shaped to be merged straight into a run's `meta.json`, so a probe score
/// lands beside the `instance_id` it belongs to.
///
/// `version` is carried in the record because a change to what this tool
/// measures silently invalidates comparison with every earlier reading, and a
/// stored score with no version cannot be told apart from one taken under
/// different semantics.
fn as_json(result: &ProbeResult) -> String {
    format!(
        concat!(
            "{{\"probe\":\"memory-chase\",",
            "\"version\":\"{}\",",
            "\"million_accesses_per_sec\":{:.3},",
            "\"ns_per_access\":{:.2},",
            "\"accesses\":{},",
            "\"elapsed_s\":{:.3},",
            "\"threads\":{},",
            "\"working_set_bytes_per_thread\":{}}}"
        ),
        env!("CARGO_PKG_VERSION"),
        result.million_accesses_per_sec(),
        result.ns_per_access(),
        result.accesses,
        result.elapsed.as_secs_f64(),
        result.threads,
        result.working_set_bytes,
    )
}

fn as_human(result: &ProbeResult) -> String {
    format!(
        "score  {:.3} M accesses/s   (higher = faster host right now)\n\
         latency {:.2} ns/access      (expect ~60-120 ns uncontended DRAM)\n\
         {} threads x {} MB chain, {} accesses in {:.2} s",
        result.million_accesses_per_sec(),
        result.ns_per_access(),
        result.threads,
        result.working_set_bytes / BYTES_PER_MB,
        result.accesses,
        result.elapsed.as_secs_f64(),
    )
}

fn main() -> ExitCode {
    match parse_args(std::env::args().skip(1)) {
        Ok(Args::Help) => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        Ok(Args::Version) => {
            println!(concat!("tachyon ", env!("CARGO_PKG_VERSION")));
            ExitCode::SUCCESS
        }
        Ok(Args::Run { config, json }) => {
            let result = run(&config);
            if json {
                println!("{}", as_json(&result));
            } else {
                println!("{}", as_human(&result));
            }
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("error: {message}\n");
            eprint!("{USAGE}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Args, String> {
        parse_args(args.iter().map(|s| (*s).to_string()))
    }

    fn config_of(args: &[&str]) -> ProbeConfig {
        match parse(args) {
            Ok(Args::Run { config, .. }) => config,
            other => panic!("expected a run, got {other:?}"),
        }
    }

    #[test]
    fn defaults_are_a_usable_probe() {
        let config = config_of(&[]);
        assert!(config.duration.as_secs_f64() > 0.0);
        assert_eq!(config.working_set_bytes, DEFAULT_WORKING_SET_MB * BYTES_PER_MB);
        assert!(config.threads >= 1);
    }

    #[test]
    fn flags_override_every_default() {
        let config = config_of(&["--seconds", "0.5", "-w", "8", "-t", "3", "--seed", "77"]);
        assert!((config.duration.as_secs_f64() - 0.5).abs() < 1e-9);
        assert_eq!(config.working_set_bytes, 8 * BYTES_PER_MB);
        assert_eq!(config.threads, 3);
        assert_eq!(config.seed, 77);
    }

    #[test]
    fn a_flag_missing_its_value_is_an_error() {
        // Not a default: a truncated command line must fail loudly rather than
        // silently probe with settings the operator did not choose.
        for args in [vec!["--seconds"], vec!["--working-set-mb"], vec!["--threads"], vec!["--seed"]]
        {
            let err = parse(&args).expect_err("expected an error");
            assert!(err.contains("requires a value"), "{err}");
        }
    }

    #[test]
    fn zero_and_negative_settings_are_rejected() {
        // Each of these would produce a probe that reports a number without
        // measuring anything: no time, no working set, no threads.
        for args in [
            vec!["--seconds", "0"],
            vec!["--seconds", "-1"],
            vec!["--working-set-mb", "0"],
            vec!["--threads", "0"],
        ] {
            assert!(parse(&args).is_err(), "{args:?} should have been rejected");
        }
    }

    #[test]
    fn out_of_range_settings_are_rejected_rather_than_panicking() {
        // Both of these used to reach arithmetic that fails silently or loudly
        // in the wrong way: `--seconds` panicked inside Duration::from_secs_f64,
        // and `--working-set-mb` wrapped in release and probed a cache-resident
        // chain that reported thousands of M accesses/s.
        let err = parse(&["--seconds", "1e20"]).expect_err("huge --seconds must be rejected");
        assert!(err.contains("--seconds"), "{err}");
        let err = parse(&["--working-set-mb", "17592186044416"])
            .expect_err("overflowing --working-set-mb must be rejected");
        assert!(err.contains("--working-set-mb"), "{err}");
    }

    #[test]
    fn a_working_set_at_the_overflow_boundary_is_still_accepted() {
        // The guard must reject only what actually overflows, not shave off
        // legitimately large working sets.
        let largest = usize::MAX / BYTES_PER_MB;
        assert_eq!(
            config_of(&["-w", &largest.to_string()]).working_set_bytes,
            largest * BYTES_PER_MB
        );
    }

    #[test]
    fn nonsense_values_are_rejected() {
        assert!(parse(&["--seconds", "soon"]).is_err());
        assert!(parse(&["--threads", "many"]).is_err());
        assert!(parse(&["--seed", "lucky"]).is_err());
        assert!(parse(&["--nope"]).is_err());
    }

    #[test]
    fn help_is_requested_not_run() {
        assert!(matches!(parse(&["--help"]), Ok(Args::Help)));
        assert!(matches!(parse(&["-h"]), Ok(Args::Help)));
    }

    #[test]
    fn version_is_requested_not_run() {
        assert!(matches!(parse(&["--version"]), Ok(Args::Version)));
        assert!(matches!(parse(&["-V"]), Ok(Args::Version)));
    }

    #[test]
    fn usage_documents_every_flag_the_parser_accepts() {
        // The help text is the only place the flag set is written down for a
        // user, so a flag added to the parser and not to USAGE is invisible.
        for flag in [
            "--seconds",
            "--working-set-mb",
            "--threads",
            "--seed",
            "--json",
            "--version",
            "--help",
        ] {
            assert!(USAGE.contains(flag), "USAGE omits {flag}");
            assert!(parse(&[flag]).is_ok() || parse(&[flag, "1"]).is_ok(), "{flag} not accepted");
        }
    }

    #[test]
    fn json_is_flat_and_carries_the_score() {
        let result = ProbeResult {
            accesses: 1_000_000,
            elapsed: Duration::from_secs(1),
            threads: 2,
            working_set_bytes: 4 * BYTES_PER_MB,
        };
        let json = as_json(&result);
        // Flat and quoted so it merges into meta.json without reshaping.
        assert!(json.starts_with('{') && json.ends_with('}'));
        assert!(json.contains("\"million_accesses_per_sec\":1.000"), "{json}");
        assert!(json.contains("\"threads\":2"), "{json}");
        assert!(json.contains("\"accesses\":1000000"), "{json}");
    }

    #[test]
    fn json_carries_the_version_that_produced_the_reading() {
        let result = ProbeResult {
            accesses: 1,
            elapsed: Duration::from_secs(1),
            threads: 1,
            working_set_bytes: BYTES_PER_MB,
        };
        let expected = format!("\"version\":\"{}\"", env!("CARGO_PKG_VERSION"));
        assert!(as_json(&result).contains(&expected), "{}", as_json(&result));
    }

    #[test]
    fn json_field_names_are_pinned() {
        // Downstream runs join stored scores on these names, so a rename is a
        // silent break of every record already written. Nothing else in the
        // suite would notice one.
        let result = ProbeResult {
            accesses: 1,
            elapsed: Duration::from_secs(1),
            threads: 1,
            working_set_bytes: BYTES_PER_MB,
        };
        let json = as_json(&result);
        for key in [
            "probe",
            "version",
            "million_accesses_per_sec",
            "ns_per_access",
            "accesses",
            "elapsed_s",
            "threads",
            "working_set_bytes_per_thread",
        ] {
            assert!(json.contains(&format!("\"{key}\":")), "JSON lost the {key} field: {json}");
        }
        // One object, no nesting: a `{` after the first character would mean a
        // nested value, which the flat-merge contract forbids.
        assert_eq!(json.matches('{').count(), 1, "JSON must not nest: {json}");
    }

    #[test]
    fn human_output_names_both_metrics() {
        let result = ProbeResult {
            accesses: 500_000,
            elapsed: Duration::from_secs(1),
            threads: 1,
            working_set_bytes: BYTES_PER_MB,
        };
        let text = as_human(&result);
        assert!(text.contains("M accesses/s"), "{text}");
        assert!(text.contains("ns/access"), "{text}");
    }
}
