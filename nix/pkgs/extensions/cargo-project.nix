{ stdenv
, lib
, makeRustPlatform
, pkg-config
, protobuf
, sources
, pkgs
, clang
, libxfs
, llvmPackages
, openssl
, git
, gitVersions
, openapi-generator
, which
, utillinux
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
          (allowedPrefix:
            (lib.hasPrefix (toString (src + "/${allowedPrefix}")) path) ||
            (type == "directory" && lib.hasPrefix path (toString (src + "/${allowedPrefix}")))
          )
          allowedPrefixes)
      src;
  PROTOC = "${protobuf}/bin/protoc";
  PROTOC_INCLUDE = "${protobuf}/include";
  version = gitVersions.version;
  src_list = [
    "Cargo.lock"
    "Cargo.toml"
    "exporter"
    "rpc"
    "console-logger"
    "call-home"
    "upgrade"
    "dependencies/control-plane/openapi/Cargo.toml"
    "dependencies/control-plane/openapi/build.rs"
    "dependencies/control-plane/control-plane/plugin"
    "dependencies/control-plane/control-plane/rest/openapi-specs"
    "dependencies/control-plane/scripts/rust/generate-openapi-bindings.sh"
    "dependencies/control-plane/scripts/rust/branch_ancestor.sh"
    "dependencies/control-plane/common"
    "dependencies/control-plane/utils"
    "dependencies/control-plane/rpc"
    "dependencies/control-plane/k8s/forward"
    "dependencies/control-plane/k8s/operators"
    "k8s"
  ];
  src = whitelistSource ../../../. src_list;
  buildProps = rec {
    name = "extensions-${version}";
    inherit version src;
    GIT_VERSION_LONG = "${gitVersions.long}";
    GIT_VERSION = "${gitVersions.tag_or_long}";

    inherit PROTOC PROTOC_INCLUDE;
    nativeBuildInputs = [ clang pkg-config git openapi-generator which ];
    buildInputs = [ llvmPackages.libclang protobuf openssl utillinux ];
    doCheck = false;
  };
  release_build = { "release" = true; "debug" = false; };
in
let
  build_with_naersk = { buildType, cargoBuildFlags }:
    naersk.buildPackage (buildProps // {
      release = release_build.${buildType};
      cargoBuildOptions = attrs: attrs ++ cargoBuildFlags;
      preBuild = ''
        # don't run during the dependency build phase
        if [ ! -f build.rs ]; then
          patchShebangs ./dependencies/control-plane/scripts/rust/
          ./dependencies/control-plane/scripts/rust/generate-openapi-bindings.sh --skip-git-diff
        fi
      '';
      doCheck = false;
      usePureFromTOML = true;
    });
  build_with_default = { buildType, cargoBuildFlags }:
    rustPlatform.buildRustPackage (buildProps // {
      inherit buildType cargoBuildFlags;
      preBuild = "patchShebangs ./dependencies/control-plane/scripts/rust/";
      cargoLock = {
        lockFile = ../../../Cargo.lock;
      };
    });
  builder = if incremental then build_with_naersk else build_with_default;
in
{
  inherit PROTOC PROTOC_INCLUDE version src whitelistSource;

  build = { buildType, cargoBuildFlags ? [ ] }:
    if allInOne then
      builder { inherit buildType; cargoBuildFlags = [ "-p rpc" "-p exporter" "-p call-home" "-p upgrade" "-p call-home"]; }
    else
      builder { inherit buildType cargoBuildFlags; };
}
