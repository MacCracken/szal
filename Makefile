.PHONY: check fmt clippy test audit deny vet build doc bench clean

# Run all CI checks locally
check: fmt clippy test audit

# Format check
fmt:
	cargo fmt --all -- --check

# Lint (zero warnings)
clippy:
	cargo clippy --all-targets -- -D warnings

# Run test suite
test:
	cargo test

# Security audit
audit:
	cargo audit

# Supply-chain license/advisory checks
deny:
	cargo deny check

# Supply-chain verification
vet:
	cargo vet --locked

# Build release
build:
	cargo build --release

# Generate documentation
doc:
	cargo doc --no-deps

# Run benchmarks
bench:
	cargo bench

# Clean build artifacts
clean:
	cargo clean
