class Srchr < Formula
  desc "Interactive live-grep file search"
  homepage "https://github.com/webcodr/srchr"
  version "VERSION"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/webcodr/srchr/releases/download/TAG/srchr-TAG-macos-aarch64.tar.gz"
      sha256 "SHA256_MACOS_AARCH64"
    else
      url "https://github.com/webcodr/srchr/releases/download/TAG/srchr-TAG-macos-x86-64.tar.gz"
      sha256 "SHA256_MACOS_X86_64"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/webcodr/srchr/releases/download/TAG/srchr-TAG-linux-aarch64.tar.gz"
      sha256 "SHA256_LINUX_AARCH64"
    else
      url "https://github.com/webcodr/srchr/releases/download/TAG/srchr-TAG-linux-x86-64.tar.gz"
      sha256 "SHA256_LINUX_X86_64"
    end
  end

  def install
    bin.install "srchr"
  end

  test do
    assert_match "Live-grep file search", shell_output("#{bin}/srchr --help")
  end
end
