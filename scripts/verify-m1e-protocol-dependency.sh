#!/usr/bin/env bash
set -euo pipefail

readonly qualified_requirement="=0.90.0"
readonly supported_range=">=0.90.0,<1.0.0"

metadata="$(cargo metadata --no-deps --format-version 1)"

jq -e \
  '.packages[0]
   | .version == "0.1.0"
     and .edition == "2024"
     and .rust_version == "1.96.0"
     and .license == "MIT"
     and (.description | length > 0)
     and (.repository | startswith("https://github.com/"))
     and (.homepage | startswith("https://github.com/"))' \
  <<<"$metadata" >/dev/null

jq -e \
  --arg requirement "$qualified_requirement" \
  '.packages[0].dependencies
   | any(.name == "boundline-protocol"
         and .req == $requirement
         and (.source | startswith("registry+"))
         and .path == null)
     and (all(.name != "boundline-adapters"))' \
  <<<"$metadata" >/dev/null

jq -e \
  --arg supported_range "$supported_range" \
  '.packages[0].metadata["boundline-framework-adapter"].supported_boundline_range
   == $supported_range' \
  <<<"$metadata" >/dev/null

if rg -n 'boundline-adapters|tag = "0\.66\.0"|>=0\.66\.0,<0\.67\.0' \
  Cargo.toml Cargo.lock src tests README.md CHANGELOG.md >/dev/null; then
  echo "legacy Boundline 0.66 bridge remains" >&2
  exit 1
fi

printf 'Speckit M1E protocol dependency checks passed.\n'
