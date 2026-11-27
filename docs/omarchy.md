# Poratake on Omarchy 4.5

Omarchy 4.5 runs Arch Linux on Hyprland, on x86_64 and aarch64. Poratake ships
Linux x64 and arm64 archives for that session, starting with the first GPUI
release. Earlier releases have no Linux assets.

## Requirements

Omarchy 4.5 ships Hyprland, the ScreenCast portal, and the bar. Poratake also
uses `ffmpeg` for recording and export and `wl-clipboard` for clipboard file
handoff; the installer warns when either is missing. Install them with
`sudo pacman -S ffmpeg wl-clipboard`.

## Install

The installer downloads the latest release, verifies the SHA-512 published in
`latest-linux.yml`, installs under `~/.local/opt/poratake`, and adds a desktop
entry. It does not need root.

```sh
curl -fsSL https://raw.githubusercontent.com/Porabuild/Poratake/main/packaging/omarchy/install.sh -o install-poratake.sh
bash install-poratake.sh
```

To install a specific release or directory:

```sh
bash install-poratake.sh --version 1.0.6
bash install-poratake.sh --install-dir "$HOME/Applications/Poratake"
```

## Start on login

**Settings → General → Start on login** is on by default. On Hyprland the app
writes a managed block to `~/.config/hypr/autostart.conf`:

```conf
# >>> Poratake start on login
exec-once = uwsm-app -- /home/you/.local/opt/poratake/poratake-gpui
# <<< Poratake start on login
```

On Hyprland the app removes its XDG autostart entry and uses only this block,
so the session does not launch it twice. Disabling the setting removes the
block and leaves the rest of the file alone. The block needs an executable path
without spaces; the default install path satisfies that.

## Keybindings

Hyprland does not expose global shortcuts to Wayland clients, so Poratake's
shortcut recorder stays disabled there. Add bindings to
`~/.config/hypr/bindings.conf` instead, then run `hyprctl reload`:

```conf
bindd = SUPER SHIFT, S, Poratake area capture, exec, /home/you/.local/opt/poratake/poratake-gpui --intent capture-area
bindd = SUPER SHIFT, A, Poratake all-in-one, exec, /home/you/.local/opt/poratake/poratake-gpui --intent all-in-one
bindd = SUPER SHIFT, H, Poratake history, exec, /home/you/.local/opt/poratake/poratake-gpui --intent history
bindd = SUPER SHIFT, P, Poratake settings, exec, /home/you/.local/opt/poratake/poratake-gpui --intent open-settings
```

The install command prints the same lines with the chosen install directory.
Other intent ids include `capture-screen`, `capture-window`, `capture-text`,
`scan-qr-code`, `timer-capture`, `scroll-capture`, `record-area`,
`record-screen`, `record-window`, `open-in-editor`, and `open-in-video-editor`.
An intent that the current session does not support shows a "Not available"
toast instead of failing silently.

## Expected to work on Omarchy 4.5

These paths are verified on GNOME Wayland and use the same ScreenCast and
overlay code, but they still need a pass on a real Hyprland session:

- Screen and area screenshots through the ScreenCast portal
- The screenshot editor, all-in-one panel, history, pin, print, and video editor
- Single instance, file handoff, cloud upload, and theme following
- aarch64 and x86_64 builds

## Not available on Wayland yet

- Screen, area, and window recording
- Window screenshots, OCR, timer capture, scroll capture, freeze screen,
  display selection, desktop wallpaper, and the color picker
- Application-level global shortcuts (use Hyprland bindings above)

The Wayland recorder is deliberately disabled. `zed-scap` reports a zero output
size on Linux and does not expose the portal stream's source type, position, or
size, so an area recording cannot be cropped or passed to FFmpeg correctly. The
`linuxWayland` target in `src/types/platform-capabilities.json` omits recording,
and `src/main/app-gpui/src/system/capabilities.rs` applies the runtime
Wayland/ScreenCast feature gating. Re-enable recording only after source-aware
stream selection and area mapping have been verified on a real Hyprland session.

X11 sessions keep the full Linux feature set, including recording and window
capture.

## Uninstall

```sh
rm -rf "${PORATAKE_INSTALL_DIR:-$HOME/.local/opt/poratake}"
rm -f "${XDG_BIN_HOME:-$HOME/.local/bin}/poratake-gpui"
rm -f "${XDG_DATA_HOME:-$HOME/.local/share}/applications/com.porabuild.poratake.desktop"
rm -f "${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/256x256/apps/poratake.png"
rm -f "${XDG_CONFIG_HOME:-$HOME/.config}/autostart/com.porabuild.poratake-gpui.desktop"
```

Remove the managed block from `~/.config/hypr/autostart.conf` if it is still
present. The block starts with `# >>> Poratake start on login` and ends with
`# <<< Poratake start on login`.
