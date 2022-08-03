{
  inputs = {
    naersk.url = "github:nix-community/naersk/master";
    nixpkgs.url = "nixpkgs";
    utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, utils, naersk }:
    utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        naersk-lib = pkgs.callPackage naersk { };
      in
      {
        packages.apicurio-sync = naersk-lib.buildPackage ./.;
        packages.default = self.packages.${system}.apicurio-sync;

        apps.default = utils.lib.mkApp {
          name = "apicurio-sync";
          drv = self.packages.${system}.default;
        };

        devShells.default = with pkgs; mkShell {
          buildInputs = [ cargo rustc rustfmt pre-commit rustPackages.clippy ];
          RUST_SRC_PATH = rustPlatform.rustLibSrc;
        };
      });
}
