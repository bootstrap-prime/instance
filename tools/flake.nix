{
  description = "Nix flake to pin everything in place for the rust dev env.";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url  = "github:numtide/flake-utils";
  };

  outputs = inputs@{ self, flake-utils, nixpkgs, rust-overlay, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
      in
        rec {
          devShell = let
            rust = (pkgs.rust-bin.selectLatestNightlyWith (toolchain:
              toolchain.default.override {
                extensions = [ "rust-src" "rustfmt" ];
              }));
          # unfortunately we cannot use an llvm environment because littlefs requires gcc to build.
          in pkgs.mkShell {
            nativeBuildInputs = with pkgs; [
              # get current rust toolchain defaults (this includes clippy and rustfmt)
              rust

              # for a good developer experience
              cargo-edit
              rust-analyzer
            ];

            CARGO_NET_GIT_FETCH_WITH_CLI = "true";
            RUST_BACKTRACE = 1;
          };
        }
    );
}
