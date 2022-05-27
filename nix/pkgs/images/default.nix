# It would be cool to produce OCI images instead of docker images to
# avoid dependency on docker tool chain. Though the maturity of OCI
# builder in nixpkgs is questionable which is why we postpone this step.

{ dockerTools, lib, extensions }:
let
  image_suffix = { "release" = ""; "debug" = "-debug"; "coverage" = "-coverage"; };
  build-extensions-image = { pname, buildType, package, config ? { } }:
    dockerTools.buildImage {
      tag = extensions.version;
      created = "now";
      name = "mayadata/mayastor-${pname}${image_suffix.${buildType}}";
      contents = [ package ];
      config = {
        Entrypoint = [ package.binary ];
        ExposedPorts = {
          "9052/tcp" = { };
        };
      } // config;
    };
in
let
  build-images = { buildType }: {
    exporters = {
      recurseForDerivations = true;
      metrics = {
        recurseForDerivations = true;
        pool = build-extensions-image rec {
          inherit buildType;
          package = extensions.${buildType}.exporters.metrics.pool;
          pname = package.pname;
        };
      };
    };
  };
in
{
  release = build-images { buildType = "release"; };
  debug = build-images { buildType = "debug"; };
}
