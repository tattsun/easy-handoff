# easy-handoff

English | [日本語](README.ja.md)

Connect and disconnect Bluetooth headphones and earbuds (AirPods, Sony, Bose, ...) with a single click from the Windows system tray.
If you switch your headphones between a phone and a Windows PC, this saves you from opening the Bluetooth settings and pressing "Connect" every time.

## Installation

Download `easy-handoff-setup-x.y.z.exe` from [Releases](https://github.com/tattsun/easy-handoff/releases) and run it (no administrator rights required).
If you prefer not to use the installer, put `easy-handoff-x.y.z-portable.exe` anywhere you like and run it.

> The device must already be paired in the Windows Bluetooth settings.

## Usage

| Action | Result |
| --- | --- |
| Left click | Toggle connect / disconnect |
| Right click | Menu (connect / disconnect, device selection, launch at startup, quit) |

Tray icon:

- Filled blue circle: connected
- Gray ring: disconnected
- Spinning arc: switching (blue = connecting, gray = disconnecting)

Choose the target device from **Devices** in the right-click menu (by default, the first device whose name contains `AirPods`, or else the first Bluetooth audio device). The selection is remembered.

The UI is shown in Japanese when the Windows display language is Japanese, and in English otherwise. Set `EASY_HANDOFF_LANG=en` or `ja` to override.

### CLI

```
easy-handoff.exe list | connect | disconnect | toggle | tray  [device-name]
```

The executable is built as a GUI app, so PowerShell does not wait for it or show its output unless you pipe it, e.g. `easy-handoff.exe list | Out-String`.

## How it works

Just like the "Connect" button in the Bluetooth settings, it sends
`KSPROPERTY_ONESHOT_RECONNECT` / `KSPROPERTY_ONESHOT_DISCONNECT` from the `KSPROPSETID_BtAudio` property set to the device's Bluetooth audio KS filters.

## Development

The toolchain is managed with [mise](https://mise.jdx.dev/). Building requires Visual Studio Build Tools (MSVC).

```
mise install
cargo build --release
```

The installer is built with [Inno Setup 6](https://jrsoftware.org/isinfo.php).

```
iscc /DAppVersion=0.1.0 installer\easy-handoff.iss
```

### Releasing

Bump `version` in `Cargo.toml`, then push a tag with the same version. GitHub Actions will create the release.

```
git tag v0.1.0
git push origin v0.1.0
```

## License

[MIT](LICENSE)
