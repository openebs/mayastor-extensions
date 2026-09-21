{ pkgs }:
let
  lib = pkgs.lib;
  sources = import ../sources.nix;
in
rec {
  makeRustTarget = platform: platform.rust.rustcTargetSpec;
  naersk_package = channel: pkgs.callPackage sources.naersk {
    rustc = channel.stable;
    cargo = channel.stable;
    # Rewrite crates.io API URLs to static CDN to avoid intermittent 403s
    fetchurl = attrs@{ url, ... }:
      let
        m = builtins.match "https://crates\\.io/api/v1/crates/([^/]+)/([^/]+)/download" url;
        resolvedUrl =
          if m != null then
            "https://static.crates.io/crates/${builtins.elemAt m 0}/${builtins.elemAt m 0}-${builtins.elemAt m 1}.crate"
          else url;
      in
      pkgs.fetchurl (attrs // { url = resolvedUrl; });
  };
  rust_default = { override ? { } }: rec {
    nightly_pkg = pkgs.rust-bin.nightly."2026-07-16";
    stable_pkg = pkgs.rust-bin.stable."1.97.1";

    nightly = nightly_pkg.default.override (override);
    stable = stable_pkg.default.override (override);

    nightly_src = nightly_pkg.rust-src;
    release_src = stable_pkg.rust-src;
  };
  default = rust_default { };
  default_src = rust_default {
    override = { extensions = [ "rust-src" ]; };
  };
  static-arch = { target }: rust_default {
    override = { targets = [ "${target}" ]; };
  };

  rustPlatformDeps = { target, sources }: rec {
    os = platform: builtins.replaceStrings [ "${platform.qemuArch}-" ] [ "" ] platform.system;
    hostPlatform = "${makeRustTarget pkgs.pkgsStatic.hostPlatform}";
    targetPlatform = "${makeRustTarget pkgs.pkgsCross."${target}".hostPlatform}";
    pkgsTarget = if hostPlatform == targetPlatform then pkgs else pkgs.pkgsCross."${target}";
    pkgsTargetNative = if hostPlatform == targetPlatform then pkgs else if hostOs == targetOs then
      import sources.nixpkgs
        {
          config = { };
          overlays = [ ];
          system = "${pkgsTarget.system}";
        } else pkgs.pkgsCross."${target}";
    hostOs = os pkgs.hostPlatform;
    targetOs = os pkgs.pkgsCross."${target}".hostPlatform;
    naersk = naersk_package (static-arch {
      target = targetPlatform;
    });
    targetUpper = lib.toUpper (
      builtins.replaceStrings [ "-" ] [ "_" ] targetPlatform
    );
    check_assert =
      if (targetOs == "darwin") then
        if hostOs == "darwin" && hostPlatform != targetPlatform
        # maybe can be achieved used unstable-pkgs until the fixes drop on the stable channel/release.
        then lib.asserts.assertMsg (false) "Cross-compiling on darwin not supported yet"
        else lib.asserts.assertMsg (pkgs.hostPlatform.isDarwin) "${targetOs} binaries can only be built on darwin (ie not ${hostOs})"
      else lib.asserts.assertMsg (pkgs.hostPlatform.isLinux) "${targetOs} binaries can only be built on linux (ie not ${hostOs})";
  };
  rustBuilderOpts = { rustPlatformDeps }: rustPlatformDeps // {
    preBuild = lib.optionalString (rustPlatformDeps.pkgsTarget.hostPlatform.isWindows) ''
      # The workspace commits resolver = "1" (see Cargo.toml).
      # Windows must build under the v2 feature resolver so the FIPS crate's
      # `cfg(not(target_os = "windows"))` gate is honoured and aws-lc-fips-sys
      # (MSVC/vcvarsall-only, can't cross-compile) is dropped. Flip it here, in
      # preBuild, which naersk runs in both the dependency and main build phases
      # so it lands before cargo resolves features.
      sed -i -E 's/^resolver = "1"/resolver = "2"/' Cargo.toml
      export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS="-C link-args=''$(echo $NIX_LDFLAGS | tr ' ' '\n' | grep -- '^-L' | tr '\n' ' ')"
      export NIX_LDFLAGS=
      export NIX_LDFLAGS_FOR_BUILD=
    '';
    addPreBuild = "";
    nativeBuildInputs = with pkgs;
      [ pkg-config protobuf paperclip which git ] ++
        [ rustPlatformDeps.pkgsTarget.stdenv.cc ] ++
        # cmake/go/perl are only for aws-lc-fips-sys, which we build on every
        # target except Windows (see the fips crate). The non-FIPS aws-lc-sys
        # used on Windows builds with cc-rs and shipped pregenerated bindings,
        # so it needs none of them - just nasm for its assembly.
        lib.optionals (!rustPlatformDeps.pkgsTarget.hostPlatform.isWindows) [ cmake go perl ] ++
        lib.optional (rustPlatformDeps.pkgsTarget.hostPlatform.isWindows) pkgs.nasm ++
        lib.optionals (rustPlatformDeps.pkgsTarget.hostPlatform.isDarwin) (with pkgs; [
          # aws-lc-fips-sys re-signs libcrypto.dylib on aarch64-darwin with a
          # bare `codesign -s -` after injecting the FIPS integrity hash, on top
          # of the ad-hoc signature the linker already applied. nixpkgs' sigtool
          # codesign aborts on an already-signed file unless -f is passed, where
          # Apple's overwrites silently - so shim it to always force. Listed
          # first so it wins on PATH over sigtool's own codesign. x86_64-darwin
          # never hits this: aws-lc's codesign step is gated to arm64.
          (writeShellScriptBin "codesign" ''exec ${darwin.sigtool}/bin/codesign -f "$@"'')
        ]) ++
        lib.optionals (rustPlatformDeps.pkgsTarget.hostPlatform.isDarwin) (with pkgs.darwin; [ autoSignDarwinBinariesHook sigtool ]);
    dontUseCmakeConfigure = true;
    addNativeBuildInputs = [ ];
    buildInputs = if (rustPlatformDeps.pkgsTarget.hostPlatform.isWindows) then with rustPlatformDeps.pkgsTargetNative.windows; [ mingw_w64_pthreads pthreads ] else [ ];
  };
  # cargo-auditable records the crates a binary was built from in the binary
  # itself, which is what an SBOM scan of it reads back. nixpkgs does this by
  # default in buildRustPackage, as used for the container images, but naersk
  # has no such thing, so the build command asks for it here. Only the release
  # binaries are published, and so scanned, so only those pay for it.
  auditableBuild = release: lib.optionalAttrs release {
    # naersk's default is "cargo $cargo_options build ...", so this only slips
    # the subcommand in front of it.
    cargoBuild = default: "cargo auditable" + lib.removePrefix "cargo" default;
  };
  rustPackageBuilder = { rustBuildOpts, name, src, release, version, singleStep, GIT_VERSION, GIT_VERSION_LONG }: rustBuildOpts.naersk.buildPackage (auditableBuild release // {
    inherit name release src version singleStep GIT_VERSION_LONG GIT_VERSION;

    preBuild = rustBuildOpts.preBuild + rustBuildOpts.addPreBuild;
    cargoBuildOptions = attrs: attrs ++ rustBuildOpts.buildOptions;
    nativeBuildInputs = rustBuildOpts.nativeBuildInputs ++ rustBuildOpts.addNativeBuildInputs
      ++ lib.optional release pkgs.cargo-auditable;
    buildInputs = rustBuildOpts.buildInputs;

    doCheck = false;
    check_assert = rustBuildOpts.check_assert;
    usePureFromTOML = true;
    CARGO_BUILD_RUSTFLAGS = "-C target-feature=+crt-static";
    CARGO_BUILD_TARGET = rustBuildOpts.targetPlatform;
    "CARGO_TARGET_${rustBuildOpts.targetUpper}_LINKER" = with rustBuildOpts.pkgsTarget.stdenv;
      if (rustBuildOpts.check_assert) then "${cc}/bin/${cc.targetPrefix}cc" else null;
    ${if pkgs.hostPlatform.isDarwin then null else "CC_${builtins.replaceStrings [ "-" ] [ "_" ] rustBuildOpts.hostPlatform}"} = "${pkgs.musl.dev}/bin/musl-gcc";
    #${if pkgs.hostPlatform.isDarwin then "LIBCLANG_PATH" else null} = "${rustBuildOpts.pkgsTarget.llvmPackages.libclang.lib}/lib";
  });
}
