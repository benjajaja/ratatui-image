{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane.url = "github:ipetkov/crane";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    self,
    nixpkgs,
    crane,
    flake-utils,
    rust-overlay,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = nixpkgs.legacyPackages.${system};
      inherit (pkgs) lib;

      craneLib = crane.mkLib pkgs;

      unfilteredRoot = ./.;
      src = lib.fileset.toSource {
        root = unfilteredRoot;
        fileset = lib.fileset.unions [
          # Default files from crane (Rust and cargo files)
          (craneLib.fileset.commonCargoSources unfilteredRoot)
          (lib.fileset.maybeMissing ./assets)
          (lib.fileset.fileFilter (file: file.hasExt "snap") ./src)
        ];
      };

      commonArgs = {
        inherit src;
        strictDeps = true;

        buildInputs = with pkgs;
          [
            cargo-watch
            cargo-semver-checks
            cargo-release
            cargo-make
            rust-analyzer
          ]
          ++ lib.optionals pkgs.stdenv.isDarwin [
            # Additional darwin specific inputs can be set here
            pkgs.libiconv
          ];

        # Additional environment variables can be set directly
        # MY_CUSTOM_VAR = "some value";
      };

      # Build *just* the cargo dependencies, so we can reuse
      # all of that work (e.g. via cachix) when running in CI
      cargoArtifacts = craneLib.buildDepsOnly commonArgs;

      ratatui-image = craneLib.buildPackage (commonArgs
        // {
          inherit cargoArtifacts;
          nativeBuildInputs = [ pkgs.makeWrapper ];
          buildInputs = [ pkgs.chafa ];
          cargoExtraArgs = "--features chafa";
          LD_LIBRARY_PATH = lib.makeLibraryPath [ pkgs.chafa ]; # for tests
        });

      ratatui-demo = craneLib.buildPackage (commonArgs
        // {
          inherit cargoArtifacts;
          pname = "demo";
          nativeBuildInputs = [ pkgs.makeWrapper ];
          buildInputs = [ pkgs.chafa ];
          cargoExtraArgs = "--example demo --features crossterm,chafa";
          LD_LIBRARY_PATH = lib.makeLibraryPath [ pkgs.chafa ]; # for tests
        });

      screenshotTests = import ./screenshot-tests.nix { inherit pkgs src self system; };

      # Cross-compile to Windows
      pkgsWindows = import nixpkgs {
        overlays = [ (import rust-overlay) ];
        localSystem = system;
        crossSystem = {
          config = "x86_64-w64-mingw32";
          libc = "msvcrt";
        };
      };
      craneLibWindows = (crane.mkLib pkgsWindows).overrideToolchain (p:
        p.rust-bin.stable.latest.default.override {
          targets = [ "x86_64-pc-windows-gnu" ];
        }
      );
      ratatui-image-windows = craneLibWindows.buildPackage {
        src = craneLibWindows.cleanCargoSource ./.;
        strictDeps = true;
        doCheck = false;
      };

    in {
      checks = {
        inherit ratatui-image;

        # Run clippy (and deny all warnings) on the crate source,
        # again, reusing the dependency artifacts from above.
        #
        # Note that this is done as a separate derivation so that
        # we can block the CI if there are issues here, but not
        # prevent downstream consumers from building our crate by itself.
        ratatui-image-clippy = craneLib.cargoClippy (commonArgs
          // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--all-targets --features crossterm -- --deny warnings";
          });

        ratatui-image-doc = craneLib.cargoDoc (commonArgs
          // {
            inherit cargoArtifacts;
          });

        # Check formatting
        ratatui-image-fmt = craneLib.cargoFmt {
          inherit src;
        };

        # Run tests with cargo-nextest
        # Consider setting `doCheck = false` on `ratatui-image` if you do not want
        # the tests to run twice
        ratatui-image-nextest = craneLib.cargoNextest (commonArgs
          // {
            inherit cargoArtifacts;
            partitions = 1;
            partitionType = "count";
          });
      };

      packages =
        {
          default = ratatui-image;
          demo = ratatui-demo;
          windows = ratatui-image-windows;
        }
        // screenshotTests;

      apps.default = flake-utils.lib.mkApp {
        drv = ratatui-image;
      };

      devShells.default = craneLib.devShell {
        # Inherit inputs from checks.
        checks = self.checks.${system};

        # Additional dev-shell environment variables can be set directly
        # MY_CUSTOM_DEVELOPMENT_VAR = "something else";

        # Extra inputs can be added here; cargo and rustc are provided by default.
        packages = with pkgs; [
          cargo-release
          cargo-insta
          chafa
          pkg-config
          llvmPackages.libclang
        ];
        LD_LIBRARY_PATH = lib.makeLibraryPath [ pkgs.chafa ];
      };
    });
}
