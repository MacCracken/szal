.PHONY: check fmt clippy test audit deny vet build doc bench clean coverage fuzz semver msrv bench-history

# Run all CI checks locally
check: fmt clippy test audit

# Format check
fmt:
	cargo fmt --all -- --check

# Lint (zero warnings)
clippy:
	cargo clippy --all-features --all-targets -- -D warnings

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

# Coverage report
coverage:
	cargo llvm-cov --lcov --output-path lcov.info

# Fuzz targets (30s each)
fuzz:
	cargo +nightly fuzz run fuzz_step_deser -- -max_total_time=30 || true
	cargo +nightly fuzz run fuzz_flow_deser -- -max_total_time=30 || true
	cargo +nightly fuzz run fuzz_flow_validate -- -max_total_time=30 || true
	cargo +nightly fuzz run fuzz_state_transitions -- -max_total_time=30 || true

# Semver compatibility check
semver:
	cargo semver-checks check-release || true

# MSRV check (1.89)
msrv:
	cargo +1.89 check
	cargo +1.89 test

# Run benchmarks with CSV history
bench-history:
	./scripts/bench-history.sh

# Clean build artifacts
clean:
	cargo clean
