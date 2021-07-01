{ pkgs ? import <nixpkgs> {} }:

with pkgs;

mkShell {
  buildInputs = [
    cargo
    cargo-edit
    clippy
    rustfmt
    rustc
    rustracer
    rust-analyzer
  ];

  CARGO_NET_GIT_FETCH_WITH_CLI = "true";
}
