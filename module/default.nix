# Nami home-manager module — GPU-rendered TUI browser
#
# Namespace: programs.nami
#
# Module factory: receives { hmHelpers } from flake.nix, returns HM module.
{ hmHelpers }:
{
  lib,
  config,
  pkgs,
  ...
}:
with lib;
let
  cfg = config.programs.nami;
in
{
  options.programs.nami = {
    enable = mkOption {
      type = types.bool;
      default = false;
      description = "Enable the nami TUI browser.";
    };
    package = mkOption {
      type = types.package;
      default = pkgs.nami;
      description = "The nami package to install.";
    };
  };
  config = mkIf cfg.enable {
    home.packages = [ cfg.package ];
  };
}
