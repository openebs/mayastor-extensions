{ norust ? false }:
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
  channel = import ./nix/lib/rust.nix { inherit sources; };
  # python environment for tests/bdd
  pytest_inputs = python3.withPackages
    (ps: with ps; [ virtualenv grpcio grpcio-tools black ]);
in
mkShell {
  name = "exporter-shell";
  buildInputs = [
    cargo-expand
    cargo-udeps
    commitlint
    git
    pkg-config
    pre-commit
    pytest_inputs
    python3
  ] ++ pkgs.lib.optional (!norust) channel.default_src.nightly;

  PROTOC = "${protobuf}/bin/protoc";
  PROTOC_INCLUDE = "${protobuf}/include";

  shellHook = ''
    ${pkgs.lib.optionalString (norust) "cowsay ${norust_moth}"}
    ${pkgs.lib.optionalString (norust) "echo 'Hint: use rustup tool.'"}
    ${pkgs.lib.optionalString (norust) "echo"}
    pre-commit install
    pre-commit install --hook commit-msg
    export EXTENSIONS_SRC=`pwd`
  '';
}
