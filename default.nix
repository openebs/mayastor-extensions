{ allInOne ? true, incremental ? false, img_tag ? "" }:
let
  sources = import ./nix/sources.nix;
  pkgs = import sources.nixpkgs {
    overlays = [ (_: _: { inherit sources; }) (import ./nix/overlay.nix { inherit allInOne incremental img_tag; }) ];
  };
in
pkgs
