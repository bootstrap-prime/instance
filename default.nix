#{ pkgs, lib, rustPlatform, ... }:
{ pkgs ? import <nixpkgs> {} }:

pkgs.rustPlatform.buildRustPackage {
  pname = "instance";
  version = "0.1.0";

  src = ./.;

  cargoSha256 = "sha256-zHpSn3RQBhi5MOnwalH5rx0vmG+zfWo1hmFjh2UB4tc=";

  meta = with pkgs.lib; {
    description = "template copier for common operations";
    homepage = "https://github.com/bootstrap-prime/instance";
    license = licenses.mit;
    maintainers = with maintainers; [ bootstrap-prime ];
    platforms = platforms.linux;
  };
}
