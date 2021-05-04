{ pkgs ? import <nixpkgs> {} }:

with pkgs;

mkShell {
  buildInputs = [
    cargo
    cargo-edit
    rustfmt
  ];

  CARGO_NET_GIT_FETCH_WITH_CLI = "true";
}
