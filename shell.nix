{
  pkgs ? (import (builtins.fetchTarball {
           url = "https://github.com/nixos/nixpkgs/tarball/25.11";
           sha256 = "1zn1lsafn62sz6azx6j735fh4vwwghj8cc9x91g5sx2nrg23ap9k";
         }) {})
}:
let
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
pkgs.mkShell {
  buildInputs = with pkgs; [
    # APK acquisition
    apkeep
    # Extraction & analysis
    unzip file tree findutils binwalk binutils hexdump p7zip
    # Android tools
    android-tools apktool
    # Decompilation
    jadx jdk radare2
    # Text processing
    ripgrep jq xmlstarlet
    # Rust toolchain
    rustc cargo clippy rustfmt rust-analyzer pkg-config openssl.dev
    # Project management
    just git curl wget
    # Documentation
    pandoc graphviz
    # Python (for custom scripts)
    python3 python3Packages.requests python3Packages.lxml
  ];
  shellHook = ''
    export JADX_OPTS="-Xmx4g"
    export JAVA_OPTS="-Xmx4g"
    mkdir -p extracted analysis decompiled reports secrets
  '';
}
