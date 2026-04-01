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
          extensions = [ "rust-src" "rust-analyzer" "clippy" "llvm-tools" ];
        };

        # Cargo tooling for high-assurance development
        cargoTools = with pkgs; [
          # Testing
          cargo-nextest           # Parallel test runner (replaces cargo test)
          
          # Supply Chain Security
          cargo-deny              # Dependency linting and CVE checking
          cargo-vet               # Supply chain auditing
          
          # Code Quality
          cargo-machete           # Unused dependency detection
          cargo-mutants           # Mutation testing
          
          # Performance
          cargo-criterion         # Benchmarking
          
          # Security
          cargo-audit             # Security vulnerability auditing
        ];

        # Native dependencies for Clawdius
        nativeBuildInputs = with pkgs; [
          rustToolchain
          pkg-config
          gnumake
          cmake
        ] ++ cargoTools ++ formalTools;

        # Runtime libraries and tools required by Clawdius
        buildInputs = with pkgs; [
          openssl
          sqlite
          libiconv
          protobuf               # Protocol Buffers compiler (protoc) for lancedb

          # Tree-sitter for AST Indexing
          tree-sitter
        ] ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
          liburing               # io_uring support for monoio (Linux-only)
          bubblewrap             # Sentinel sandboxing (Linux-only)
          podman                 # Container sandboxing (Linux-only)
          perf-tools             # Performance analysis (Linux-only)
          valgrind               # Memory debugging (Linux-only)
        ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
          pkgs.darwin.apple_sdk.frameworks.Security
          pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
          pkgs.darwin.apple_sdk.frameworks.CoreFoundation
        ];

        # Formal verification is Linux-only and optional
        formalTools = pkgs.lib.optionals pkgs.stdenv.isLinux [ pkgs.lean4 ];

      in
      {
        devShells.default = pkgs.mkShell {
          inherit nativeBuildInputs buildInputs;

          # Set environment variables for Rust builds
          shellHook = ''
            export RUST_BACKTRACE=1
            export RUSTFLAGS="-D warnings -C target-cpu=native"
            export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath buildInputs}''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
            export PROTOC="${pkgs.protobuf}/bin/protoc"

            echo "🦀 Clawdius Environment Materialized"
            echo "   ═══════════════════════════════════"
            echo "   Phase: -0.5 (Environment Materialization)"
            echo "   Runtime: Rust $(rustc --version)"
            echo "   Async: monoio (io_uring, thread-per-core)"
            echo "   ═══════════════════════════════════"
            echo "   Tooling:"
            echo "   - cargo-nextest: $(cargo nextest --version 2>/dev/null || echo 'available')"
            echo "   - cargo-deny: $(cargo deny --version 2>/dev/null || echo 'available')"
            echo "   - cargo-vet: $(cargo vet --version 2>/dev/null || echo 'available')"
            echo "   - cargo-mutants: $(cargo mutants --version 2>/dev/null || echo 'available')"
            echo "   ═══════════════════════════════════"
            ${pkgs.lib.optionalString pkgs.stdenv.isLinux ''
            echo "   Sentinel: $(bwrap --version | head -n 1)"
            ''}
            ${pkgs.lib.optionalString pkgs.stdenv.isLinux ''
            echo "   Formal: $(lean --version 2>/dev/null | head -n 1 || echo 'not available')"
            ''}
            echo ""
            echo "   Ready for high-assurance development!"
          '';

          # Integration for crates like openssl-sys
          PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";
        };

        # Package definition for building the binary
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "clawdius";
          version = "1.2.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
          
          nativeBuildInputs = [ pkgs.pkg-config pkgs.cmake pkgs.protobuf ];
          buildInputs = with pkgs; [
            openssl
            sqlite
            libiconv
            protobuf
          ] ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
            liburing
          ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
            pkgs.darwin.apple_sdk.frameworks.Security
            pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
            pkgs.darwin.apple_sdk.frameworks.CoreFoundation
          ];

          PROTOC = "${pkgs.protobuf}/bin/protoc";
        };
      }
    );
}
