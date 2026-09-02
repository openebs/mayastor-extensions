{ norust ? false, devrustup ? true, rust-profile ? "stable" }:
let
  sources = import ./nix/sources.nix;
  pkgs = import sources.nixpkgs {
    overlays = [ (_: _: { inherit sources; }) (import ./nix/overlay.nix { }) (import sources.rust-overlay) ];
  };
  lib = pkgs.lib;
in
let
  norust_moth =
    "You have requested an environment without rust, you should provide it!";
  devrustup_moth =
    "You have requested an environment for rustup, you should provide it!";
  channel = import ./nix/lib/rust.nix { inherit pkgs; };
  rust_chan = channel.default_src;
  rust = rust_chan.${rust-profile}.overrideAttrs (oldAttrs: {
    # don't propagate any build inputs - this allows us to set cc in stdenv below
    propagatedBuildInputs = [ ];
    depsHostHostPropagated = [ pkgs.clang ];
    depsTargetTargetPropagated = [ ];
  });
  usePreCommit = builtins.getEnv "IN_NIX_SHELL" == "impure" && builtins.getEnv "CI" != "1";
  pre-commit = pkgs.runCommand "pre-commit" { } ''
    mkdir -p $out/bin
    cp ${pkgs.pre-commit}/bin/pre-commit $out/bin/pre-commit
  '';
  k8sShellAttrs = import ./scripts/k8s/shell.nix { inherit pkgs; };
  helmShellAttrs = import ./chart/shell.nix { inherit pkgs; };
  bddShellAttrs = import ./tests/bdd/shell.nix { inherit pkgs; };
  buildInputs = with pkgs; [
    cacert
    cargo-expand
    cargo-udeps
    clang
    commitlint
    coreutils
    cowsay
    git
    jq
    llvmPackages.libclang
    niv
    nixpkgs-fmt
    paperclip
    pkg-config
    utillinux
  ] ++ pkgs.lib.optional (usePreCommit) pre-commit;
in
pkgs.mkShellNoCC {
  name = "extensions-shell";
  buildInputs = buildInputs ++ pkgs.lib.optional (!norust) rust
    ++ k8sShellAttrs.buildInputs ++ helmShellAttrs.buildInputs ++ bddShellAttrs.buildInputs
    ++ pkgs.lib.optional (pkgs.system == "aarch64-darwin") pkgs.darwin.apple_sdk.frameworks.Security;

  PROTOC = "${pkgs.protobuf}/bin/protoc";
  PROTOC_INCLUDE = "${pkgs.protobuf}/include";

  # using the nix rust toolchain
  USE_NIX_RUST = "${toString (!norust)}";
  # copy the rust toolchain to a writable directory, see: https://github.com/rust-lang/cargo/issues/10096
  # the whole toolchain is copied to allow the src to be retrievable through "rustc --print sysroot"
  RUST_TOOLCHAIN = ".rust-toolchain/${rust.version}";
  RUST_TOOLCHAIN_NIX = pkgs.lib.optional (!norust) "${rust}";

  shellHook = ''
    ./scripts/nix/git-submodule-init.sh
    if [ "${toString usePreCommit}" = "1" ]; then
      echo
      pre-commit install
      pre-commit install --hook commit-msg
    fi
    export EXTENSIONS_SRC=`pwd`
    export CTRL_SRC="$EXTENSIONS_SRC"/dependencies/control-plane
    export PATH="$(pwd)/target/debug:$PATH"

    ${lib.optionalString (norust) "cowsay ${norust_moth}"}
    ${lib.optionalString (norust) "echo"}

    rust_version="${rust.version}" rustup_channel="${lib.strings.concatMapStringsSep "-" (x: x) (lib.lists.drop 1 (lib.strings.splitString "-" rust.version))}" \
    dev_rustup="${toString (devrustup)}" devrustup_moth="${devrustup_moth}" . "$CTRL_SRC"/scripts/rust/env-setup.sh
    unset CC
    unset AR
  '';
}
