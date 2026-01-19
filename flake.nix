{
  description = "Handy - Cross-platform desktop speech-to-text application";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";


    bun2nix.url = "github:nix-community/bun2nix";
    bun2nix.inputs.nixpkgs.follows = "nixpkgs";
  };

  nixConfig = {
    extra-substituters = [
      "https://cache.nixos.org"
      "https://nix-community.cachix.org"
    ];
    extra-trusted-public-keys = [
      "cache.nixos.org-1:6NCHdD59X431o0gWypbMrAURkbJ16ZPMQFGspcDShjY="
      "nix-community.cachix.org-1:mB9FSh9qf2dCimDSUo8Zy7bkq5CX+/rkCWyvRCYg3Fs="
    ];
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, bun2nix }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        # Rust toolchain with specific version for consistency
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" ];
          targets = [ "x86_64-unknown-linux-gnu" ];
        };

        # System dependencies for Tauri
        systemDeps = with pkgs; [
          # Build tools
          pkg-config
          cmake
          
          # Audio libraries
          alsa-lib
          alsa-lib.dev
          
          # Graphics and UI
          gtk3
          gtk3.dev
          webkitgtk_4_1
          webkitgtk_4_1.dev
          
          # System tray support
          libayatana-appindicator
          libayatana-appindicator.dev
          
          # Additional dependencies
          librsvg
          librsvg.dev
          openssl
          openssl.dev
          glib
          glib.dev
          cairo
          cairo.dev
          gdk-pixbuf
          gdk-pixbuf.dev
          atk
          atk.dev
          pango
          pango.dev
          
          # Vulkan development dependencies (for whisper-rs)
          vulkan-headers
          vulkan-loader
          vulkan-tools
          vulkan-validation-layers
          shaderc # provides glslc
          
          # Additional C/C++ build dependencies
          gcc
          glibc.dev
          stdenv.cc.libc.dev
          clang
          
          # Modern linker to avoid argument length issues
          mold
          
          # Linux input method tools (for fallback)
          wtype
          xdotool
          
          # udev for evdev support
          udev
          udev.dev
          
          # X11 libraries (needed by some dependencies)
          xorg.libX11
          xorg.libX11.dev
          xorg.libXext
          xorg.libXrandr
          xorg.libXcursor
          xorg.libXdamage
          xorg.libXfixes
          xorg.libXi
          xorg.libXrender
          xorg.libXScrnSaver
          xorg.libXtst
          xorg.libxcb
          
          # Tools for development
          curl
          wget
          git
          
          # Patchelf for fixing rpath
          patchelf
        ];

        # Bun for frontend dependencies  
        bunDeps = with pkgs; [
          bun
          nodejs_20
        ];

          # Environment variables for Tauri development
        tauriEnv = {
          # Required for Tauri
          WEBKIT_DISABLE_DMABUF_RENDERER = "1";
          
          # Graphics acceleration
          LIBGL_ALWAYS_SOFTWARE = "0";
          
          # Fix for potential linker issues
          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath systemDeps;
          
          # PKG_CONFIG setup
          PKG_CONFIG_PATH = pkgs.lib.makeSearchPath "lib/pkgconfig" systemDeps;
          
          # libclang for bindgen (whisper-rs-sys needs this)
          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
          BINDGEN_EXTRA_CLANG_ARGS = "-I${pkgs.glibc.dev}/include -I${pkgs.llvmPackages.clang}/resource-root/include";
          
          # CMAKE configuration for native builds
          CMAKE_C_COMPILER = "${pkgs.clang}/bin/clang";
          CMAKE_CXX_COMPILER = "${pkgs.clang}/bin/clang++";
          CC = "${pkgs.clang}/bin/clang";
          CXX = "${pkgs.clang}/bin/clang++";
          
          # Use mold linker directly to avoid argument length issues
          RUSTFLAGS = "-C linker=${pkgs.clang}/bin/clang -C link-arg=-fuse-ld=${pkgs.mold}/bin/mold";
          
          # Development mode optimizations
          RUST_BACKTRACE = "1";
          RUST_LOG = "debug";
          
          # uinput permissions for evdev (development)
          # Note: In production, users should configure proper groups
          HANDY_WARN_UINPUT = "1";
        };

      in
      {
        # Development shell
        devShells.default = pkgs.mkShell (tauriEnv // {
          buildInputs = systemDeps ++ bunDeps ++ [
            rustToolchain
            pkgs.pkg-config
            pkgs.cmake
            pkgs.llvmPackages.libclang
            pkgs.llvmPackages.clang
            pkgs.clang
            pkgs.mold
          ];

	  packages = with pkgs; [
            bun
            bun2nix.packages.${system}.default
	  ];
          
          shellHook = ''
            echo "🔧 Handy Development Environment"
            echo "================================"
            echo
            echo "📋 Available commands:"
            echo "  bun install          - Install frontend dependencies"  
            echo "  bun tauri dev        - Start development server"
            echo "  bun tauri build      - Build for production"
            echo "  cargo test           - Run Rust tests"
            echo "  cargo clippy         - Run Rust linter"
            echo
            echo "🔑 uinput Setup (for evdev support):"
            echo "  sudo usermod -aG input \$USER"
            echo "  # Then log out and back in"
            echo
            echo "📁 Project structure:"
            echo "  src/                 - Frontend (React/TypeScript)"
            echo "  src-tauri/src/       - Backend (Rust)"
            echo "  src-tauri/Cargo.toml - Rust dependencies"
            echo
            echo "🚀 Quick start:"
            echo "  bun install && bun tauri dev"
            echo
            
            # Check if user has uinput access
            if [ -c /dev/uinput ]; then
              if [ -w /dev/uinput ]; then
                echo "✅ uinput access available - evdev input method will work"
              else
                echo "⚠️  uinput exists but not writable - run: sudo usermod -aG input \$USER"
              fi
            else
              echo "❌ /dev/uinput not found - evdev input method unavailable"
              echo "   Try: sudo modprobe uinput"
            fi
            
            # Verify required tools
            echo
            echo "🔍 Input method tools:"
            which wtype >/dev/null 2>&1 && echo "✅ wtype available" || echo "⚠️  wtype not found"
            which xdotool >/dev/null 2>&1 && echo "✅ xdotool available" || echo "⚠️  xdotool not found"
            which dotool >/dev/null 2>&1 && echo "✅ dotool available" || echo "⚠️  dotool not found"
            
            echo
          '';
        });

        # Build packages
        packages = {
          default = self.packages.${system}.handy;
          
          handy = pkgs.rustPlatform.buildRustPackage rec {
            pname = "handy";
            version = "0.6.11";

            # Source filtering to exclude non-essential files
            src = pkgs.lib.cleanSourceWith {
              filter = path: type:
                let baseName = baseNameOf path; in
                !(pkgs.lib.hasSuffix ".direnv" path) &&
                !(pkgs.lib.hasSuffix ".git" path) &&
                !(pkgs.lib.hasSuffix ".github" path) &&
                !(pkgs.lib.hasSuffix "target" path) &&
                !(pkgs.lib.hasSuffix "node_modules" path) &&
                !(pkgs.lib.hasSuffix ".envrc" path) &&
                !(pkgs.lib.hasPrefix "." baseName);
              src = ./.;
            };

	    cargoHash = "sha256-YNZaJbtORFK3X6RZTDopGyHWIivl2mhbvi3aRvpc1m0=";

          # Build dependencies
            nativeBuildInputs = systemDeps ++ bunDeps ++ [
              rustToolchain
              pkgs.pkg-config
              pkgs.cmake
              pkgs.llvmPackages.libclang
              pkgs.llvmPackages.clang
              pkgs.clang
              pkgs.mold
              bun2nix.packages.${system}.default
            ];

            buildInputs = systemDeps;

            # Pre-build: Install frontend dependencies
            preBuild = ''
              # Install Bun dependencies
              export HOME=$(mktemp -d)
              #cd $sourceRoot
              bun install --frozen-lockfile
              
              # Build frontend
              bun run build
            '';

            # Build configuration
            buildAndTestSubdir = "src-tauri";
            
            # Environment for build
            env = tauriEnv // {
              # Disable network access during build
              CARGO_NET_OFFLINE = "true";
              
              # Tauri build configuration
              TAURI_PRIVATE_KEY = "";
              TAURI_KEY_PASSWORD = "";
              
              # Skip model downloads during build
              HANDY_SKIP_MODEL_DOWNLOAD = "1";
            };

            # Post-install fixes
            postInstall = ''
              # Wrap binary with required library paths
              wrapProgram $out/bin/handy \
                --prefix LD_LIBRARY_PATH : "${pkgs.lib.makeLibraryPath systemDeps}" \
                --set WEBKIT_DISABLE_DMABUF_RENDERER "1"
              
              # Install desktop file
              install -Dm644 $sourceRoot/src-tauri/icons/128x128.png \
                $out/share/pixmaps/handy.png
                
              # Create desktop entry
              mkdir -p $out/share/applications
              cat > $out/share/applications/handy.desktop << 'DESKTOP_EOF'
[Desktop Entry]
Type=Application
Name=Handy
Comment=Cross-platform desktop speech-to-text application
Exec=${placeholder "out"}/bin/handy
Icon=handy
Categories=AudioVideo;Audio;
StartupWMClass=handy
DESKTOP_EOF
            '';

            meta = with pkgs.lib; {
              description = "Cross-platform desktop speech-to-text application";
              homepage = "https://github.com/cjpais/Handy";
              license = licenses.mit;
              maintainers = [ ];
              platforms = platforms.linux;
              
              longDescription = ''
                Handy is a free, open source, and extensible speech-to-text application 
                that works completely offline. Built with Tauri (Rust + React/TypeScript), 
                it provides simple, privacy-focused speech transcription using local 
                Whisper models with GPU acceleration when available.
                
                Features:
                - Completely offline - your voice stays on your computer
                - Multiple model sizes (Small/Medium/Turbo/Large Whisper variants)
                - Cross-platform (Linux, macOS, Windows)
                - Global keyboard shortcuts
                - Multiple text input methods including native evdev support
                - Voice Activity Detection with Silero VAD
              '';
            };
          };
        };

        # Development tools and checks
        checks = {
          # Rust formatting
          rust-fmt = pkgs.runCommand "rust-fmt-check" {
            nativeBuildInputs = [ rustToolchain ];
          } ''
            cd ${self}
            cargo fmt --all --check
            touch $out
          '';

          # Rust linting  
          rust-clippy = pkgs.runCommand "rust-clippy-check" {
            buildInputs = systemDeps;
            nativeBuildInputs = [ rustToolchain ] ++ systemDeps;
          } (tauriEnv // {
            buildCommand = ''
              cd ${self}/src-tauri
              cargo clippy --all-targets --all-features -- -D warnings
              touch $out
            '';
          });

          # Rust tests
          rust-test = pkgs.runCommand "rust-test" {
            buildInputs = systemDeps;
            nativeBuildInputs = [ rustToolchain ] ++ systemDeps;
          } (tauriEnv // {
            buildCommand = ''
              cd ${self}/src-tauri
              cargo test --all --all-features
              touch $out
            '';
          });
        };

        # Development commands
        apps = {
          default = self.apps.${system}.handy;
          
          handy = flake-utils.lib.mkApp {
            drv = self.packages.${system}.handy;
          };
          
          # Development server
          dev = flake-utils.lib.mkApp {
            drv = pkgs.writeShellScriptBin "handy-dev" ''
              set -e
              echo "🚀 Starting Handy development server..."
              
              if [ ! -d "node_modules" ]; then
                echo "📦 Installing frontend dependencies..."
                ${pkgs.bun}/bin/bun install
              fi
              
              echo "🔧 Starting Tauri dev server..."
              ${pkgs.bun}/bin/bun tauri dev
            '';
          };
          
          # Production build
          build = flake-utils.lib.mkApp {
            drv = pkgs.writeShellScriptBin "handy-build" ''
              set -e
              echo "🏗️  Building Handy for production..."
              
              if [ ! -d "node_modules" ]; then
                echo "📦 Installing frontend dependencies..."
                ${pkgs.bun}/bin/bun install
              fi
              
              echo "🔧 Building Tauri application..."
              ${pkgs.bun}/bin/bun tauri build
              
              echo "✅ Build complete!"
              echo "📁 Output: $(pwd)/src-tauri/target/release/bundle/"
            '';
          };
        };

        # Formatter for nix files
        formatter = pkgs.nixpkgs-fmt;
      }
    );
}
