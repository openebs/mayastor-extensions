let
  sources = import ../../nix/sources.nix;
  pkgs = import sources.nixpkgs { };
  whitelistSource = src: allowedPrefixes:
    builtins.path {
      filter = (path: type:
        pkgs.lib.any
          (allowedPrefix:
            (pkgs.lib.hasPrefix (toString (src + "/${allowedPrefix}")) path) ||
            (type == "directory" && pkgs.lib.hasPrefix path (toString (src + "/${allowedPrefix}")))
          )
          allowedPrefixes);
      path = src;
      name = "extensions";
    };
  test-src = whitelistSource ../.. [ "scripts" "chart" "nix" ];
in
pkgs.nixosTest {
  name = "k8s-helm-install";
  nodes = {
    machine = { config, pkgs, ... }: {
      virtualisation = {
        memorySize = 8192;
        diskSize = 16384;
        cores = 8;
      };
      boot = {
        kernel.sysctl = {
          "vm.nr_hugepages" = 1800;
        };
        kernelModules = [
          "nvme-tcp"
        ];
      };
      environment.systemPackages = with pkgs; [
        kubernetes-helm
        kubectl
        kind
        docker
        util-linux
        jq
        sudo
      ];
      virtualisation = {
        docker = {
          enable = true;
        };
      };
      networking.firewall.enable = false;
    };
  };

  testScript = ''
    machine.wait_for_unit("default.target")

    machine.succeed("${test-src}/scripts/k8s/deployer.sh start --label")
    machine.succeed("${test-src}/scripts/helm/install.sh --wait")
    machine.succeed("kubectl get pods -A -o wide 1>&2")
  '';
}
