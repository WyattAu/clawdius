{
  description = "Clawdius: High-Assurance Rust-Native Engineering Engine";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        
        # Define the Rust toolchain (using the latest stable for 2024 edition support)
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" "clippy" ];
        };

        # Native dependencies for Clawdius
        nativeBuildInputs = with pkgs; [
          rustToolchain
          pkg-config
          gnumake
          cmake
        ];

        # Runtime libraries and tools required by Clawdius
        buildInputs = with pkgs; [
          openssl
          sqlite
          libiconv
          
          # Sentinel Sandboxing Tools
          bubblewrap
          podman
          
          # Formal Verification Engine
          lean4
          
          # Tree-sitter for AST Indexing
          tree-sitter
        ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
          pkgs.darwin.apple_sdk.frameworks.Security
          pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
          pkgs.darwin.apple_sdk.frameworks.CoreFoundation
        ];
      in
      {
        devShells.default = pkgs.mkShell {
          inherit nativeBuildInputs buildInputs;

          # Set environment variables for Rust builds
          shellHook = ''
            export RUST_BACKTRACE=1
            export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath buildInputs}:$LD_LIBRARY_PATH"
            
            echo "🦀 Clawdius Environment Materialized"
            echo "   - Phase: -0.5 (Environment Materialization)"
            echo "   - Runtime: Rust $(rustc --version)"
            echo "   - Sentinel: $(bwrap --version | head -n 1)"
            echo "   - Formal: $(lean --version)"
          '';

          # Integration for crates like openssl-sys
          PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";
        };
      }
    );
}