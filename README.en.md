<p align="center">
  <img src="ui/assets/file2file_logo.png" width="96" alt="File2File">
</p>

<h1 align="center">File2File</h1>

<p align="center">
  <strong>A P2P desktop client built for large file transfers</strong><br>
  Direct peer connection · Encrypted by default · Free · Cross-platform
</p>

<p align="center">
  <a href="README.md">简体中文</a> ·
  <a href="README.en.md">English</a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-1d4ed8" alt="Platform">
  <img src="https://img.shields.io/badge/P2P-webrpc-0ea5e9" alt="webrpc">
  <img src="https://img.shields.io/badge/Tauri-2-24c8db" alt="Tauri">
  <img src="https://img.shields.io/badge/version-0.1.0-64748b" alt="Version">
</p>

---

## Overview

**File2File** is a desktop peer-to-peer transfer client built on [webrpc](https://webrpc.cn). After you log in with a Token, you can open a direct session with another peer: chat, send files, share screenshots, and start a voice call. Traffic goes over an encrypted channel. **No public IP is required, and nothing is stored on a central cloud drive.**

This is not another cloud-disk client. Files travel from your machine to theirs. The channel is encrypted by default. Anything from a screenshot to a multi-gigabyte disk image can go through the same session. It is meant for moving large files across devices, networks, and operating systems.

---

## Screenshots

<p align="center">
  <img src="assets/loginp.png" alt="Login" width="880">
</p>
<p align="center"><em>Login: join the webrpc network with a Token</em></p>

<p align="center">
  <img src="assets/sms.png" alt="Chat and file transfer" width="880">
</p>
<p align="center"><em>Session: text, screenshots, and large files on one encrypted channel (example: 2.76 GB at 3.93 MB/s average)</em></p>

<p align="center">
  <img src="assets/yuying.png" alt="Voice call" width="880">
</p>
<p align="center"><em>Voice: incoming-call dialog with Answer / Decline</em></p>

---

## Why File2File

| Strength | What it means |
| --- | --- |
| **P2P direct** | After the handshake, data goes peer to peer. No public IP. No upload-to-cloud-then-download detour. |
| **Secure by default** | The transfer channel is encrypted. Token plus an optional passphrase keeps uninvited peers out. |
| **Free** | The client is free. There is no per-transfer fee. |
| **Cross-platform** | macOS, Windows, and Linux (x86_64 / ARM64) share the same workflow. |
| **Chat + files** | One session carries messages, screenshots, and arbitrary files, with size, duration, and average speed on the card. |
| **Built for large files** | Designed for bulky payloads: GB-scale images, installers, and media packs. |
| **Voice calls** | Invite, answer, decline, hang up, and mute on a connected session, with echo cancellation. |

---

## Features

- **Session management**: create sessions, add remarks, keep local history across logins.
- **Instant messages**: Enter to send, Shift+Enter for a new line.
- **File transfer**: pick a file or drop it onto the window; receive progress in real time.
- **Screenshot send**: capture a region and send it to the current session; optionally hide this window first.
- **Voice calls**: invite, answer, decline, hang up, mute.
- **Saved accounts**: optionally remember the Token for next time.

---

## How to use

Both sides need File2File installed and a Token from [webrpc.cn](https://webrpc.cn).

### 1. Get a Token

Open [https://webrpc.cn](https://webrpc.cn), register, and copy your Token and password.

### 2. Log in

1. Launch File2File.
2. Enter **Token** and **password**.
3. **Auth passphrase** is optional. If both sides agree on the same passphrase, only peers who know it can connect.
4. Check **Save Token** if you want it filled in next time, then click **Login**.

The top bar then shows your Token, passphrase, login time, and the number of live sessions.

### 3. Create and connect a session

1. Click **New session**, enter the peer’s Token (and their passphrase if they set one).
2. Select the session and click **Connect**. Wait until the status reads **Connected**.
3. The peer must be online. If connect fails, check that their Token is online and that the network path is reachable.

### 4. Chat and send files

Once connected, use the composer at the bottom:

| Action | How |
| --- | --- |
| Send text | Type and press Enter |
| Send a file | Click **Select file**, or drop a file onto the window |
| Send a screenshot | Click **Screenshot** (macOS shortcut `⌃⌘A`); optionally hide this window first |
| Voice call | Click **Voice**; the peer sees Answer / Decline |

Transfer cards show bytes sent, elapsed time, and average speed. Incoming images can be previewed in place.

### 5. Sign out

**Log out** in the top-right disconnects current sessions and returns to the login page. Local session history is kept.

---

## Platforms

| Platform | Artifact | Notes |
| --- | --- | --- |
| macOS 11+ | `File2File.app` | Built on the host |
| Windows x64 | `File2File.exe` | WebView2 required (preinstalled on most Win10/11 systems) |
| Linux amd64 | `.deb` | Debian / Ubuntu and similar |
| Linux arm64 | `.deb` | e.g. ARM Linux, Raspberry Pi 64-bit |

Prebuilt packages land in `dist-tauri/` after you run a local build.

---

## Build from source

You need Rust (`cargo`) and [Tauri CLI 2](https://v2.tauri.app/). macOS also needs Xcode Command Line Tools. Cross-platform packaging needs Docker.

```bash
# Current host only
./build-tauri.sh

# All targets: macOS on the host, Linux / Windows in Docker
./build-all.sh

# A single target
./build-all.sh macos
./build-all.sh linux-arm64
./build-all.sh linux-amd64
./build-all.sh windows
```

Output directory: `dist-tauri/`.

```text
dist-tauri/
├── macos/File2File.app
├── linux-amd64/*.deb
├── linux-arm64/*.deb
└── windows-x64/File2File.exe
```

Dev mode:

```bash
cd src-tauri
cargo tauri dev
```

---

## Roadmap

- Live chat translation
- Video calls
- Further session and transfer UX improvements

---

## Stack

| Layer | Tech |
| --- | --- |
| Desktop shell | [Tauri 2](https://v2.tauri.app/) (Rust) |
| UI | HTML / CSS / JavaScript |
| P2P channel | [webrpc](https://webrpc.cn) SDK |
| Voice | cpal capture/playback + AEC3 echo cancellation |

---

## Links

- Get a Token: [https://webrpc.cn](https://webrpc.cn)
- Issues and ideas: open an Issue in this repository

> File2File only moves data between peers. It does not host your files. Keep your Token and passphrase private.
