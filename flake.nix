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

      ratatui-demo = craneLib.buildPackage (commonArgs
        // {
          inherit cargoArtifacts;
          pname = "demo";
          cargoExtraArgs = "--example demo --features crossterm";
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

      makeScreenshotTest = { terminal, terminalCommand, terminalPackage, setup ? null, sleep ? null, xwayland ? false }: pkgs.nixosTest {
        name = "ratatui-test-wayland-${terminal}";

        nodes.machine = { pkgs, ... }: {
          virtualisation.memorySize = 4096;

          programs.sway = {
            enable = true;
            wrapperFeatures.gtk = true;
          };

          programs.xwayland.enable = xwayland;

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
            terminalPackage
          ];
        };

        testScript = ''
          machine.wait_for_unit("graphical.target")

          machine.wait_until_succeeds("pgrep -f sway")

          machine.succeed("mkdir -p /tmp/test-assets/assets")
          machine.copy_from_host("${src}/assets/Ada.png", "/tmp/test-assets/assets/Ada.png")

          machine.wait_until_succeeds("systemd-run --uid=test --setenv=XDG_RUNTIME_DIR=/run/user/1000 --setenv=WAYLAND_DISPLAY=wayland-1 -- swaymsg -t get_version")

          machine.succeed("${if setup != null then setup else "true"}")

          # Use systemd-run to ensure proper environment
          machine.succeed("""
            systemd-run --uid=test --setenv=XDG_RUNTIME_DIR=/run/user/1000 \
              --setenv=WAYLAND_DISPLAY=wayland-1 \
              --setenv=LIBGL_ALWAYS_SOFTWARE=1 \
              --setenv=RUST_BACKTRACE=1 \
              ${if xwayland then "--setenv=DISPLAY=:0" else ""} \
              --working-directory=/tmp/test-assets \
              -- ${terminalCommand}
          """)

          print("Waiting for /tmp/demo-ready...")

          with subtest("Waiting for /tmp/demo-ready..."):
            try:
              machine.wait_until_succeeds("test -f /tmp/demo-ready", timeout=10)
            except Exception as e:
              print(f"/tmp/demo-ready not found within timeout: {e}")
            finally:
              machine.succeed("${if sleep != null then "sleep ${toString sleep}" else "true"}")
              machine.screenshot("screenshot-${terminal}")
              print("Screenshot saved to test output directory as screenshot-${terminal}.png")
        '';
      };

      screenshotTests = {
        screenshot-test-foot = makeScreenshotTest {
          terminal = "foot";
          terminalCommand = "foot ${self.packages.${system}.demo}/bin/demo --tmp-demo-ready";
          terminalPackage = pkgs.foot;
        };

        screenshot-test-kitty = makeScreenshotTest {
          terminal = "kitty";
          terminalCommand = "kitty ${self.packages.${system}.demo}/bin/demo --tmp-demo-ready";
          terminalPackage = pkgs.kitty;
          sleep = 5;
        };

        screenshot-test-wezterm = makeScreenshotTest {
          terminal = "wezterm";
          terminalCommand = "wezterm start --always-new-process --cwd /tmp/test-assets -- ${self.packages.${system}.demo}/bin/demo --tmp-demo-ready";
          terminalPackage = pkgs.wezterm;
          sleep = 5;
        };

        screenshot-test-ghostty = makeScreenshotTest {
          terminal = "ghostty";
          terminalCommand = "ghostty -e ${self.packages.${system}.demo}/bin/demo --tmp-demo-ready";
          terminalPackage = pkgs.ghostty;
        };

        screenshot-test-mlterm = makeScreenshotTest {
          terminal = "mlterm";
          terminalCommand = "mlterm -e ${self.packages.${system}.demo}/bin/demo --tmp-demo-ready";
          terminalPackage = pkgs.mlterm;
        };

        screenshot-test-rio = makeScreenshotTest {
          terminal = "rio";
          terminalCommand = "rio -w /tmp/test-assets -e ${self.packages.${system}.demo}/bin/demo --tmp-demo-ready";
          terminalPackage = pkgs.rio;
          setup = "mkdir -p /home/test/.config/rio && touch /home/test/.config/rio/config.toml"; # Skip welcome screen
        };

        screenshot-test-xterm-vt340 = makeScreenshotTest {
          terminal = "xterm-vt340";
          terminalCommand = "xterm -ti vt340 -e ${self.packages.${system}.demo}/bin/demo --tmp-demo-ready";
          terminalPackage = pkgs.xterm;
          xwayland = true;
        };

        screenshot-test-xterm = makeScreenshotTest {
          terminal = "xterm";
          terminalCommand = "xterm -e ${self.packages.${system}.demo}/bin/demo --tmp-demo-ready";
          terminalPackage = pkgs.xterm;
          xwayland = true;
        };

        screenshot-test-blackbox = makeScreenshotTest {
          terminal = "blackbox";
          terminalCommand = "blackbox -c \"${self.packages.${system}.demo}/bin/demo --tmp-demo-ready\"";
          terminalPackage = pkgs.blackbox-terminal;
        };

        screenshot-test-xfce4-terminal = makeScreenshotTest {
          terminal = "xfce4-terminal";
          terminalCommand = "xfce4-terminal -e \"${self.packages.${system}.demo}/bin/demo --tmp-demo-ready\"";
          terminalPackage = pkgs.xfce.xfce4-terminal;
        };

        screenshot-test-contour = makeScreenshotTest {
          terminal = "contour";
          terminalCommand = "contour --working-directory /tmp/test-assets /run/current-system/sw/bin/bash -c \"sleep 1; ${self.packages.${system}.demo}/bin/demo --tmp-demo-ready\"";
          terminalPackage = pkgs.contour;
        };

        screenshot-test-alacritty = makeScreenshotTest {
          terminal = "alacritty";
          terminalCommand = "alacritty -e ${self.packages.${system}.demo}/bin/demo --tmp-demo-ready";
          terminalPackage = pkgs.alacritty;
        };
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
          windows = ratatui-image-windows;
          demo = ratatui-demo;
        }
        // screenshotTests
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
