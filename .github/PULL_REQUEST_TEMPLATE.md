## What changed

<!-- One or two sentences. -->

## Why

<!-- The problem this solves. Link an issue if there is one. -->

## Measurement impact

<!-- REQUIRED for any change to what determines a reading: src/, the
     [profile.release] section of Cargo.toml, or rust-toolchain.toml. This
     tool's output is compared across months and across machines, so a change
     that alters what it measures invalidates past readings.

     State one of:
       - No change to what is measured (refactor, docs, CI).
       - Changes what is measured, and here is the before/after on one host.

     If the second, include scores from the same machine before and after. -->

## Checklist

- [ ] `cargo ci-fmt`, `cargo ci-lint`, `cargo ci-test` pass locally
- [ ] New behaviour has a test; a bug fix has a regression test
- [ ] README/docs updated if a flag, default, or output field changed
- [ ] CHANGELOG.md updated under `## Unreleased`
