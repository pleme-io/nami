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
    enableMcpBin = mkOption {
      type = types.bool;
      default = false;
      description = ''
        Install a `nami-mcp` shim on PATH that runs `nami mcp`
        (stdio transport) — useful for registering with
        blackmatter-anvil:

          blackmatter.components.anvil.mcp.servers.nami = {
            command = "''${config.home.homeDirectory}/.local/bin/nami-mcp";
            args = [ ];
            enableFor = [ "claude-code" ];
          };
      '';
    };
  };
  config = mkIf cfg.enable (mkMerge [
    {
      home.packages = [ cfg.package ];
    }
    (mkIf cfg.enableMcpBin {
      home.file.".local/bin/nami-mcp" = {
        executable = true;
        text = ''
          #!${pkgs.bash}/bin/bash
          exec ${cfg.package}/bin/nami mcp "$@"
        '';
      };
    })
  ]);
}
