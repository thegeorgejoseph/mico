default:
    @just --list

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

release bump="patch":
    cargo run -p xtask -- release {{bump}}

ship bump="patch":
    cargo run -p xtask -- ship {{bump}}

update-tap version:
    cargo run -p xtask -- update-tap {{version}}
