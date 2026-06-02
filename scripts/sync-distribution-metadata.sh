#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

metadata_file="distribution/channel-metadata.toml"
adapter_version="$(sed -n 's/^version = "\([^"]*\)"$/\1/p' Cargo.toml | head -n 1)"
metadata_version="$(sed -n 's/^adapter_version = "\([^"]*\)"$/\1/p' "$metadata_file" | head -n 1)"

if [[ -z "$adapter_version" || -z "$metadata_version" ]]; then
  echo "failed to resolve adapter version from Cargo.toml or distribution/channel-metadata.toml" >&2
  exit 1
fi

if [[ "$adapter_version" != "$metadata_version" ]]; then
  echo "Cargo.toml version $adapter_version does not match distribution/channel-metadata.toml version $metadata_version" >&2
  exit 1
fi

homebrew_formula="distribution/homebrew/Formula/boundline-adapter-speckit.rb"

mkdir -p "$(dirname "$homebrew_formula")"

cat > "$homebrew_formula" <<EOF
# frozen_string_literal: true

class BoundlineAdapterSpeckit < Formula
  desc "Workflow bridge from Boundline into Spec Kit"
  homepage "https://github.com/apply-the/boundline-adapter-speckit"
  url "https://github.com/apply-the/boundline-adapter-speckit", using: :git, tag: "${adapter_version}"
  version "${adapter_version}"
  license "MIT"

  head "https://github.com/apply-the/boundline-adapter-speckit", branch: "main", using: :git

  livecheck do
    url :stable
    strategy :git do |tags|
      tags.filter_map { |tag| tag[/^(\d+\.\d+\.\d+)$/, 1] }.max_by { |value| Gem::Version.new(value) }
    end
  end

  depends_on "rustup" => :build

  def install
    rustup_bin = Formula["rustup"].opt_bin/"rustup"
    cargo_bin = Formula["rustup"].opt_bin/"cargo"

    install_toolchain(rustup_bin, toolchain_version_for(buildpath) || "stable")

    ENV["CARGO_NET_GIT_FETCH_WITH_CLI"] = "true"

    system cargo_bin, "install",
           "--locked",
           "--path", ".",
           "--root", prefix
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/boundline-adapter-speckit --version")
  end

  private

  def toolchain_version_for(root)
    toolchain_file = root/"rust-toolchain.toml"
    return nil unless toolchain_file.exist?

    toolchain_file.read[/channel\s*=\s*"([^"]+)"/, 1]
  end

  def install_toolchain(rustup_bin, toolchain_version)
    system rustup_bin, "toolchain", "install", toolchain_version,
           "--profile", "minimal",
           "--component", "rustfmt",
           "--component", "clippy",
           "--no-self-update"
  end
end
EOF

echo "Synced Homebrew formula for Boundline Adapter Speckit ${adapter_version}."
echo "Homebrew formula: ${homebrew_formula}"