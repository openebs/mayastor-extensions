{ norust ? false, devrustup ? true, rust-profile ? "stable" }:
let
  pkgs = import ./nix/pkgs.nix;
  lib = pkgs.lib;
in
let
  norust_moth =
    "You have requested an environment without rust, you should provide it!";
  devrustup_moth =
    "You have requested an environment for rustup, you should provide it!";
  rust-shell = import ./nix/shells/rust.nix { inherit pkgs rust-profile; cc = pkgs.clang; };
  build-shell = import ./nix/shells/build.nix { inherit pkgs; };
  rust = rust-shell.rust;
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
    commitlint
    coreutils
    cowsay
    jq
    niv
    nixpkgs-fmt
    utillinux
  ] ++ pkgs.lib.optional (usePreCommit) pre-commit;
in
pkgs.mkShellNoCC {
  name = "extensions-shell";
  inputsFrom = [ build-shell ] ++ pkgs.lib.optional (!norust) rust-shell;
  buildInputs = buildInputs
    ++ k8sShellAttrs.buildInputs ++ helmShellAttrs.buildInputs ++ bddShellAttrs.buildInputs;

  # using the nix rust toolchain
  USE_NIX_RUST = "${toString (!norust)}";
  # copy the rust toolchain to a writable directory, see: https://github.com/rust-lang/cargo/issues/10096
  # the whole toolchain is copied to allow the src to be retrievable through "rustc --print sysroot"
  RUST_TOOLCHAIN = ".rust-toolchain/${rust.version}";
  RUST_TOOLCHAIN_NIX = pkgs.lib.optional (!norust) "${rust}";

  shellHook = ''
    ./scripts/nix/git-submodule-init.sh
    export TMPDIR=/tmp
    if [ "${toString usePreCommit}" = "1" ]; then
      echo
      pre-commit install
      pre-commit install --hook commit-msg
    fi
    # Looks like vscode with the nix plugin is running commands in different shells without setting the temp variables
    # Since nix-shell by default nests the temp variables, we need to set them here
    if [ "$VSCODE_CLI" = "1" ] || [ -n "$VSCODE_CWD" ]; then
      export TMPDIR=/tmp
      export TMP=/tmp
    fi

    export EXTENSIONS_SRC=`pwd`
    export CTRL_SRC="$EXTENSIONS_SRC"/dependencies/control-plane
    export PATH="$(pwd)/target/debug:$PATH"

    ${lib.optionalString (norust) "cowsay ${norust_moth}"}
    ${lib.optionalString (norust) "echo"}

    rust_version="${rust.version}" rustup_channel="${lib.strings.concatMapStringsSep "-" (x: x) (lib.lists.drop 1 (lib.strings.splitString "-" rust.version))}" \
    dev_rustup="${toString (devrustup)}" devrustup_moth="${devrustup_moth}" . "$CTRL_SRC"/scripts/rust/env-setup.sh
  '';
}
