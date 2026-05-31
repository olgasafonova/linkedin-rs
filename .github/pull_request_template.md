## What this does
<one or two sentences>

## Builds on / stacked under
<links to prerequisite PRs, or "none">

## What I verified
<!-- Run from ./linkedin/ — CI uses RUSTFLAGS="-Dwarnings" -->
- [ ] `cargo fmt --check` clean
- [ ] `cargo clippy --all-targets -- -D warnings` clean
- [ ] `cargo build --all-targets` clean
- [ ] `cargo test` green (N tests)
- [ ] New paths covered by tests: <which, or "n/a">
- [ ] Manually ran: <commands + brief result, or "n/a — pure logic, covered by tests">

## Notes for the reviewer
<anything non-obvious: a deliberate trade-off, a contract change, a follow-up you're deferring>

<!--
Before requesting review, confirm:
- No new .unwrap()/.expect()/panic!/unreachable!/unchecked indexing in non-test code
- No &str byte-slicing (use .chars().take(n)); validate server-derived ranges before indexing
- Any new dependency is justified above, pinned in Cargo.lock, and reputable
- No credentials/cookies/tokens logged or in URLs; no new non-LinkedIn egress
See CONTRIBUTING.md for the full bar.
-->
