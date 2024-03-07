{
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-23.05";

  inputs.fenix.url = "github:nix-community/fenix";
  inputs.fenix.inputs.nixpkgs.follows = "nixpkgs";

  outputs = { self, nixpkgs, fenix }:
    let
      name = "protostar";
      pkgs = system: import nixpkgs { inherit system; };
      shell = pkgs:
        pkgs.mkShell { inputsFrom = [ self.packages.${pkgs.system}.default ]; };
      package = pkgs:
        let toolchain = fenix.packages.${pkgs.system}.minimal.toolchain;
        in (pkgs.makeRustPlatform {
          cargo = toolchain;
          rustc = toolchain;
        }).buildRustPackage rec {
          pname = name;
          src = ./.;

          # ---- START package specific settings ----
          version = "0.8.0";
          cargoLock = {
            lockFile = ./Cargo.lock;
            outputHashes = {
              "stardust-xr-0.14.1" =
                "sha256-AZRVXa0mIrNSlH3tTnUarU7RghyPiK6PsKruk3cZsjk=";
              "stardust-xr-molecules-0.29.0" =
                "sha256-RzLvTQOG9fE3fhM17RtodzsLmeFF1qDjJH0lz9+tPEo=";
            };
          };

          # TODO: nothing renders, needs to use res dirs for everything
          STARDUST_RES_PREFIXES = ./hexagon_launcher/res;

          buildInputs = with pkgs; [ libxkbcommon xorg.libxcb ];
          checkFlags = [
            # depends on system env
            "--skip=xdg::test_get_desktop_files"
            "--skip=xdg::test_get_icon_path"
            "--skip=xdg::test_render_svg_to_png"
          ];
          # ---- END package specific settings ----
        };
    in {
      overlays.default = final: prev: {
        stardust-xr = (prev.stardust-xr or { }) // { ${name} = package final; };
      };

      packages."x86_64-linux".default = package (pkgs "x86_64-linux");
      packages."aarch64-linux".default = package (pkgs "aarch64-linux");

      devShells."x86_64-linux".default = shell (pkgs "x86_64-linux");
      devShells."aarch64-linux".default = shell (pkgs "aarch64-linux");
    };
}
