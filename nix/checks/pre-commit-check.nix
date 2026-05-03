{ inputs, pkgs, ... }:
let
  inherit (pkgs.stdenv.hostPlatform) system;

  treefmtEval = inputs.treefmt.lib.evalModule pkgs ../treefmt.nix;

  fenix = inputs.fenix.packages.${system};
  rustToolchain = fenix.combine [
    fenix.stable.cargo
    fenix.stable.clippy
    fenix.stable.rustfmt
  ];
in
inputs.git-hooks.lib.${system}.run {
  src = inputs.self;
  hooks = {
    nil.enable = true;
    statix.enable = true;
    treefmt = {
      enable = true;
      package = treefmtEval.config.build.wrapper;
    };
    rustfmt = {
      enable = true;
      packageOverrides.cargo = rustToolchain;
      packageOverrides.rustfmt = rustToolchain;
    };
    clippy = {
      enable = true;
      packageOverrides.cargo = rustToolchain;
      packageOverrides.clippy = rustToolchain;
      settings = {
        denyWarnings = true;
        allFeatures = true;
        offline = false;
      };
    };
  };
}
