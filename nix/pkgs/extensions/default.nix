{ stdenv, git, lib, pkgs, allInOne, incremental, tag ? "" }:
let
  versionDrv = import ../../lib/version.nix { inherit lib stdenv git tag; };
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
    upgrade = rec {
      recurseForDerivations = true;
      upgrade_builder = { buildType, builder, cargoBuildFlags ? [ "-p upgrade" ] }: builder.build { inherit buildType cargoBuildFlags; };
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
    stats = rec {
      recurseForDerivations = true;
      stats_builder = { buildType, builder, cargoBuildFlags ? [ "-p stats" ] }: builder.build { inherit buildType cargoBuildFlags; };
      stats_installer = { pname, src }: installer { inherit pname src; };
      aggregator = stats_installer {
        src =
          if allInOne then
            stats_builder { inherit buildType builder; cargoBuildFlags = [ "-p stats" ]; }
          else
            stats_builder { inherit buildType builder; cargoBuildFlags = [ "--bin stats-aggregator" ]; };
        pname = "stats-aggregator";
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
