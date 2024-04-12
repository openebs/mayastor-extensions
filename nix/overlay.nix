{ allInOne ? true, incremental ? false, static ? false, img_tag ? "", tag ? "", img_org ? "" }:
self: super: {
  sourcer = super.callPackage ./lib/sourcer.nix { };
  images = super.callPackage ./pkgs/images { inherit img_tag img_org; };
  extensions = super.callPackage ./pkgs/extensions { inherit allInOne incremental static tag; };
  openapi-generator = super.callPackage ./../dependencies/control-plane/nix/pkgs/openapi-generator { };
  utils = super.callPackage ./pkgs/utils { inherit incremental; };
  channel = import ./lib/rust.nix { pkgs = super.pkgs; };
}
