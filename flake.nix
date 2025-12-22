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

      # Build chafa with static library support (uses autotools)
      chafaStatic = pkgs.chafa.overrideAttrs (old: {
        configureFlags = (old.configureFlags or []) ++ [
          "--enable-static"
          "--enable-shared"
        ];
      });

      # We also need static glib for full static linking (uses meson)
      glibStatic = pkgs.glib.overrideAttrs (old: {
        mesonFlags = (old.mesonFlags or []) ++ [
          "-Ddefault_library=both"
        ];
      });

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
          nativeBuildInputs = with pkgs; [
            makeWrapper
            pkg-config
            llvmPackages.libclang
          ];
          buildInputs = with pkgs; [
            chafaStatic
            chafaStatic.dev
            glibStatic.dev
            libsysprof-capture
            pcre2.dev
            libffi.dev
            zlib.dev
          ];
          cargoExtraArgs = "--features chafa-static";
          # Environment for pkg-config and bindgen
          PKG_CONFIG_PATH = "${chafaStatic.dev}/lib/pkgconfig:${glibStatic.dev}/lib/pkgconfig:${pkgs.libsysprof-capture}/lib/pkgconfig:${pkgs.pcre2.dev}/lib/pkgconfig:${pkgs.libffi.dev}/lib/pkgconfig:${pkgs.zlib.dev}/lib/pkgconfig";
          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
          BINDGEN_EXTRA_CLANG_ARGS = "-isystem ${pkgs.glibc.dev}/include";
        });

      ratatui-demo = craneLib.buildPackage (commonArgs
        // {
          inherit cargoArtifacts;
          pname = "demo";
          nativeBuildInputs = [ pkgs.makeWrapper ];
          buildInputs = [ pkgs.chafa ];
          cargoExtraArgs = "--example demo --features crossterm,chafa-libload";
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
          chafaStatic       # for chafa-static feature (has libchafa.a)
          chafaStatic.dev
          glibStatic.dev    # required by chafa.pc (has libglib-2.0.a)
          # Dependencies needed for static linking
          libsysprof-capture
          pcre2.dev
          libffi.dev
          zlib.dev
          pkg-config
          llvmPackages.libclang
        ];
        LD_LIBRARY_PATH = lib.makeLibraryPath [ pkgs.chafa ];
        # For chafa-static feature, bindgen needs LIBCLANG_PATH and C headers
        LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
        BINDGEN_EXTRA_CLANG_ARGS = "-isystem ${pkgs.glibc.dev}/include";
        # Ensure chafaStatic is found first for static linking
        PKG_CONFIG_PATH = "${chafaStatic.dev}/lib/pkgconfig:${glibStatic.dev}/lib/pkgconfig";
      };
    });
}
