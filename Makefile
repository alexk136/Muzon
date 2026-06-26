# Muzon developer entry points.
#
# Conventions:
# - All targets call the project-local `cargo`, never a system-wide
#   `cargo`. The toolchain is pinned in `rust-toolchain.toml`; on a
#   `rustup`-managed system, `cargo` resolves to the right toolchain
#   automatically.
# - All targets are PHONY (no file-name collisions in the root).
# - The agent launcher include from `Makefile.agent` is kept last so
#   `make <role>` keeps working.

SHELL := /bin/bash
.SHELLFLAGS := -eu -o pipefail -c
.DEFAULT_GOAL := help

# Make `cargo` resolve to the rustup-managed toolchain if it exists on
# PATH. On developer machines without rustup, fall back to whatever
# `cargo` is on PATH. CI uses rustup so this is the common path.
ifneq (,$(shell command -v rustup 2>/dev/null))
CARGO := cargo
else
CARGO := $(shell command -v cargo 2>/dev/null || echo cargo)
endif

# Where the local toolchain lives if rustup is installed but not on
# PATH (e.g. when this Makefile runs in a container without
# $HOME/.cargo/bin). This matches the rustup default. We also put
# the bin on PATH so `rustc` (which cargo invokes via `rustc -vV`)
# resolves correctly. The toolchain channel is `stable` (resolved by
# rustup to whatever the latest stable is); the actual directory name
# is `stable-<target-triple>`.
RUSTUP_TOOLCHAIN_BIN ?= $(HOME)/.rustup/toolchains/stable-$(shell $(CARGO) -V 2>/dev/null | head -1 | awk '{print $$4}')/bin
ifeq (,$(wildcard $(RUSTUP_TOOLCHAIN_BIN)/cargo))
# Fall back: scan the toolchains directory for any installed toolchain.
RUSTUP_TOOLCHAIN_BIN := $(firstword $(wildcard $(HOME)/.rustup/toolchains/*/bin))
endif
ifneq (,$(wildcard $(RUSTUP_TOOLCHAIN_BIN)/cargo))
CARGO := $(RUSTUP_TOOLCHAIN_BIN)/cargo
export PATH := $(RUSTUP_TOOLCHAIN_BIN):$(PATH)
endif

WORKSPACE := --workspace
RELEASE_FLAG ?=
TARGET_DIR := target

.PHONY: help all build build-release test test-release test-all fmt fmt-check clippy clippy-strict lint deny clean doc run-cli run-cli-play run-cli-paths install-tools

help: ## Show this help.
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / {printf "  \033[36m%-18s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)

all: build test ## Build + test (default for pre-merge checks).

build: ## cargo build --workspace (debug).
	$(CARGO) build $(WORKSPACE)

build-release: ## cargo build --workspace --release.
	$(CARGO) build $(WORKSPACE) --release

test: ## cargo test --workspace (debug).
	$(CARGO) test $(WORKSPACE) --no-fail-fast

test-release: ## cargo test --workspace --release.
	$(CARGO) test $(WORKSPACE) --release --no-fail-fast

test-all: ## Run unit + integration + doc tests.
	$(CARGO) test $(WORKSPACE) --all-features --no-fail-fast

fmt: ## cargo fmt (apply).
	$(CARGO) fmt --all

fmt-check: ## cargo fmt --check.
	$(CARGO) fmt --all -- --check

clippy: ## cargo clippy -- -D warnings.
	$(CARGO) clippy $(WORKSPACE) --all-targets -- -D warnings

clippy-strict: ## cargo clippy --all-features --all-targets.
	$(CARGO) clippy $(WORKSPACE) --all-features --all-targets -- -D warnings

lint: fmt-check clippy ## Combined: fmt-check + clippy.

deny: ## cargo deny check (licenses + advisories + bans).
	$(CARGO) deny check

clean: ## cargo clean.
	$(CARGO) clean

doc: ## cargo doc (open API docs).
	$(CARGO) doc $(WORKSPACE) --no-deps --open

run-cli: ## Run the muzon CLI (debug). Extra args go to the binary.
	$(CARGO) run -p muzon-cli --bin muzon -- $(ARGS)

run-cli-play: ## Run `muzon play <file>`. Usage: make run-cli-play FILE=path/to/file.flac
	$(CARGO) run -p muzon-cli --bin muzon -- play $(FILE)

run-cli-paths: ## Run `muzon --print-paths` to see resolved XDG paths.
	$(CARGO) run -p muzon-cli --bin muzon -- --print-paths

install-tools: ## Install cargo-deny, sqlx-cli, and rustfmt components.
	rustup component add rustfmt clippy
	$(CARGO) install --locked cargo-deny
	@echo "Optional: $(CARGO) install --locked sqlx-cli"

# >>> sync agent launcher include >>>
-include Makefile.agent
# <<< sync agent launcher include <<<
