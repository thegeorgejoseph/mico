run:
	cargo run -- dashboard

doctor:
	cargo run -- doctor

paths:
	cargo run -- paths

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

check:
	cargo check --workspace --all-targets

lint:
	cargo clippy --workspace --all-targets -- -D warnings

test:
	cargo test --workspace --all-targets

ci: fmt-check check test

release:
	cargo run -p xtask -- release $(or $(BUMP),patch)

ship:
	cargo run -p xtask -- ship $(or $(BUMP),patch)

update-tap:
	cargo run -p xtask -- update-tap $(VERSION)
