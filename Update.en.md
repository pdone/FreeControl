# Changelog

<a href="Update.md">中文</a> | English

## v1.2.0

### New

- A macOS build: one dmg for Apple silicon and one for Intel; open it and drag the app into "Applications" to use it, allowing it through on first launch as the docs describe. Both "Check for updates" in "About" and the website downloads pick the build for your architecture.
- Interface language: switch the UI between Simplified Chinese and English on the Settings page, or follow the system. Headings, buttons and hints switch right away, with no restart.
- Wireless pairing and discovery: pair once with the pairing code, then list the devices on your network that have wireless debugging on in one click; the connection port is worked out by the app.
- Adding a device is one dialog now: USB, a manual address, a network scan and a pairing code all live in the same place, and you can name the device or each scan result up front.

### Improved

- Parameter dropdowns no longer use the system popup list but a list drawn inside the page: long lists scroll and the keyboard works, and the parameter help panel sits above the list instead of being covered by it.
- Device status now comes straight from the adb service: devices going online or offline and connection changes show up right away, and the actual online state is checked once more before connecting.
- Hover feedback and panel entrance timings were evened out, and switching between dark and light cross-fades the page once — not on first apply, and not with reduced motion on.
- Installer file names now carry the platform and architecture (for example `FreeControl2_1.2.0_windows_x64.exe` and `FreeControl2_1.2.0_macos_arm64.dmg`), so the releases page shows at a glance which file is for your system.
- On older versions, "Check for updates" reports that no update package is available; download this version once by hand, after which automatic updates resume.

### Fixed

- After switching the runtime to another directory, one click on "Connect" now succeeds, with no repeated retries.

## v1.1.0

### New

- Supports scrcpy 5.0.1, with controls for its six new parameters.
- In-app updates: click "Check for updates" in "About" to see whether a new version exists, download it in place and swap the program after verification. The portable builds on Windows and Linux restart on their own to finish; package builds are handed to the system installer.
- Automatic update checks: switch it on in "Settings" and the app looks for new versions in the background. When one is found it only opens the update window, without downloading or installing anything by itself.
- A native ARM64 build for Windows: Windows now ships one portable file per architecture, so ARM machines no longer run under emulation.
- Appearance: dark, light and follow-system, with an accent color from eight presets or one you pick yourself.
- A configurable runtime folder: point the app at a folder you already have instead of the one it downloads.
- Every parameter has a "?" detail: what it does, platform and version limits, conflicts with other options, and what the less obvious values mean. The line shown permanently keeps only the unit, the empty state and the switch polarity.

### Improved

- Logs no longer have a page of their own in the sidebar: session output, log-file options and log files are now three cards on the "Settings" page. The "Log" button on a device card jumps there and scrolls the session output into view.
- Parameter hints read the same everywhere: the permanently shown line no longer ends with a period, numeric fields spell out what "0" and "leave empty" mean, and the detail popovers share one shadow and entrance animation. A "?" detail covers the other entries while it is open, and clicking its trigger pins it in place.
- A command that is too long for the preview now scrolls sideways instead of being squeezed onto one line and cut off.
- The self-drawn line at the top of the window is gone, so screenshots no longer include it; the system frame on the left, right and bottom edges is unchanged.

### Fixed

- On Linux the three window buttons in the top-right corner could stop responding after a cold start until the window was maximized once.
- On Linux, virtual machines, software rendering and remote sessions could draw the first frame and then stop updating. The app now detects this at launch and avoids that rendering path on its own; for the setups it cannot detect, starting it with that rendering turned off still works around it.

## v1.0.1

### Improved

- Tooltips now appear next to the control they describe, with a small arrow pointing at it: no more grey system bubbles, and no more clipping by the sidebar or card edges. Tooltips that only repeated text already on screen, or merely said that something could be clicked, were removed.
- In the "About" dialog, "Support" became "License" (it opens the full license) and "Website" now points at the new site, moved to the first position.

### Fixed

- Opening the links in "About", or the folder that holds the logs and the runtime, no longer hangs for a moment, and the window no longer goes briefly unresponsive.
- The line at the top of the window is now drawn only when the system does not draw one. Windows 11 already draws its own, so the two used to stack into a line thicker than the other three edges; it now matches them. Windows 10 looks the same as before.

## v1.0.0

### New

- The first release: an Android mirroring tool that turns the common scrcpy parameters into UI controls. It is one portable file that relies on the WebView2 runtime that ships with Windows; scrcpy itself is not bundled and the runtime is fetched on demand.
- Device management: give each device an alias and keep its connection ID (USB or wireless); add, rename and delete, with connected / online / offline shown live.
- Connection control: connect and disconnect a single device, or all of them at once; several devices can mirror at the same time.
- A wireless device that drops off can be recovered in place: after being disconnected outside the app, click "Connect" on its card to connect and mirror again; when that fails, the reason is shown (port closed, address changed, timed out). An offline USB device cannot be connected.
- Parameter panels: the common options of six groups — source, video, audio, control, recording and window — are all controls, and the display source can be the main screen, a virtual display or a camera; the "Advanced" page has a box for parameters that do not have a control yet.
- Defaults are not pre-set for you: picture-related options start unset and are left to scrcpy, and audio forwarding is off by default.
- Per-device parameter overrides: give a single device its own parameter set, switch it on or off as a whole without touching the defaults; an enabled override is marked on the device card.
- Runtime: scrcpy and adb are fetched on demand; see where they are installed, open that folder, check for updates, and choose between "Download the best-fit version" and "Download the latest version".
- Download channels: several built-in channels that fall back in order when the direct one fails, showing which one is in use; switch channel or cancel while downloading, and nothing is written until it matches the official checksums.
- Top bar status: shows "n devices · x online · y connected"; click it to return to the devices page, click the indicator on the right to go to the runtime page.
- A log page: read each session's output one by one (finished sessions are kept too) and clear it at any time.
- Log retention: session output and key actions are written to log files, kept for 7 days / 30 days / forever (7 days by default), with a button to open the log folder. Clearing the session output only clears what is on screen; the files on disk are untouched.
- Command preview: the full command about to run is shown live at the bottom of the devices page and of every parameter page, and can be copied in one click.
- The sidebar collapses to an icon rail to free up width.
- The title bar is drawn by the app itself: the three window buttons sit at the right end of the top bar, empty space drags the window and a double click toggles maximize. Snap Layouts on Windows 11 is therefore unavailable.
- After the last device is disconnected the main window comes back to the front, even if you had switched to another program.
- Quitting the app does not stop the adb server: wireless connections survive the exit and are reused on the next start.
- The "About" dialog shows the current version and the scrcpy version it targets (each with a copy button), the tech stack, and links to the website, GitHub, the changelog and support.
