{ norust ? false, devrustup ? true, rust-profile ? "nightly" }:
let
  sources = import ./nix/sources.nix;
  pkgs = import sources.nixpkgs {
    overlays = [ (_: _: { inherit sources; }) (import ./nix/overlay.nix { }) ];
  };
in
with pkgs;
let
  norust_moth =
    "You have requested an environment without rust, you should provide it!";
  devrustup_moth =
    "You have requested an environment for rustup, you should provide it!";
  channel = import ./nix/lib/rust.nix { inherit sources; };
  rust_chan = channel.default_src;
  rust = rust_chan.${rust-profile};
in
mkShell {
  name = "extensions-shell";
  buildInputs = [
    cacert
    cargo-expand
    cargo-udeps
    clang
    commitlint
    cowsay
    git
    helm-docs
    kubernetes-helm-wrapped
    llvmPackages.libclang
    niv
    nixpkgs-fmt
    openapi-generator
    openssl
    pkg-config
    pre-commit
    python3
    semver-tool
    utillinux
    which
    yq-go
  ] ++ pkgs.lib.optional (!norust) channel.default_src.nightly
  ++ pkgs.lib.optional (system == "aarch64-darwin") darwin.apple_sdk.frameworks.Security;

  PROTOC = "${protobuf}/bin/protoc";
  PROTOC_INCLUDE = "${protobuf}/include";
  NODE_PATH = "${nodePackages."@commitlint/config-conventional"}/lib/node_modules";

  # using the nix rust toolchain
  USE_NIX_RUST = "${toString (!norust)}";
  # copy the rust toolchain to a writable directory, see: https://github.com/rust-lang/cargo/issues/10096
  # the whole toolchain is copied to allow the src to be retrievable through "rustc --print sysroot"
  RUST_TOOLCHAIN = ".rust-toolchain/${rust.version}";
  RUST_TOOLCHAIN_NIX = "${rust}";

  shellHook = ''
    ./scripts/nix/git-submodule-init.sh
    pre-commit install
    pre-commit install --hook commit-msg
    export EXTENSIONS_SRC=`pwd`
    export CTRL_SRC="$EXTENSIONS_SRC"/dependencies/control-plane
    export PATH="$PATH:$(pwd)/target/debug"

    ${pkgs.lib.optionalString (norust) "cowsay ${norust_moth}"}
    ${pkgs.lib.optionalString (norust) "echo"}

    rustup_channel="${lib.strings.concatMapStringsSep "-" (x: x) (lib.lists.drop 1 (lib.strings.splitString "-" rust.version))}" \
    dev_rustup="${toString (devrustup)}" devrustup_moth="${devrustup_moth}" . "$CTRL_SRC"/scripts/rust/env-setup.sh
  '';
}
