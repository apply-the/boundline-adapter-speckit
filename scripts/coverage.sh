#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

cargo llvm-cov clean --workspace
cargo llvm-cov --workspace --all-features --lcov --output-path lcov.info "$@"