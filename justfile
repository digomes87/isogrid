# Run `just --list` to see every recipe.

default: check

# Everything CI runs, in the order CI runs it.
check: fmt-check clippy test doc

# Format the workspace.
fmt:
    cargo fmt --all

# Fail if anything is unformatted.
fmt-check:
    cargo fmt --all --check

# Lint with warnings denied.
clippy:
    cargo clippy --all-targets --all-features -- -D warnings

# Run unit, integration and doc tests.
test:
    cargo test --all-features --workspace

# Build the documentation, denying broken links.
doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features

# Open the documentation in a browser.
doc-open:
    cargo doc --no-deps --all-features --open

# Coverage summary in the terminal.
cov:
    cargo llvm-cov --all-features --workspace

# Coverage as an HTML report.
cov-html:
    cargo llvm-cov --all-features --workspace --html --open

# Run the benchmarks.
bench:
    cargo bench --all-features

# Advisory and license audit.
audit:
    cargo deny check

# Build and serve the mdBook guide.
book:
    mdbook serve book --open
