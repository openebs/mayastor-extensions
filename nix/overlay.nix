{ allInOne ? true, incremental ? false }:
self: super: {
  images = super.callPackage ./pkgs/images { };
  extensions = super.callPackage ./pkgs/extensions { inherit allInOne incremental; };
  openapi-generator = super.callPackage ./../call-home/mayastor-control-plane/nix/pkgs/openapi-generator { };
}
