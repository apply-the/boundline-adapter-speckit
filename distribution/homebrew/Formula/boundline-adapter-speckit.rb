# frozen_string_literal: true

class BoundlineAdapterSpeckit < Formula
  desc "Workflow bridge from Boundline into Spec Kit"
  homepage "https://github.com/apply-the/boundline-adapter-speckit"
  url "https://github.com/apply-the/boundline-adapter-speckit", using: :git, tag: "0.1.0"
  version "0.1.0"
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
