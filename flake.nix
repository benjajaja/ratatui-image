{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane.url = "github:ipetkov/crane";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = {
    self,
    nixpkgs,
    crane,
    fenix,
    flake-utils,
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
          (lib.fileset.maybeMissing ./src/snapshots)
        ];
      };

      commonArgs = {
        inherit src;
        strictDeps = true;

        buildInputs = with pkgs;
          [
            pkg-config
            cargo-watch
            cargo-semver-checks
            cargo-release
            cargo-make
            rust-analyzer
            # Add additional build inputs here
          ]
          ++ lib.optionals pkgs.stdenv.isDarwin [
            # Additional darwin specific inputs can be set here
            pkgs.libiconv
          ];

        # Additional environment variables can be set directly
        # MY_CUSTOM_VAR = "some value";
      };

      craneLibLLvmTools =
        craneLib.overrideToolchain
        (fenix.packages.${system}.complete.withComponents [
          "cargo"
          "llvm-tools"
          "rustc"
          "clippy"
        ]);

      # Build *just* the cargo dependencies, so we can reuse
      # all of that work (e.g. via cachix) when running in CI
      cargoArtifacts = craneLib.buildDepsOnly commonArgs;

      ratatui-image = craneLib.buildPackage (commonArgs
        // {
          inherit cargoArtifacts;
        });

      # Build the screenshot example as a separate package
      ratatui-image-screenshot = craneLib.buildPackage (commonArgs
        // {
          inherit cargoArtifacts;
          pname = "ratatui-image-screenshot";
          cargoExtraArgs = "--example screenshot --features crossterm";
        });

      toolchain = with fenix.packages.${system};
        combine [
          minimal.rustc
          minimal.cargo
          targets.x86_64-pc-windows-gnu.latest.rust-std
        ];
      craneLibWindows = (crane.mkLib pkgs).overrideToolchain toolchain;
      ratatui-image-windows = craneLibWindows.buildPackage {
        inherit src;

        strictDeps = true;
        doCheck = false;

        CARGO_BUILD_TARGET = "x86_64-pc-windows-gnu";

        # fixes issues related to libring
        TARGET_CC = "${pkgs.pkgsCross.mingwW64.stdenv.cc}/bin/${pkgs.pkgsCross.mingwW64.stdenv.cc.targetPrefix}cc";

        #fixes issues related to openssl
        OPENSSL_DIR = "${pkgs.openssl.dev}";
        OPENSSL_LIB_DIR = "${pkgs.openssl.out}/lib";
        OPENSSL_INCLUDE_DIR = "${pkgs.openssl.dev}/include/";

        depsBuildBuild = with pkgs; [
          pkgsCross.mingwW64.stdenv.cc
          pkgsCross.mingwW64.windows.pthreads
        ];
      };
    in {
      checks = {
        # Build the crate as part of `nix flake check` for convenience
        inherit ratatui-image;

        # NixOS test for screenshot functionality with kitty (Wayland)
        kitty-screenshot-test-wayland = pkgs.nixosTest {
          name = "ratatui-image-kitty-screenshot-wayland";

          nodes.machine = { pkgs, ... }: {
            imports = [ ];

            # Increase VM memory to handle the screenshot example
            virtualisation.memorySize = 2048;

            # Enable Wayland with sway
            programs.sway = {
              enable = true;
              wrapperFeatures.gtk = true;
            };

            services.xserver.enable = true;
            services.displayManager.sddm.enable = true;
            services.displayManager.sddm.wayland.enable = true;

            services.displayManager.autoLogin = {
              enable = true;
              user = "test";
            };

            services.displayManager.defaultSession = "sway";

            # Create test user
            users.users.test = {
              isNormalUser = true;
              extraGroups = [ "wheel" "video" ];
              packages = [ ];
            };

            # Ensure required packages are available
            environment.systemPackages = with pkgs; [
              kitty
              grim  # Wayland screenshot tool
            ];
          };

          testScript = ''
            machine.wait_for_unit("graphical.target")

            # Wait for sway to start
            machine.wait_until_succeeds("pgrep -f sway")
            machine.sleep(3)

            # Check what Wayland display is actually available
            machine.succeed("ls -la /run/user/1000/ || true")
            machine.succeed("ps aux | grep sway || true")

            # Copy the Ada.png asset to the test environment
            machine.succeed("mkdir -p /tmp/test-assets/assets")
            machine.copy_from_host("${src}/assets/Ada.png", "/tmp/test-assets/assets/Ada.png")

            # Run kitty with the main ratatui-image program
            # Use systemd-run to ensure proper environment
            machine.succeed("""
              systemd-run --uid=test --setenv=XDG_RUNTIME_DIR=/run/user/1000 \
                --setenv=WAYLAND_DISPLAY=wayland-1 \
                --working-directory=/tmp/test-assets \
                -- kitty \
                -o font_size=7 \
                -o background=#222222 \
                -o foreground=#ffffff \
                ${ratatui-image}/bin/ratatui-image assets/Ada.png &
            """)

            # Wait for kitty to appear
            machine.wait_until_succeeds("pgrep kitty")
            machine.sleep(5)

            # Take a screenshot using the machine's screenshot function
            machine.screenshot("kitty-wayland-screenshot")

            print("Wayland Screenshot test completed successfully")
            print("Screenshot saved to test output directory as kitty-wayland-screenshot.png")
          '';
        };

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
          windows = ratatui-image-windows;
          screenshot = ratatui-image-screenshot;
        }
        // lib.optionalAttrs (!pkgs.stdenv.isDarwin) {
          ratatui-image-llvm-coverage = craneLibLLvmTools.cargoLlvmCov (commonArgs
            // {
              inherit cargoArtifacts;
            });
        };

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
        ];
      };
    });
}
