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

# --- VS Code extension --------------------------------------------------
#
# Build the VSIX locally. By default it bundles the sqlshield-lsp binary
# for the host platform only; `vscode-vsix-target TARGET=…` builds for
# any target rustc can hit (cross-compiling toolchain must already be
# installed). The marketplace `--target` is inferred from TARGET.

VSCODE_DIR := editors/vscode
DIST_DIR   := dist

# Host (no --target): produce a generic, untargeted VSIX. Fine for `code
# --install-extension` on whatever machine you run this on.
vscode-vsix:
	cargo build -p sqlshield-lsp --release
	mkdir -p $(VSCODE_DIR)/server
	cp target/release/sqlshield-lsp $(VSCODE_DIR)/server/sqlshield-lsp
	chmod +x $(VSCODE_DIR)/server/sqlshield-lsp
	cd $(VSCODE_DIR) && npm ci && npm run compile
	mkdir -p $(DIST_DIR)
	cd $(VSCODE_DIR) && npx --yes @vscode/vsce package \
		--out ../../$(DIST_DIR)/sqlshield-host.vsix

# Cross-targeted build. Example:
#   make vscode-vsix-target TARGET=aarch64-apple-darwin VSCE_TARGET=darwin-arm64
# Maps:
#   x86_64-unknown-linux-gnu  -> linux-x64
#   aarch64-unknown-linux-gnu -> linux-arm64
#   x86_64-apple-darwin       -> darwin-x64
#   aarch64-apple-darwin      -> darwin-arm64
#   x86_64-pc-windows-msvc    -> win32-x64
#   aarch64-pc-windows-msvc   -> win32-arm64
vscode-vsix-target:
	@if [ -z "$(TARGET)" ] || [ -z "$(VSCE_TARGET)" ]; then \
		echo "TARGET= and VSCE_TARGET= are required"; exit 1; \
	fi
	rustup target add $(TARGET)
	cargo build -p sqlshield-lsp --release --target $(TARGET)
	mkdir -p $(VSCODE_DIR)/server
	@if [ "$(VSCE_TARGET)" = "win32-x64" ] || [ "$(VSCE_TARGET)" = "win32-arm64" ]; then \
		cp target/$(TARGET)/release/sqlshield-lsp.exe $(VSCODE_DIR)/server/sqlshield-lsp.exe; \
	else \
		cp target/$(TARGET)/release/sqlshield-lsp $(VSCODE_DIR)/server/sqlshield-lsp; \
		chmod +x $(VSCODE_DIR)/server/sqlshield-lsp; \
	fi
	cd $(VSCODE_DIR) && npm ci && npm run compile
	mkdir -p $(DIST_DIR)
	cd $(VSCODE_DIR) && npx --yes @vscode/vsce package \
		--target $(VSCE_TARGET) \
		--out ../../$(DIST_DIR)/sqlshield-$(VSCE_TARGET).vsix

vscode-clean:
	rm -rf $(VSCODE_DIR)/server $(VSCODE_DIR)/out $(DIST_DIR)

.PHONY: hooks venv deps develop dev-setup build test all clean \
	vscode-vsix vscode-vsix-target vscode-clean
