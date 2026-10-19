<p align="center">
  <img src="assets/icons/desktop/128x128.png" width="128" height="128" alt="FreeControl2" />
</p>

<h1 align="center">FreeControl2</h1>

<p align="center">
  <a href="https://github.com/pdone/FreeControl2/actions/workflows/build.yml"><img src="https://img.shields.io/github/actions/workflow/status/pdone/FreeControl2/build.yml?style=for-the-badge" alt="Build status" /></a>
  <a href="https://github.com/pdone/FreeControl2/releases/latest"><img src="https://img.shields.io/github/release/pdone/FreeControl2?style=for-the-badge" alt="Latest release" /></a>
  <a href="https://github.com/pdone/FreeControl2/releases"><img src="https://img.shields.io/github/downloads/pdone/FreeControl2/total?style=for-the-badge" alt="Downloads" /></a>
  <a href="https://github.com/pdone/FreeControl2"><img src="https://img.shields.io/github/stars/pdone/FreeControl2?style=for-the-badge" alt="Stars" /></a>
  <a href="https://github.com/pdone/FreeControl2/issues"><img src="https://img.shields.io/github/issues/pdone/FreeControl2?style=for-the-badge" alt="Issues" /></a>
  <a href="LICENSE"><img src="https://img.shields.io/github/license/pdone/FreeControl2?style=for-the-badge" alt="License" /></a>
</p>

<p align="center">Control Android devices on PC</p>

<p align="center">
  <a href="README.md">中文</a> | English | <a href="https://freecontrol2.pages.dev/">Website</a>
</p>


## Features

- **Device list** — one alias per device, mapped to a USB serial or `IP:port`; add, rename and remove, with live online/connected status
- **Parameter panels** — the common options of six groups as controls, each mapped to its scrcpy long option name, plus a custom-arguments box
- **Per-device overrides** — a device's own parameter set, switched on or off as a whole
- **Connection control** — connect or disconnect one device or all of them; several devices can mirror at once, with processes and output managed by the app
- **Runtime** — scrcpy and adb fetched on demand; view the path, open the directory, check for updates
- **Download channels** — direct, built-in proxy and your own proxies, tried in order, with the channel in use shown in the UI and downloads verified against the official checksums
- **Logs** — per-session output persisted, kept for 7 days / 30 days / forever
- **Command preview** — the exact command about to run, shown at the bottom and copyable
- **Appearance** — light/dark themes and accent color, following the system by default, with a sidebar that collapses to an icon rail

## Interface

The app UI is currently in Chinese.

**Devices** — device list and connection control:

<img src="assets/screenshots/devices.png" alt="Devices page: device list and connection controls" />

**Parameters** — the common option controls:

<img src="assets/screenshots/video.png" alt="Video parameter page" />

**Runtime** — where scrcpy and adb are fetched:

<img src="assets/screenshots/environment.png" alt="Runtime page" />

**Logs** — session output:

<img src="assets/screenshots/logs.png" alt="Logs page" />

## Quick Start

