# LinkedIn Reversed — project recipes
# All recipes run inside nix-shell via `nix-shell --run "just <recipe>"`

MANIFEST := "linkedin/Cargo.toml"

# Build the workspace
build:
    cargo build --manifest-path {{MANIFEST}}

# Run all tests
test:
    cargo test --manifest-path {{MANIFEST}}

# Run clippy linter (warnings are errors)
lint:
    cargo clippy --manifest-path {{MANIFEST}} -- -D warnings

# Format all code
fmt:
    cargo fmt --manifest-path {{MANIFEST}} --all

# Check formatting without modifying files
fmt-check:
    cargo fmt --manifest-path {{MANIFEST}} --all -- --check

# End-to-end gate: build, test, lint, format check
e2e: build test lint fmt-check

# Run the CLI with arguments
run *ARGS:
    cargo run --manifest-path {{MANIFEST}} --bin li -- {{ARGS}}
