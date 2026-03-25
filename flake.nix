{
  description = "linkedin-rs — LinkedIn CLI and API client";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.11";
    flake-compat = {
      url = "github:edolstra/flake-compat";
      flake = false;
    };
  };

  outputs = { self, nixpkgs, ... }:
    let
      supportedSystems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      forAllSystems = nixpkgs.lib.genAttrs supportedSystems;
    in
    {
      packages = forAllSystems (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};

          linkedin-cli = pkgs.rustPlatform.buildRustPackage {
            pname = "linkedin-cli";
            version = "0.1.0";

            src = pkgs.lib.cleanSourceWith {
              src = ./.;
              filter = path: type:
                let baseName = baseNameOf path; in
                (pkgs.lib.hasPrefix (toString ./linkedin) path) ||
                baseName == "Cargo.toml" ||
                baseName == "Cargo.lock";
            };

            cargoRoot = "linkedin";
            buildAndTestSubdir = "linkedin";
            cargoBuildFlags = [ "-p" "linkedin-cli" ];
            cargoTestFlags = [ "-p" "linkedin-cli" "-p" "linkedin-api" ];

            cargoHash = "sha256-uA2lTcWnpiSuFi8yD2zMHP9c0G6gCMnNqmVjrwQTWvU=";

            nativeBuildInputs = [ pkgs.pkg-config ];
            nativeCheckInputs = [ pkgs.cacert ];
            buildInputs = pkgs.lib.optionals pkgs.stdenv.hostPlatform.isDarwin [
              pkgs.darwin.apple_sdk.frameworks.Security
              pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
            ];
          };
        in
        {
          linkedin-cli = linkedin-cli;
          default = linkedin-cli;
        }
      );

      devShells = forAllSystems (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};

          apkeep = pkgs.rustPlatform.buildRustPackage rec {
            pname = "apkeep";
            version = "0.18.0";
            src = pkgs.fetchFromGitHub {
              owner = "EFForg";
              repo = "apkeep";
              rev = version;
              hash = "sha256-wOpPyO2TULHoNZLfYgjwR9wbIyBQPIFxLsDMp7am8AM=";
            };
            cargoHash = "sha256-PTuhD73R0AxykkVeFEHaVnXrOTHJoRl0CxBJmeh3WgQ=";
            nativeBuildInputs = [ pkgs.pkg-config ];
            buildInputs = [ pkgs.openssl.dev ];
          };
        in
        {
          default = pkgs.mkShell {
            buildInputs = with pkgs; [
              apkeep
              unzip file tree findutils binwalk binutils hexdump p7zip
              android-tools apktool
              jadx jdk radare2
              ripgrep jq xmlstarlet
              rustc cargo clippy rustfmt rust-analyzer pkg-config openssl.dev
              just git curl wget
              pandoc graphviz
              python3 python3Packages.requests python3Packages.lxml
            ];
            shellHook = ''
              export JADX_OPTS="-Xmx4g"
              export JAVA_OPTS="-Xmx4g"
              mkdir -p extracted analysis decompiled reports secrets
            '';
          };
        }
      );
    };
}
