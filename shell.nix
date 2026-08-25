{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  # Build tools
  nativeBuildInputs = with pkgs; [
    cargo
    rustc
    pkg-config
    gcc
  ];

  # Tauri 2.x native dependencies — pkg-config will find these via
  # the automatic PKG_CONFIG_PATH setup that mkShell provides.
  buildInputs = with pkgs; [
    # Core Tauri deps
    webkitgtk_4_1
    gtk3
    librsvg
    libappindicator-gtk3
    libusb1
    openssl

    # GTK/GLib transitive deps (Tauri links these)
    glib
    cairo
    pango
    gdk-pixbuf
    atk
    harfbuzz

    # X11 deps (Tauri/tao windowing)
    xorg.libXtst
    xorg.libX11
    xorg.libXcomposite
    xorg.libXcursor
    xorg.libXdamage
    xorg.libXi
    xorg.libXrandr
    xorg.libXinerama
    xorg.libXext
    xorg.libXfixes
    libglvnd
    cups
    libsoup_3
  ];

  shellHook = ''
    echo "Redragon Stream Deck dev shell"
    echo "  Build:   cargo build --release --workspace"
    echo "  Daemon:  cargo build --release -p redragon-daemon"
    echo "  Install: sudo install -Dm755 target/release/redragon-daemon /usr/local/bin/"
  '';
}
