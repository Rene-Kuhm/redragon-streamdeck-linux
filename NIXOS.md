# NixOS Installation

NixOS uses declarative configuration — packages, udev rules, and systemd
services are defined in `/etc/nixos/configuration.nix`, not installed
imperatively. This guide walks through the full setup.

## Quick start

```bash
git clone https://github.com/Rene-Kuhm/redragon-streamdeck-linux.git
cd redragon-streamdeck-linux
./install.sh --skip-deps
```

This builds the project and generates NixOS config snippets. Then follow the
steps below to add them to your `configuration.nix`.

## 1. System packages

Add to `environment.systemPackages` in `/etc/nixos/configuration.nix`:

```nix
environment.systemPackages = with pkgs; [
  # ... your existing packages ...

  # Redragon Stream Deck — Tauri GUI build dependencies
  gtk3
  webkitgtk_4_1
  librsvg
  libappindicator-gtk3
  libusb1
  openssl
  pkg-config
  gcc

  # Runtime tools
  ydotool
  playerctl
];
```

## 2. User groups

Add `input` to your user's groups (for ydotool keyboard emulation):

```nix
users.users.YOUR_USER = {
  extraGroups = [
    "networkmanager" "wheel"
    "input"   # ydotool (Stream Deck keyboard actions)
  ];
};
```

## 3. Udev rules

Add to `services.udev.extraRules`:

```nix
services.udev.extraRules = ''
  # Redragon SS-550 Stream Deck (VID 0200 PID 1000)
  SUBSYSTEM=="usb", ATTR{idVendor}=="0200", ATTR{idProduct}=="1000", \
    GROUP="plugdev", MODE="0660"
  SUBSYSTEM=="hidraw", ATTRS{idVendor}=="0200", ATTRS{idProduct}=="1000", \
    GROUP="plugdev", MODE="0660"
'';
```

Both rules are needed:
- The `usb` rule gives the daemon access to the raw USB device via libusb
- The `hidraw` rule gives access to the HID device node

The `usbhid` kernel driver must be detached to claim the USB interface, which
requires root. This is why the daemon runs as a system service (see step 5).

## 4. ydotool

Add a system service for `ydotoold`:

```nix
systemd.services.ydotoold = {
  description = "ydotoold - ydotool daemon";
  wantedBy    = [ "multi-user.target" ];
  after       = [ "systemd-udevd.service" ];
  serviceConfig = {
    Type              = "simple";
    RuntimeDirectory  = "ydotoold";
    RuntimeDirectoryMode = "0755";
    ExecStart = "${pkgs.ydotool}/bin/ydotoold --socket-path=/run/ydotoold/socket --socket-perm=0660";
    Restart           = "on-failure";
    RestartSec        = "2s";
  };
};
```

Create `~/.config/environment.d/60-redragon-ydotool.conf`:

```
YDOTOOL_SOCKET=/run/ydotoold/socket
```

## 5. Daemon service

On NixOS the daemon runs as a **system service** (root) because `libusb` needs
to detach the `usbhid` kernel driver, which requires root privileges.

Add to `systemd.services`:

```nix
systemd.services.redragon-daemon = {
  description = "Redragon Stream Deck daemon";
  wantedBy    = [ "multi-user.target" ];
  after       = [ "systemd-udevd.service" ];
  requires    = [ "systemd-udevd.service" ];
  serviceConfig = {
    Type      = "simple";
    ExecStart = "/usr/local/bin/redragon-daemon --config-dir /home/YOUR_USER/.local/share/com.tecnodespegue.redragon-streamdeck";
    Restart   = "on-failure";
    RestartSec = "5s";
    Environment = [ "YDOTOOL_SOCKET=/run/ydotoold/socket" ];
  };
};
```

Replace `YOUR_USER` with your actual username.

## 6. Build and install

Use the provided `shell.nix` for the build environment:

```bash
cd redragon-streamdeck-linux
nix-shell --run "cargo build --release --workspace"
sudo install -Dm755 target/release/redragon-daemon /usr/local/bin/redragon-daemon
sudo install -Dm755 target/release/redragon-streamdeck /usr/local/bin/redragon-streamdeck
```

## 7. Apply configuration

```bash
sudo nixos-rebuild switch
```

## 8. Replug the device

Disconnect and reconnect the Stream Deck so the new udev rules apply.

## GUI permissions

The GUI also needs root to open the USB device via libusb. Create a polkit
policy to allow passwordless escalation:

