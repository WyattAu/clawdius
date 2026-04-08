{
  description = "Clawdius - Rust-native agentic coding engine with formal verification";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" "clippy" "rustfmt" "llvm-tools" ];
        };

        commonBuildInputs = with pkgs; [
          openssl
          sqlite
          protobuf
        ] ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
          liburing
        ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
          pkgs.darwin.apple_sdk.frameworks.Security
          pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
          pkgs.darwin.apple_sdk.frameworks.CoreFoundation
        ];

        commonNativeBuildInputs = with pkgs; [
          pkg-config
          cmake
          protobuf
        ];

        buildRustBin = { pname, version }: pkgs.rustPlatform.buildRustPackage {
          inherit pname version;
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;

          inherit commonNativeBuildInputs;
          buildInputs = commonBuildInputs;
          nativeBuildInputs = commonNativeBuildInputs;

          PROTOC = "${pkgs.protobuf}/bin/protoc";

          cargoBuildFlags = [ "--package" pname ];
          cargoTestFlags = [ "--package" pname ];
        };

        version = "1.6.0";

        cargoTools = with pkgs; [
          cargo-nextest
          cargo-deny
          cargo-audit
          cargo-machete
        ];

        formalTools = pkgs.lib.optionals pkgs.stdenv.isLinux [ pkgs.lean4 ];
      in
      {
        packages = {
          clawdius = buildRustBin { pname = "clawdius"; inherit version; };
          clawdius-mcp = buildRustBin { pname = "clawdius-mcp"; inherit version; };
          clawdius-code = buildRustBin { pname = "clawdius-code"; inherit version; };
          clawdius-server = buildRustBin { pname = "clawdius-server"; inherit version; };
          default = self.packages.${system}.clawdius;
        };

        devShells.default = pkgs.mkShell {
          packages = [ rustToolchain ] ++ cargoTools ++ formalTools;

          buildInputs = commonBuildInputs;
          nativeBuildInputs = commonNativeBuildInputs;

          PROTOC = "${pkgs.protobuf}/bin/protoc";
          PKG_CONFIG_PATH = "${pkgs.lib.makeSearchPath "lib/pkgconfig" [ pkgs.openssl.dev ]}";

          shellHook = ''
            export RUST_BACKTRACE=1
            export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath commonBuildInputs}''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
          '';
        };

        formatter = pkgs.nixfmt-rfc-style;
      }
    );
}
