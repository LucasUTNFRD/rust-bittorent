PHONY: run-daemon test check clippy fmt

run-daemon:
	RUST_LOG=debug cargo run -p bittorent_daemon

test:
	cargo test


check:
	cargo check

clippy:
	cargo clippy  -- -D warnings

fmt:
	cargo fmt