```bash
sudo tee /etc/polkit-1/rules.d/50-redragon-streamdeck.rules <<'EOF'
polkit.addRule(function(action, subject) {
    if (action.id == "org.freedesktop.policykit.exec" &&
        action.lookup("exec") == "/usr/local/bin/redragon-streamdeck") {
        if (subject.isInGroup("users")) {
            return polkit.Result.YES;
        }
    }
});
EOF
```

Then use this wrapper for `/usr/local/bin/redragon-streamdeck`:

```bash
#!/bin/sh
LIBAPPINDICATOR="/nix/store/...-libappindicator-gtk3-.../lib"  # find with: nix-store -qR $(nix-instantiate -A libappindicator-gtk3) | grep appindicator
if [ "$(id -u)" -eq 0 ]; then
    export LD_LIBRARY_PATH="${LIBAPPINDICATOR}${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
    exec /usr/local/bin/.redragon-streamdeck-bin "$@"
fi
exec pkexec env \
    DISPLAY="${DISPLAY:-:0}" \
    XAUTHORITY="${XAUTHORITY:-$HOME/.Xauthority}" \
    LD_LIBRARY_PATH="${LIBAPPINDICATOR}" \
    HOME="$HOME" \
    /usr/local/bin/.redragon-streamdeck-bin "$@"
```

(Rename the original binary to `.redragon-streamdeck-bin` first.)

## Full configuration.nix example

```nix
{ config, lib, pkgs, ... }:
{
  environment.systemPackages = with pkgs; [
    gtk3 webkitgtk_4_1 librsvg libappindicator-gtk3
    libusb1 openssl pkg-config gcc
    ydotool playerctl
  ];

  users.users.tecnodespegue.extraGroups = [ "input" ];

  services.udev.extraRules = ''
    SUBSYSTEM=="usb", ATTR{idVendor}=="0200", ATTR{idProduct}=="1000", \
      GROUP="plugdev", MODE="0660"
    SUBSYSTEM=="hidraw", ATTRS{idVendor}=="0200", ATTRS{idProduct}=="1000", \
      GROUP="plugdev", MODE="0660"
  '';

  systemd.services.ydotoold = {
    description = "ydotoold - ydotool daemon";
    wantedBy    = [ "multi-user.target" ];
    after       = [ "systemd-udevd.service" ];
    serviceConfig = {
      Type              = "simple";
      RuntimeDirectory  = "ydotoold";
      RuntimeDirectoryMode = "0755";
      ExecStart = "${pkgs.ydotool}/bin/ydotoold --socket-path=/run/ydotoold/socket --socket-perm=0660";
      Restart           = "on-failure";
      RestartSec        = "2s";
    };
  };

  systemd.services.redragon-daemon = {
    description = "Redragon Stream Deck daemon";
    wantedBy    = [ "multi-user.target" ];
    after       = [ "systemd-udevd.service" ];
    requires    = [ "systemd-udevd.service" ];
    serviceConfig = {
      Type      = "simple";
      ExecStart = "/usr/local/bin/redragon-daemon --config-dir /home/tecnodespegue/.local/share/com.tecnodespegue.redragon-streamdeck";
      Restart   = "on-failure";
      RestartSec = "5s";
      Environment = [ "YDOTOOL_SOCKET=/run/ydotoold/socket" ];
    };
  };
}
```

## Troubleshooting

### Device permission denied

The daemon runs as root via systemd, but if you see permission errors:

```bash
# Check udev rules are loaded
sudo udevadm control --reload-rules && sudo udevadm trigger
# Verify permissions
ls -la /dev/bus/usb/$(lsusb -d 0200:1000 | cut -d' ' -f1,2 | tr ' ' '/' | tr -d ':')
ls -la /dev/hidraw*
```

### GUI fails to open device

The GUI needs root to detach the kernel driver. Use the polkit wrapper
described above, or run with sudo:

```bash
sudo redragon-streamdeck
```

### Daemon not finding config

When running as root, the daemon looks in `/root/.local/share/...` by default.
Use `--config-dir` to point to your user's config:

```bash
ExecStart = "/usr/local/bin/redragon-daemon --config-dir /home/YOUR_USER/.local/share/com.tecnodespegue.redragon-streamdeck"
```

### Keyboard actions not working

```bash
# Check ydotoold is running
systemctl status ydotoold
# Check socket exists with correct permissions
ls -la /run/ydotoold/socket
# Test
YDOTOOL_SOCKET=/run/ydotoold/socket ydotool type ""
```
