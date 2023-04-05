# It would be cool to produce OCI images instead of docker images to
# avoid dependency on docker tool chain. Though the maturity of OCI
# builder in nixpkgs is questionable which is why we postpone this step.

{ dockerTools, lib, extensions, busybox, gnupg, kubernetes-helm-wrapped, runCommand, img_tag ? "" }:
let
  helm_chart_src = builtins.filterSource (path: type: true) ../../../chart;
  image_suffix = { "release" = ""; "debug" = "-debug"; "coverage" = "-coverage"; };
  build-extensions-image = { pname, buildType, package, extraCommands ? '''', contents ? [ ], config ? { } }:
    dockerTools.buildImage {
      inherit extraCommands;
      tag = if img_tag != "" then img_tag else extensions.version;
      created = "now";
      name = "openebs/mayastor-${pname}${image_suffix.${buildType}}";
      contents = [ package ] ++ contents;
      config = {
        Entrypoint = [ package.binary ];
      } // config;
    };
  build-exporter-image = { buildType }: {
    pool = build-extensions-image rec{
      inherit buildType;
      package = extensions.${buildType}.exporters.metrics.pool;
      pname = package.pname;
      config = {
        ExposedPorts = {
          "9052/tcp" = { };
        };
      };
    };
  };
  build-upgrade-operator-image = { buildType }:
    build-extensions-image rec{
      inherit buildType;
      package = extensions.${buildType}.operators.upgrade;
      contents = [ helm_chart_src kubernetes-helm-wrapped busybox ];
      extraCommands = ''
        mkdir -p chart && cp -drf --preserve=mode ${helm_chart_src}/* chart/
      '';
      pname = package.pname;
      config = {
        ExposedPorts = {
          "8080/tcp" = { };
        };
        Env = [ "CHART_DIR=/chart" ];
      };
    };
  build-upgrade-image = { buildType, name }:
    build-extensions-image rec{
      inherit buildType;
      package = extensions.${buildType}.upgrade.${name};
      contents = [ helm_chart_src kubernetes-helm-wrapped busybox ];
      extraCommands = ''
        mkdir -p chart && cp -drf --preserve=mode ${helm_chart_src}/* chart/
      '';
      pname = package.pname;
      config = {
        Env = [ "CORE_CHART_DIR=/chart" ];
    };
  };
  build-obs-callhome-image = { buildType }:
    build-extensions-image rec{
      inherit buildType;
      package = extensions.${buildType}.obs.callhome;
      contents = [ ./../../../call-home/assets busybox gnupg ];
      extraCommands = ''
        mkdir -p encryption_dir
      '';
      pname = package.pname;
      config = {
        Env = [ "KEY_FILEPATH=/key/public.gpg" "ENCRYPTION_DIR=/encryption_dir" ];
      };
    };

in
let
  build-exporter-images = { buildType }: {
    metrics = build-exporter-image {
      inherit buildType;
    };
  };
  build-upgrade-operator-images = { buildType }: {
    upgrade = build-upgrade-operator-image {
      inherit buildType;
    };
  };
  build-upgrade-images = { buildType }: {
    job = build-upgrade-image {
      inherit buildType;
      name = "job";
    };
  };
  build-obs-images = { buildType }: {
    callhome = build-obs-callhome-image {
      inherit buildType;
    };
  };
in
let
  build-images = { buildType }: {
    exporters = build-exporter-images { inherit buildType; } // {
      recurseForDerivations = true;
    };
    operators = build-upgrade-operator-images { inherit buildType; } // {
      recurseForDerivations = true;
    };
    upgrade = build-upgrade-images { inherit buildType; } // {
      recurseForDerivations = true;
    };
    obs = build-obs-images { inherit buildType; } // {
      recurseForDerivations = true;
    };
  };
in
{
  release = build-images { buildType = "release"; };
  debug = build-images { buildType = "debug"; };
}
