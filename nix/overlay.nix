{ allInOne ? true, incremental ? false }:
self: super: {
  images = super.callPackage ./pkgs/images { };
  extensions = super.callPackage ./pkgs/extensions { inherit allInOne incremental; };
}
