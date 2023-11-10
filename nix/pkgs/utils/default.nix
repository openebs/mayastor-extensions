{ git, lib, stdenv, openapi-generator, pkgs, which, sources, llvmPackages, protobuf, extensions, incremental, channel }:
let
  src = extensions.project-builder.src;
  version = extensions.version;
  GIT_VERSION_LONG = extensions.gitVersions.long;
  GIT_VERSION = extensions.gitVersions.tag_or_long;
  singleStep = !incremental;
  preBuildOpenApi = ''
    # don't run during the dependency build phase
    if [ ! -f build.rs ]; then
      patchShebangs ./dependencies/control-plane/scripts/rust/
      ./dependencies/control-plane/scripts/rust/generate-openapi-bindings.sh --skip-git-diff
    fi
  '';
  buildKubectlPlugin = { target, release, addBuildOptions ? [ ] }:
    let
      platformDeps = channel.rustPlatformDeps { inherit target sources; };
      # required for darwin because its pkgsStatic is not static!
      static_ssl = (platformDeps.pkgsTarget.pkgsStatic.openssl.override {
        static = true;
      });
      rustBuildOpts = channel.rustBuilderOpts { rustPlatformDeps = platformDeps; } // {
        buildOptions = [ "-p" "kubectl-plugin" ] ++ addBuildOptions;
        ${if !pkgs.hostPlatform.isDarwin then "addNativeBuildInputs" else null} = [ platformDeps.pkgsTargetNative.pkgsStatic.openssl.dev ];
        addPreBuild = preBuildOpenApi + ''
          export OPENSSL_STATIC=1
        '' + lib.optionalString (pkgs.hostPlatform.isDarwin) ''
          export OPENSSL_LIB_DIR=${static_ssl.out}/lib
          export OPENSSL_INCLUDE_DIR=${static_ssl.dev}/include
        '';
      };
      name = "kubectl-plugin";
    in
    channel.rustPackageBuilder {
      inherit name release src version singleStep GIT_VERSION_LONG GIT_VERSION rustBuildOpts;
    };

  components = { release ? false }: {
    windows-gnu = rec {
      kubectl-plugin = buildKubectlPlugin {
        inherit release;
        target = "mingwW64";
        addBuildOptions = [ "--no-default-features" "--features" "tls" ];
      };
    };
    linux-musl = rec {
      kubectl-plugin = buildKubectlPlugin {
        inherit release;
        target = "musl64";
      };
    };
    apple-darwin = rec {
      kubectl-plugin = buildKubectlPlugin {
        inherit release;
        target = "x86_64-darwin";
      };
    };
  };
in
{
  inherit version;

  release = components { release = true; };
  debug = components { release = false; };
}
