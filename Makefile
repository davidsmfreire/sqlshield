hooks:
	pip install pre-commit
	pre-commit install
	pre-commit install --hook-type commit-msg

venv:
	python -m venv .venv

deps:
	pip install 'maturin[patchelf]'

dev-setup: deps hooks venv

develop:
	maturin develop -m sqlshield-cli/Cargo.toml
	maturin develop -m sqlshield-py/Cargo.toml
