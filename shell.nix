{ pkgs ? import <nixpkgs> {} }:

with pkgs;

mkShell {
  buildInputs = [
    cargo
    cargo-edit
    clippy
    rustfmt
    rustc
  ];

  CARGO_NET_GIT_FETCH_WITH_CLI = "true";

  INSTANCE_TEMPLATE_DIR = /home/bootstrap/projects/instance/templates;
}
