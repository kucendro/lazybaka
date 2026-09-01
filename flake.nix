{
  description = "bakasync - public Bakalari timetable to Google Calendar sync";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forEachSystem = nixpkgs.lib.genAttrs systems;
    in
    {
      packages = forEachSystem (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          bakasync = pkgs.callPackage ./nix/package.nix { };
        in
        {
          inherit bakasync;
          default = bakasync;
        }
      );

      checks = forEachSystem (system: {
        bakasync = self.packages.${system}.bakasync;
      });

      devShells = forEachSystem (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          default = pkgs.mkShell {
            packages = [
              self.packages.${system}.bakasync
            ]
            ++ (with pkgs; [
              cargo
              rustc
              clippy
              rustfmt
              rust-analyzer
              lefthook
              nixfmt-rfc-style
              mdbook
            ]);
          };
        }
      );

      overlays.default = final: _prev: {
        bakasync = final.callPackage ./nix/package.nix { };
      };

      nixosModules.bakasync = { lib, pkgs, ... }: {
        imports = [ ./nix/module.nix ];
        services.bakasync.package = lib.mkDefault self.packages.${pkgs.stdenv.hostPlatform.system}.bakasync;
      };

      nixosModules.default = self.nixosModules.bakasync;
    };
}
