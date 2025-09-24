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

        # NixOS test for screenshot functionality with xterm (X11)
        xterm-screenshot-test-x11 = pkgs.nixosTest {
          name = "ratatui-image-xterm-screenshot-x11";

          nodes.machine = { pkgs, ... }: {
            imports = [ ];

            # Enable X11 for xterm
            services.xserver = {
              enable = true;
              displayManager.lightdm.enable = true;
              desktopManager.xfce.enable = true;
            };

            services.displayManager.autoLogin = {
              enable = true;
              user = "test";
            };

            # Create test user
            users.users.test = {
              isNormalUser = true;
              extraGroups = [ "wheel" ];
              packages = [ ];
            };

            # Ensure required packages are available
            environment.systemPackages = with pkgs; [
              xterm
              imagemagick  # for convert command
              xorg.xwd     # for xwd command
            ];
          };

          testScript = ''
            machine.wait_for_x()
            machine.wait_for_unit("graphical.target")

            # Copy the Ada.png asset to the test environment
            machine.succeed("mkdir -p /tmp/test-assets/assets")
            machine.copy_from_host("${src}/assets/Ada.png", "/tmp/test-assets/assets/Ada.png")

            # Run xterm with the screenshot example
            machine.succeed("""
              cd /tmp/test-assets && \
              DISPLAY=:0 xterm -ti vt340 -fa DejaVu -fs 7 -bg black -fg white \
                -e '${ratatui-image-screenshot}/bin/screenshot' &
            """)

            # Wait for the application to start and render
            machine.sleep(3)

            # Take a screenshot using the machine's screenshot function
            # This will save it to the test output directory
            machine.screenshot("xterm-x11-screenshot")

            print("X11 Screenshot test completed successfully!")
            print("Screenshot saved to test output directory as xterm-x11-screenshot.png")
          '';
        };

        # NixOS test for screenshot functionality with xterm (Wayland)
        xterm-screenshot-test-wayland = pkgs.nixosTest {
          name = "ratatui-image-xterm-screenshot-wayland";

          nodes.machine = { pkgs, ... }: {
            imports = [ ];

            # Enable Wayland with sway
            programs.sway = {
              enable = true;
              wrapperFeatures.gtk = true;
            };

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
              xterm
              imagemagick  # for convert command
              grim        # Wayland screenshot tool
              wayland-utils
            ];
          };

          testScript = ''
            machine.wait_for_unit("graphical.target")
            machine.wait_until_succeeds("pgrep sway")

            # Copy the Ada.png asset to the test environment
            machine.succeed("mkdir -p /tmp/test-assets/assets")
            machine.copy_from_host("${src}/assets/Ada.png", "/tmp/test-assets/assets/Ada.png")

            # Run xterm with the screenshot example under XWayland
            machine.succeed("""
              cd /tmp/test-assets && \
              WAYLAND_DISPLAY=wayland-1 xterm -ti vt340 -fa DejaVu -fs 7 -bg black -fg white \
                -e '${ratatui-image-screenshot}/bin/screenshot' &
            """)

            # Wait for the application to start and render
            machine.sleep(3)

            # Take a screenshot using the machine's screenshot function
            # This will save it to the test output directory
            machine.screenshot("xterm-wayland-screenshot")

            print("Wayland Screenshot test completed successfully")
            print("Screenshot saved to test output directory as xterm-wayland-screenshot.png")
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
