# Native dependencies required to build the rust code
{ pkgs ? import ../pkgs.nix }:
with pkgs;
mkShellNoCC {
  name = "build-shell";
  buildInputs = [
    clang
    # aws-lc-rs fips
    cmake
    go
    perl
    llvmPackages.libclang
    pkg-config
    # control-plane openapi/build.rs bindings generation
    paperclip
    git
  ] ++ lib.optional (system == "aarch64-darwin") darwin.apple_sdk.frameworks.Security;

  shellHook = ''
    export PROTOC="${protobuf}/bin/protoc"
    export PROTOC_INCLUDE="${protobuf}/include"
  '';
}
