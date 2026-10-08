class Rqlang < Formula
  desc "Manage and execute HTTP requests defined in .rq files"
  homepage "https://github.com/rqlang/rq"
  license "Apache-2.0"

  on_macos do
    on_arm do
      url "https://github.com/rqlang/rq/releases/download/1.2.3/rq-macos-aarch64"
      sha256 "4444444444444444444444444444444444444444444444444444444444444444"
    end
    on_intel do
      url "https://github.com/rqlang/rq/releases/download/1.2.3/rq-macos-x86_64"
      sha256 "5555555555555555555555555555555555555555555555555555555555555555"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/rqlang/rq/releases/download/1.2.3/rq-linux-aarch64"
      sha256 "1111111111111111111111111111111111111111111111111111111111111111"
    end
    on_intel do
      url "https://github.com/rqlang/rq/releases/download/1.2.3/rq-linux-x86_64"
      sha256 "3333333333333333333333333333333333333333333333333333333333333333"
    end
  end

  def install
    bin.install Dir["rq-*"].first => "rq"
    chmod 0755, bin/"rq"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/rq --version")
  end
end
