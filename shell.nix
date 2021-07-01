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
  # courtesy of https://discourse.nixos.org/t/rust-src-not-found-and-other-misadventures-of-developing-rust-on-nixos/11570/10
  RUST_SRC_PATH = "${rust.packages.stable.rustPlatform.rustLibSrc}";
  RACER_RUST_SRC_PATH = "${rust.packages.stable.rustPlatform.rustLibSrc}";
  #RACER_CMD = "${pkgs.rustracer}/bin/racer";

  # shellHook = ''
  #   echo "${rust.packages.stable.rustPlatform.rustLibSrc}"
  #   echo "${rustracer}"
  # '';

  # shellHook = ''
  #   export PATH="${pkgs.cargo}/bin/cargo:$PATH";
  # '';
}
