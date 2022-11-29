{ allInOne ? true, incremental ? false, img_tag ? "" }:
self: super: {
  images = super.callPackage ./pkgs/images { inherit img_tag; };
  extensions = super.callPackage ./pkgs/extensions { inherit allInOne incremental; };
  openapi-generator = super.callPackage ./../dependencies/control-plane/nix/pkgs/openapi-generator { };
  utils = super.callPackage ./pkgs/utils { inherit incremental; };
}
