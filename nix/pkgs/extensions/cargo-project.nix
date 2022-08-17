{ stdenv
, lib
, makeRustPlatform
, pkg-config
, protobuf
, sources
, pkgs
, openssl
, git
, version
  # with allInOne set to true all components are built as part of the same "cargo build" derivation
  # this allows for a quicker build of all components but slower single components
  # with allInOne set to false each component gets its own "cargo build" derivation allowing for faster
  # individual builds but making the build of all components at once slower
, allInOne ? true
  # EXPERIMENTAL incremental allows for faster incremental builds as the build dependencies are cached
  # it might make the initial build slightly slower as it's done in two steps
  # for this we use naersk which is not as fully fledged as the builtin rustPlatform so it should only be used
  # for development and not for CI
, incremental ? false
}:
let
  channel = import ../../lib/rust.nix { inherit sources; };
  stable_channel = {
    rustc = channel.default.stable;
    cargo = channel.default.stable;
  };
  rustPlatform = makeRustPlatform {
    rustc = stable_channel.rustc;
    cargo = stable_channel.cargo;
  };
  naersk = pkgs.callPackage sources.naersk {
    rustc = stable_channel.rustc;
    cargo = stable_channel.cargo;
  };
  whitelistSource = src: allowedPrefixes:
    builtins.filterSource
      (path: type:
        lib.any
          (allowedPrefix: lib.hasPrefix (toString (src + "/${allowedPrefix}")) path)
          allowedPrefixes)
      src;
  PROTOC = "${protobuf}/bin/protoc";
  PROTOC_INCLUDE = "${protobuf}/include";
  src_list = [
    ".git"
    "Cargo.lock"
    "Cargo.toml"
    "exporter"
    "rpc"
    "operators"
  ];
  buildProps = rec {
    name = "extensions-${version}";
    inherit version;

    src = whitelistSource ../../../. src_list;

    inherit PROTOC PROTOC_INCLUDE;
    nativeBuildInputs = [ pkg-config git ];
    buildInputs = [ protobuf ];
    doCheck = false;
  };
  release_build = { "release" = true; "debug" = false; };
in
let
  build_with_naersk = { buildType, cargoBuildFlags }:
    naersk.buildPackage (buildProps // {
      release = release_build.${buildType};
      cargoBuildOptions = attrs: attrs ++ cargoBuildFlags;
      doCheck = false;
      usePureFromTOML = true;
    });
  build_with_default = { buildType, cargoBuildFlags }:
    rustPlatform.buildRustPackage (buildProps // {
      inherit buildType cargoBuildFlags;
      cargoLock = {
        lockFile = ../../../Cargo.lock;
      };
    });
  builder = if incremental then build_with_naersk else build_with_default;
in
{
  inherit PROTOC PROTOC_INCLUDE version src_list;

  build = { buildType, cargoBuildFlags ? [ ] }:
    if allInOne then
      builder { inherit buildType; cargoBuildFlags = [ "-p rpc" "-p exporter" "-p operators" ]; }
    else
      builder { inherit buildType cargoBuildFlags; };
}
