{
  description = "rt: run the right task runner for the project";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
        version = cargoToml.package.version;
      in
      {
        packages = rec {
          rt = pkgs.rustPlatform.buildRustPackage {
            pname = "rt";
            inherit version;
            src = ./.;
            cargoLock = {
              lockFile = ./Cargo.lock;
            };
            cargoHash = pkgs.lib.fakeHash;
            meta = with pkgs.lib; {
              description = "rt is a CLI tools for running task files correctly.";
              license = licenses.mit;
              homepage = "https://github.com/unvalley/rt";
              mainProgram = "rt";
            };
          };
          default = rt;
        };

        apps.default = flake-utils.lib.mkApp {
          drv = self.packages.${system}.rt;
        };

        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            cargo
            rustc
          ];
        };
      });
}
