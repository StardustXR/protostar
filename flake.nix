{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    crane = {
      inputs.nixpkgs.follows = "nixpkgs";
      url = "github:ipetkov/crane";
    };
  };


  outputs = { self, nixpkgs, crane }:
  let supportedSystems = [ "aarch64-linux" "x86_64-linux" ];
      forAllSystems = nixpkgs.lib.genAttrs supportedSystems;
      nixpkgsFor = forAllSystems (system: import nixpkgs { inherit system; });
  in {
    packages = forAllSystems (system: let pkgs = nixpkgsFor.${system}; in {
      default = crane.lib.${system}.buildPackage {
        pname = "protostar";
        version = "0.1.0";
        src = ./.;
        
        cargoTestCommand = "echo \"looks good to me\"";
        
        STARDUST_RES_PREFIXES = pkgs.stdenvNoCC.mkDerivation {
          name = "resources";
          src = ./.;
  
          buildPhase = "cp -r $src/res $out";
        };
        
        # for tests
        nativeBuildInputs = with pkgs; [
          ashpd-demo
        ];
        XDG_CACHE_HOME = "/tmp";
      };
    });
  };
}
