{ system ? builtins.currentSystem }:

(builtins.getFlake (toString ./tools)).devShell.${system}
