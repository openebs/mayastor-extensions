{ allInOne ? true
, incremental ? false
, static ? false
, img_tag ? ""
, tag ? ""
, img_org ? ""
, product_prefix ? ""
, rustFlags ? ""
}:
let
  config = import ./config.nix;
  img_prefix = if product_prefix == "" then config.product_prefix else product_prefix;
in
self: super: {
  sourcer = super.callPackage ./lib/sourcer.nix { };
  images = super.callPackage ./pkgs/images { inherit img_tag img_org img_prefix; };
  extensions = super.callPackage ./pkgs/extensions { inherit allInOne incremental static tag rustFlags; };
  paperclip = super.callPackage ./../dependencies/control-plane/nix/pkgs/paperclip { };
  utils = super.callPackage ./pkgs/utils { inherit incremental; };
  channel = import ./lib/rust.nix { pkgs = super.pkgs; };
}
