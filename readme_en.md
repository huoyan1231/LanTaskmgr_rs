[English](./readme_en.md) | [简体中文](./readme.md)

# LanTaskmgr_rs

[C version](https://github.com/huoyan1231/LanTaskmgr)

A desktop tool that lets you **view and kill runaway processes on your PC from your phone's browser**,
over the same LAN. When Windows freezes and the desktop stops refreshing you can't open Task Manager,
but this app's lightweight built-in HTTP service can still respond — so you can "rescue" the machine
remotely.

This is a modern **Rust + Tauri v2** rewrite of the VB.NET/WinForms original
*Run Task Manager On Your Phone*, with better resource handling and a phone-friendly UI.

## Features

- **Phone rescue**: when the desktop is stuck, the WebView2 UI is frozen, but the HTTP service keeps
  answering — so you can still end the frozen process from your phone.
- **Lightweight mode** (default off): closing the window fully destroys the WebView2 window process and
  returns its memory to the system; the HTTP service keeps running for your phone. Open it again from
  the tray.
- **Grouped processes**: processes are grouped by image name; memory / CPU / window title are shown,
  sortable and filterable.
- **Safety**: critical system processes are rejected by the server; a wrong password 3 times from an IP
  bans that IP for 10 minutes.
- **Multi-language**: Simplified Chinese / English / Traditional Chinese.
- **Dark mode**, minimize-to-tray, launch-on-startup.

## Security warning (READ THIS)

- **Plain HTTP, no encryption.** The app talks to your phone in cleartext. Only use it on a trusted LAN.
- **Do NOT expose to the internet.** Never do port forwarding / DMZ on your router — anyone who reaches
  the port can see your process list (and, with a password, kill processes).
- The connection address shown in the UI is for **LAN use only**.
- A phone and the PC must be on the **same LAN/subnet**.

## Quick start (desktop)

1. Run `lantaskmgr_rs.exe`.
2. The desktop window shows: server status, current port, password, "launch on startup", lightweight
   mode, **bind to IP**, language, LAN address + QR code.
3. On your phone, open the browser and visit the address shown (scan the QR), log in if a password is
   set, then end the stubborn process.

## Settings file

Config is persisted to:

```
%APPDATA%\com.lantaskmgr.rs\config.json
```

Fields:

```json
{
  "password": "",          // empty = no login prompt
  "port": 5555,            // HTTP service port
  "language": "CN",        // CN | EN | ZHTW
  "autostart": false,      // launch on startup
  "bind": "0.0.0.0",       // bind address; 0.0.0.0 = listen on all interfaces (phones can reach it)
  "lightweight": false     // lightweight mode: closing the window destroys WebView2 and frees memory
}
```

### Bind to IP (prevent internet/foreign-network exposure)

`bind` decides which network interface the HTTP service listens on:

- Default `0.0.0.0`: listens on **all interfaces** — this is what lets the phone reach it across the LAN,
  and the only recommended setting for home use.
- A **specific LAN IP** (e.g. `192.168.1.50`): the service listens only on that address. On any other
  interface (e.g. a public Wi-Fi you connect to) the web UI simply does not appear — which at the source
  **cuts off the possibility of exposing the admin page to the internet / an untrusted network**.

> The desktop settings page adds a "Bind to IP" field: leaving it empty or entering an invalid format
> automatically falls back to `0.0.0.0`, so a typo cannot bind the service somewhere unexpected.
> Even when bound to a specific IP the traffic is still **cleartext, unencrypted** — do not use it on
> public networks, and never set up router port forwarding.

## Build

- Requires the **MSVC** toolchain + stable Rust (target `x86_64-pc-windows-msvc`).
- `cargo build --release` produces a single `lantaskmgr_rs.exe` under `target\release`.
- `windows` crate is pinned to `=0.61.3`, `tauri` / `tauri-build` to `2`.

## Notes / limitations

- Tested on Windows 10/11, x64.
- The phone UI loads on first access (lazy); startup only prints the LAN address.
- `SystemSettings.exe` and some system processes cannot be terminated from the UI (server-side reject).
- WebView2 must be installed (Edge WebView2 runtime) for the desktop window.

## License

MIT — see [LICENSE](./LICENSE).
