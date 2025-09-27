{ pkgs, src, self, system }:

let
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
      sleep = 5;
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

    screenshot-test-konsole-x11 = makeScreenshotTest {
      terminal = "konsole-x11";
      terminalCommand = "konsole -e ${self.packages.${system}.demo}/bin/demo --tmp-demo-ready";
      terminalPackage = pkgs.libsForQt5.konsole;
    };

    screenshot-test-konsole = makeScreenshotTest {
      terminal = "konsole";
      terminalCommand = "konsole -e ${self.packages.${system}.demo}/bin/demo --tmp-demo-ready";
      terminalPackage = pkgs.libsForQt5.konsole;
      xwayland = true;
    };
  };
in
screenshotTests
