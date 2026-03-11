class Clawdius < Formula
  desc "AI-powered coding assistant with multiple LLM support"
  homepage "https://github.com/clawdius/clawdius"
  version "0.2.0"
  license "Apache-2.0"
  
  livecheck do
    url :stable
    strategy :github_latest
  end
  
  on_macos do
    on_intel do
      url "https://github.com/clawdius/clawdius/releases/download/v#{version}/clawdius-#{version}-x86_64-apple-darwin.tar.gz"
      sha256 ""  # Will be filled during release
    end
    on_arm do
      url "https://github.com/clawdius/clawdius/releases/download/v#{version}/clawdius-#{version}-aarch64-apple-darwin.tar.gz"
      sha256 ""  # Will be filled during release
    end
  end
  
  on_linux do
    on_intel do
      url "https://github.com/clawdius/clawdius/releases/download/v#{version}/clawdius-#{version}-x86_64-unknown-linux-gnu.tar.gz"
      sha256 ""  # Will be filled during release
    end
    on_arm do
      url "https://github.com/clawdius/clawdius/releases/download/v#{version}/clawdius-#{version}-aarch64-unknown-linux-gnu.tar.gz"
      sha256 ""  # Will be filled during release
    end
  end
  
  def install
    bin.install "clawdius"
    bin.install "clawdius-code"
    
    generate_completions_from_executable(bin/"clawdius", "completions", shells: [:bash, :zsh, :fish])
    
    man1.install Utils.safe_popen_read(bin/"clawdius", "man") => "clawdius.1"
  end
  
  def caveats
    <<~EOS
      To complete the installation, you'll need to configure your API keys:
      
        clawdius config set api_key YOUR_API_KEY
      
      Or set environment variables:
      
        export ANTHROPIC_API_KEY=your_key
        export OPENAI_API_KEY=your_key
        export DEEPSEEK_API_KEY=your_key
      
      For VSCode integration, ensure clawdius-code is in your PATH.
    EOS
  end
  
  test do
    assert_match "clawdius #{version}", shell_output("#{bin}/clawdius --version")
    
    output = shell_output("#{bin}/clawdius --help")
    assert_match "AI-powered coding assistant", output
    
    system bin/"clawdius-code", "--version"
  end
end
