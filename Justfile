set dotenv-load := false

_default:
    @just --list

# Runs clippy on the sources
check:
	cargo clippy --locked -- -D warnings

# Runs unit tests
test:
	cargo test --locked

# Format all code using rustfmt
fmt:
	cargo fmt --all