**To run**: Windows 10 / 11 (one portable single file each for x64 and ARM64) · the WebView2 runtime that ships with the system (usually preinstalled on 10 / 11; installable via Microsoft's Evergreen bootstrapper if missing) · an Android device with USB debugging enabled.

Get the latest version from [Releases](https://github.com/pdone/FreeControl2/releases/latest): on Windows it is a single portable file — pick `FreeControl2_<version>_x64.exe` or `FreeControl2_<version>_arm64.exe` for your architecture and put it in any writable directory, with no installer; on Linux it ships as AppImage and deb release artifacts.

1. **Get the runtime** — on first launch, go to the "Runtime" page and fetch scrcpy and adb.
2. **Add a device** — on the "Devices" page, add one: for USB, enter the serial listed by `adb devices`; for wireless, enter `IP:port`.
3. **Connect and mirror** — click "Connect" on the device card. Several devices can be connected at once; the command preview at the bottom shows the exact command being run.
4. From there you can adjust the defaults on the parameter pages, or give a single device its own override.

### Build from source

You additionally need Node.js 24 and npm · a stable Rust toolchain · the MSVC build tools on Windows, or system libraries such as webkit2gtk on Linux (see `.github/workflows/build.yml` for the list).

```sh
npm install
```

Windows (single portable executable):

```sh
npm run app:build
```

The artifact is written to `src-tauri/target/release/FreeControl2.exe`.

Linux (AppImage and deb):

```sh
npm run tauri build -- --bundles appimage,deb
```

The artifacts are written to `src-tauri/target/release/bundle/`.

When you trigger `.github/workflows/build.yml` manually, the artifacts are collected into a **draft release** (never published automatically). If the same version has already been published, re-running a build will not overwrite it: the artifacts land in a `v<version>-draft` pickup draft instead — that draft only exists to retrieve artifacts and is **never published**.

### Local development

```sh
npm install
npm run app:dev
```

That one command starts the whole app, with the frontend dev server brought up by the build config (`beforeDevCommand` in `src-tauri/tauri.conf.json`; the UI runs at `http://localhost:1420`). The common commands are registered as editor tasks in `.vscode/tasks.json`, so you can also just pick them by name from the task list.

## Project Structure

```
FreeControl2/
├─ src/                    Frontend (Svelte + Vite + TypeScript)
│  ├─ components/          UI components (devices, parameter pages, logs, runtime, about dialog…)
│  └─ lib/                 Frontend logic (parameter model, command building, validation, config)
├─ src-tauri/              Backend (Rust + Tauri 2)
│  ├─ src/                 Command layer and platform code (device, scrcpy, config, assets, download, session, logging)
│  └─ tauri.conf.json      App and bundling config — the single source of the version number
├─ assets/icons/           Brand asset package (single source, with the icon build pipeline and its docs)
├─ scripts/                Icon sync script (mirrors the bundling manifest into src-tauri/icons/)
├─ .vscode/                Editor task entries (they only reference the script names in package.json)
├─ ui-design/              UI baseline prototype (read-only reference)
├─ openspec/               Specs and change artifacts
└─ Update.md               Changelog
```

## FAQ

### A device will not connect

**USB** — make sure the device has "Developer options" and "USB debugging" enabled, the cable is intact, the driver is fine on Windows (no yellow warning in Device Manager), and `adb devices` lists it. If "Developer options" is missing, tap "Build number" seven times in "Settings" - "About phone".

**Wireless** — the first time you need USB: run `adb tcpip 5555`, unplug, then `adb connect <IP>:5555`. On Android 11 and above you can also use the pairing code under "Wireless debugging" (`adb pair <IP>:<port>`, then `adb connect <IP>:<port>`). The device and the computer must be on the same network.

### It says scrcpy or adb is missing

Fetch them on the "Runtime" page. Neither ships with the app; both are downloaded into the application data directory on demand, or you can point that page at an existing custom directory.

### The download fails or hangs on one channel

A failed direct connection falls back to the next channel; you can also switch channels in place from the progress area. Channels that cannot be reached are skipped for the rest of the run instead of retried.

### The window is blank after launch on Windows

Make sure the WebView2 runtime is installed (usually included with Windows 10 / 11).

### The window stops updating after launch on Linux

On setups such as virtual machines, software rendering or remote sessions, the window may draw its first frame and then stop updating (clicks still take effect). The app detects this at launch and avoids that rendering path on its own, so most such setups need nothing from you; if you still hit it, start the app with `WEBKIT_DISABLE_DMABUF_RENDERER=1 FreeControl2` — the detection is heuristic, so some setups fall through.

### Where does the data live

All of it is under the application data root (`%APPDATA%/FreeControl2` on Windows, a directory of the same name under the XDG data root on Linux):

| Path | Purpose |
|---|---|
| `config.json` | The single configuration file (with a `schemaVersion`): device list, default parameters, per-device overrides, log retention |
| `logs/yyyy-MM-dd.log` | Persisted session output and key actions of the app itself |
| `scrcpy-{OS}-v{version}/` | The scrcpy and adb runtime downloaded on demand (when no custom directory is set) |

Everything you can change in the UI lands in `config.json`.

### How do I reset to the factory defaults

Delete `config.json` and start the app again: the device list, the parameters and the retention policy all go back to their factory values.

### Where are the logs

The "Logs" page has a one-click shortcut to the log directory; the files live under `logs/` in the application data root and are named by date.

### adb is still running after I quit the app

That is intentional: quitting does not stop the adb server, so wireless connections survive the exit and are reused on the next launch.

## License

[MIT](LICENSE)

## Acknowledgements

- [scrcpy](https://github.com/Genymobile/scrcpy) — mirroring and device control are entirely its doing; this project is a shell around it.
- [Tauri](https://tauri.app/), [Svelte](https://svelte.dev/), [Vite](https://vite.dev/), [lucide](https://lucide.dev/) — the app shell and UI.
- [FreeControl](https://github.com/pdone/FreeControl), its predecessor — this project is a rework built on the interaction lessons from it.
