{ inputs, pkgs, ... }:
let
  inherit (pkgs) lib;

  pre-commit-check = import ./checks/pre-commit-check.nix { inherit inputs pkgs; };

  inherit (pkgs.stdenv.hostPlatform) system;

  fenix = inputs.fenix.packages.${system};
  rustToolchain = fenix.combine [
    fenix.stable.cargo
    fenix.stable.rustc
    fenix.stable.clippy
    fenix.stable.rustfmt
    fenix.stable.rust-src
    fenix.stable.rust-analyzer
    fenix.targets.wasm32-unknown-unknown.stable.rust-std
  ];

  craneLib = (inputs.crane.mkLib pkgs).overrideToolchain rustToolchain;

  inherit (pkgs.stdenv.hostPlatform) isLinux;
  cargoTarget = lib.toUpper (lib.replaceStrings [ "-" ] [ "_" ] pkgs.stdenv.hostPlatform.config);
in
craneLib.devShell {
  packages = [
    pkgs.sccache
    pkgs.wasm-pack
    pkgs.wasm-bindgen-cli
  ]
  ++ lib.optionals isLinux [
    pkgs.mold
    pkgs.clang
  ];

  env = {
    RUSTC_WRAPPER = lib.getExe pkgs.sccache;
  }
  // lib.optionalAttrs isLinux {
    "CARGO_TARGET_${cargoTarget}_LINKER" = lib.getExe pkgs.clang;
    "CARGO_TARGET_${cargoTarget}_RUSTFLAGS" = "-C link-arg=-fuse-ld=${lib.getExe pkgs.mold}";
  };

  shellHook = ''
    ${pre-commit-check.shellHook}
  '';
}
