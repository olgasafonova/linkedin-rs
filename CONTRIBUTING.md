# Contributing to linkedin-rs

Thanks for contributing. This is a private, solo-maintained crate that drives a **live LinkedIn account**, so the bar for merging is "demonstrably correct and safe," not "looks done." This guide exists so that expectation is visible up front — meet it and review is fast; skip it and the PR bounces.

The core principle: **the burden of proving a change is correct sits with the author, not the reviewer.** Review is for judgment (is this the right approach, does it fit the codebase). It is not for catching things CI catches.

## Before you open a PR — definition of done

A PR is ready for review when **all** of these are true. The CI enforces the first four; do them locally first so you don't round-trip through a red build.

The workspace lives in `./linkedin/`, so run cargo from there:

```bash
cd linkedin
cargo fmt --all                       # not just --check; produce the canonical form
cargo clippy --all-targets -- -D warnings
cargo build --all-targets             # CI runs with RUSTFLAGS="-Dwarnings"
cargo test
```

- [ ] **`cargo fmt` clean** — CI runs `cargo fmt --check` and fails on any drift.
- [ ] **`cargo clippy --all-targets` clean under `-D warnings`** — this includes `dead_code` and `unused_variables`. A helper with no caller yet, or a variable you write but never read, **fails the build**. If you're adding code a later PR will call, annotate it `#[allow(dead_code)] // wired up in <follow-up>` and remove the allow when the caller lands.
- [ ] **`cargo build --all-targets` clean** — same `-D warnings` rule applies to tests and examples.
- [ ] **`cargo test` green.**
- [ ] **New code paths have tests** — see "Testing expectations" below.
- [ ] **No new `.unwrap()`, `.expect()`, `panic!`, `unreachable!`, or unchecked indexing/slicing in non-test code.** A failed API call must return `Err`, not panic. (Test code may use `.unwrap()` freely.)
- [ ] **The PR description says what you verified** — see the template below.

## Testing expectations

Tests are not optional decoration; they're the proof. Two areas are non-negotiable because they're where bugs hide:

1. **Write / mutation paths.** Any method that posts, replies, schedules, deletes, or otherwise changes LinkedIn state needs a test asserting it builds the **exact** request body / GraphQL variables. These APIs are shape-sensitive — the server rejects a malformed mutation, and you can't catch that without a test on the body builder. Factor the body construction into a pure function (e.g. `fn build_x_variables(...) -> Value`) so it's testable without a live session.
2. **Input parsing.** Anything that parses user input or server responses (byte ranges, cursors, datetimes, URNs) needs tests for the **bad** cases, not just the happy path: malformed input, empty input, out-of-range, multi-byte UTF-8, missing JSON fields. "It works on my example" is not coverage.

If the logic needs a live session to run, extract the pure part and test that. Don't leave a parser untested because it's buried in an `async fn`.

## Input safety — the recurring trap

Two failure modes show up repeatedly; please self-check for both before requesting review:

- **UTF-8 byte slicing.** Never slice a `&str` by byte index (`&s[..80]`). LinkedIn text routinely contains emoji, accents, and CJK, so a byte index lands mid-character and **panics**. Use `s.chars().take(80).collect::<String>()`.
- **Untrusted range/index math.** Validate any `start`/`end`/length derived from a server response before indexing (`start <= end`, `start < len`). Server-controlled values reaching `slice[a..=b]` are a panic waiting to happen.

## Pull request size and stacking

- **Keep PRs reviewable in one sitting.** A focused 300-line PR gets reviewed today; a 2,000-line PR waits. If a change is large, split it into a stack of small PRs, each independently correct, and say in each description what it builds on.
- **Each PR in a stack must compile and pass CI on its own** — no "this is broken until the next PR lands."
- **One concern per PR.** Don't bundle a refactor with a feature; they're hard to review together and risky to revert separately.

## Dependencies

This repo touches a live account, so the dependency surface is security-relevant.

- **Justify any new crate** in the PR description: what it's for, why an existing dep won't do.
- **Pin it** via `Cargo.lock` (commit the lockfile change) and prefer well-established crates over niche ones.
- **No git or path dependencies**, no unexpected version jumps, no anything that looks like a typosquat.
- Flag any new transitive dependencies a crate pulls in if they're surprising.

## Security and account safety

- **Never log, print, or embed credentials, cookies, CSRF tokens, or session identifiers** in URLs, errors, or debug output.
- **No `unsafe`** without a written justification in the PR and a `// SAFETY:` comment.
- **No new network egress to non-LinkedIn hosts**, no telemetry, no phone-home. The media-upload client is intentionally credential-free — keep it that way.
- Respect LinkedIn's rate limits and terms; changes that increase request volume or automation footprint need a note explaining the risk.

## PR description template

Copy this into your PR body:

```markdown
## What this does
<one or two sentences>

## Builds on / stacked under
<links to prerequisite PRs, or "none">

## What I verified
- [ ] `cargo fmt --check` clean
- [ ] `cargo clippy --all-targets -- -D warnings` clean
- [ ] `cargo test` green (N tests)
- [ ] New paths covered by tests: <which>
- [ ] Manually ran: <commands + brief result, or "n/a — pure logic, covered by tests">

## Notes for the reviewer
<anything non-obvious: a deliberate trade-off, a contract change, a follow-up you're deferring>
```

## How review works here

Review verdicts and what they mean for you:

- **Approve** / **Approve with nits** — merge at your discretion; any comments are optional.
- **Request changes** — there's a blocking item; fix it and re-request review.

Reviewer comments are tagged by priority so you know what's required:

- **Must-fix** — blocks merge.
- **Should-fix** — fix in this PR; it's a real quality issue, not a blocker on its own.
- **Could-defer** — fine as a follow-up; open an issue and link it.

If a comment is unclear, ask — a question is cheaper than a wrong guess. Thanks for holding the bar; it's what makes contributions here mergeable instead of a back-and-forth.
