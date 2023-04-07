{ stdenv, git, lib, pkgs, allInOne, incremental }:
let
  versionDrv = import ../../lib/version.nix { inherit lib stdenv git; };
  version = builtins.readFile "${versionDrv}";
  gitVersions = {
    "version" = version;
    "long" = builtins.readFile "${versionDrv.long}";
    "tag_or_long" = builtins.readFile "${versionDrv.tag_or_long}";
  };
  project-builder =
    pkgs.callPackage ../extensions/cargo-project.nix { inherit gitVersions allInOne incremental; };
  installer = { pname, src, suffix ? "" }:
    stdenv.mkDerivation rec {
      inherit pname src;
      name = "${pname}-${version}";
      binary = "${pname}${suffix}";
      installPhase = ''
        mkdir -p $out/bin
        cp $src/bin/${pname} $out/bin/${binary}
      '';
    };

  components = { buildType, builder }: rec {
    exporters = {
      metrics = rec {
        recurseForDerivations = true;
        metrics_builder = { buildType, builder, cargoBuildFlags ? [ "-p exporter" ] }: builder.build { inherit buildType cargoBuildFlags; };
        metrics_installer = { pname, src }: installer { inherit pname src; };
        pool = metrics_installer {
          src =
            if allInOne then
              metrics_builder { inherit buildType builder; }
            else
              metrics_builder { inherit buildType builder; cargoBuildFlags = [ "--bin metrics-exporter-pool" ]; };
          pname = "metrics-exporter-pool";
        };
      };
    };
    operators = rec {
      recurseForDerivations = true;
      upgrade_operator_builder = { buildType, builder, cargoBuildFlags ? [ "-p operator-upgrade" ] }: builder.build { inherit buildType cargoBuildFlags; };
      operator_installer = { pname, src }: installer { inherit pname src; };
      upgrade = operator_installer {
        src =
          if allInOne then
            upgrade_operator_builder { inherit buildType builder; }
          else
            upgrade_operator_builder { inherit buildType builder; cargoBuildFlags = [ "--bin operator-upgrade" ]; };
        pname = "operator-upgrade";
      };
    };
    upgrade = rec {
      recurseForDerivations = true;
      upgrade_builder = { buildType, builder, cargoBuildFlags ? [ "-p upgrade-job" ] }: builder.build { inherit buildType cargoBuildFlags; };
      upgrade_installer = { pname, src }: installer { inherit pname src; };
      job = upgrade_installer {
        src =
          if allInOne then
            upgrade_builder { inherit buildType builder; }
          else
            upgrade_builder { inherit buildType builder; cargoBuildFlags = [ "--bin upgrade-job" ]; };
        pname = "upgrade-job";
      };
    };
    obs = rec {
      recurseForDerivations = true;
      obs_builder = { buildType, builder, cargoBuildFlags ? [ "-p call-home" ] }: builder.build { inherit buildType cargoBuildFlags; };
      obs_installer = { pname, src }: installer { inherit pname src; };
      callhome = obs_installer {
        src =
          if allInOne then
            obs_builder { inherit buildType builder; cargoBuildFlags = [ "-p call-home" ]; }
          else
            obs_builder { inherit buildType builder; cargoBuildFlags = [ "--bin obs-callhome" ]; };
        pname = "obs-callhome";
      };
    };
  };
in
{
  PROTOC = project-builder.PROTOC;
  PROTOC_INCLUDE = project-builder.PROTOC_INCLUDE;
  inherit version gitVersions project-builder;

  release = components { builder = project-builder; buildType = "release"; };
  debug = components { builder = project-builder; buildType = "debug"; };
}