{
  description = "ratatui-image";
  nixConfig.bash-prompt = "\[nix-develop\]$ ";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        # We only need the nightly overlay in the devShell because .rs files are formatted with nightly.
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        rust = pkgs.rust-bin.stable."1.74.0".default;
      in 
      with pkgs;
      {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "ratatui-image";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
        };
        devShell = mkShell {
          buildInputs = [
            (rust.override {
              extensions = [ "rust-src" "rust-analyzer-preview" "rustfmt" "clippy" ]; 
            })
            pkg-config
            cargo-tarpaulin
            cargo-watch
          ];
        };
      });
}
