hooks:
	pip install pre-commit
	pre-commit install
	pre-commit install --hook-type commit-msg

venv:
	python -m venv .venv

deps:
	source .venv/bin/activate && \
	pip install -r requirements-dev.txt

# (Re)Install Python packages in current venv
develop:
	source .venv/bin/activate && \
	maturin develop -m sqlshield-cli/Cargo.toml && \
	maturin develop -m sqlshield-py/Cargo.toml

# Run for development
dev-setup: venv deps hooks develop

# Build Rust binaries and Python wheels
build:
	cargo build
	source .venv/bin/activate && \
	maturin build -m sqlshield-cli/Cargo.toml && \
	maturin build -m sqlshield-py/Cargo.toml

# Run tests for Rust and Python
test:
	cargo test
	source .venv/bin/activate && \
	python -m pytest .

all: clean dev-setup build test

clean:
	rm -rf .venv && \
	cargo clean
