<p align="center">
  <img src="ui/assets/file2file_logo.png" width="96" alt="File2File logo">
</p>

<h1 align="center">File2File</h1>

<p align="center">
  <strong>P2P file transfer, personal NAS, and remote desktop in one client</strong><br>
  Direct peer connection · Encrypted by default · Free · Windows / macOS / Linux
</p>

<p align="center">
  A desktop app for large files, cross-device work, and access to a disk you run yourself. Log in with a webrpc Token to chat, send files, manage your own drive, or remotely control a Windows PC.
</p>

<p align="center">
  <a href="README.md">简体中文</a> ·
  <a href="README.en.md">English</a>
</p>

<p align="center">
  <a href="https://github.com/xiaoming-software/File2File-Desktop/tree/main/dist-tauri"><img src="https://img.shields.io/badge/download-prebuilt-16a34a" alt="Download prebuilt binaries"></a>
  <img src="https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-1d4ed8" alt="macOS, Windows, Linux">
  <img src="https://img.shields.io/badge/P2P-webrpc-0ea5e9" alt="webrpc">
  <img src="https://img.shields.io/badge/Tauri-2-24c8db" alt="Tauri 2">
  <img src="https://img.shields.io/badge/version-0.1.0-64748b" alt="Version 0.1.0">
</p>

<p align="center">
  <a href="https://github.com/xiaoming-software/File2File-Desktop/tree/main/dist-tauri"><strong>Download a compiled build and run it →</strong></a>
</p>

---

## Contents

- [What File2File is](#what-file2file-is)
- [Who it is for](#who-it-is-for)
- [Screenshots](#screenshots)
- [Features](#features)
  - [Chat and large file transfer](#chat-and-large-file-transfer)
  - [Personal drive / NAS](#personal-drive--nas)
  - [Remote desktop](#remote-desktop)
  - [Voice and screenshots](#voice-and-screenshots)
- [Download and run](#download-and-run)
- [How to use](#how-to-use)
- [Current limits](#current-limits)
- [Build from source](#build-from-source)
- [FAQ](#faq)
- [Stack](#stack)
- [Roadmap](#roadmap)

---

## What File2File is

**File2File** is a cross-platform desktop client on [webrpc](https://webrpc.cn). One window covers three jobs:

1. **Peer-to-peer transfer** — chat and send files of any size after a direct session. Nothing has to sit on a public cloud first.
2. **Personal drive** — connect to your own `mywebdisk-server` and browse, upload, download, and search a folder you choose, like a file manager.
3. **Remote desktop** — view and mouse-control another computer’s screen on a connected PC session (Windows can be controlled; **macOS cannot be the controlled side yet**).

The channel is encrypted by default. **No public IP is required.** Files are not hosted on a central drive. Your Token is your identity; an optional passphrase keeps strangers out.

This is not a reskinned consumer cloud disk, and it is not a paid remote-control suite. The client is free. Prebuilt binaries are in the repository.

---

## Who it is for

- People who move **gigabyte installers, images, and media packs** across Windows, Mac, and Linux
- Anyone who wants to reach a **home PC or NAS folder** from outside, and keep the data on a path they control
- Cases where you need to **look at and click** a Windows desktop for a while, without installing a separate remote tool
- Families or small teams who want chat, files, and an occasional voice call in one place

---

## Screenshots

<p align="center">
  <img src="assets/loginp.png" alt="File2File login screen using a webrpc Token" width="880">
</p>
<p align="center"><em>Login: join the webrpc network with a Token</em></p>

<p align="center">
  <img src="assets/sms.png" alt="File2File chat session with large file transfer progress" width="880">
</p>
<p align="center"><em>Session: text, screenshots, and large files on one encrypted channel (example: 2.76 GB at 3.93 MB/s)</em></p>

<p align="center">
  <img src="assets/yuying.png" alt="File2File incoming voice call dialog" width="880">
</p>
<p align="center"><em>Voice: incoming-call dialog with Answer / Decline</em></p>

<p align="center">
  <img src="assets/mywebdisk-server-run.png" alt="Starting mywebdisk-server from a terminal for a personal NAS" width="880">
</p>
<p align="center"><em>Personal NAS server: start mywebdisk-server on the machine that holds the files</em></p>

<p align="center">
  <img src="assets/mywebdisk-client.png" alt="File2File drive explorer connected to a home NAS with capacity and file grid" width="880">
</p>
<p align="center"><em>Drive explorer: after you connect your own disk, browse capacity, upload, download, and search</em></p>

<p align="center">
  <img src="assets/yuancheng-win.png" alt="File2File remote desktop controlling a Windows PC with fullscreen and end buttons" width="880">
</p>
<p align="center"><em>Remote desktop: control a Windows PC, with in-app fullscreen and mouse input</em></p>

---

## Features

### Chat and large file transfer

- Session remarks and local history across logins
- Enter to send, Shift+Enter for a new line
- Pick a file or drop it on the window; cards show size, time, and average speed
- Built for large payloads so multi-gigabyte transfers can finish in one go

### Personal drive / NAS

Run **[mywebdisk-server](https://github.com/xiaoming-software/mywebdisk)** on a home PC, spare box, or any folder. Add that Token under **Drive connections** in File2File. Data stays in the directory you pass to the server. Reads and writes use the same encrypted webrpc channel.

Source and prebuilt binaries for each platform are in that repository — download and run, no compile required:

**https://github.com/xiaoming-software/mywebdisk**

The explorer supports:

| Action | What you get |
| --- | --- |
| Browse | List / icon views, breadcrumbs, total and free space |
| Upload | Button upload, or drop local files into the current folder |
| Download | Context-menu download; a task panel for progress and retry |
| Organize | New item, rename, move, delete, zip |
| Search | Search box at the top of the drive view |
| Multi-select | Marquee select and batch move |

Example server command (on the machine that stores the files):

```bash
./mywebdisk-server \
  --token=<webrpc Token> \
  --passwd=<Token password> \
  --permission=<passphrase you choose> \
  --path=<local folder to share>
```

When the log says the service is ready, add a drive connection in File2File with **the same Token**.

### Remote desktop

On a connected **computer session** (not a drive session), click **Remote control**. After the peer accepts, their desktop appears.

- **Fullscreen** — enlarge the picture inside the File2File window
- **Mouse** — move, left click, right click, double-click, scroll
- **Stats** — resolution, FPS, and KB/s in the header
- **Direction** — you control *their* PC; you are not sharing your own screen

See [Current limits](#current-limits) for what is not supported yet.

### Voice and screenshots

- Voice: invite, answer, decline, hang up, mute, with echo cancellation
- Screenshots: capture a region and send it to the session; optionally hide this window first (macOS shortcut `⌃⌘A`)

---

## Why File2File

| Strength | What it means |
| --- | --- |
| **P2P direct** | After the handshake, data goes peer to peer. No public IP. No upload-then-download detour. |
| **Encrypted by default** | The transfer channel is encrypted. Token plus an optional passphrase. |
| **Your disk, your path** | You pick the folder and the machine. File2File only connects and reads/writes. |
| **Remote control in the same app** | Chat, files, and remote desktop without switching tools. |
| **Free** | No per-transfer fee for the client. |
| **Cross-platform** | macOS, Windows, and Linux (x86_64 / ARM64) share the same flow. |
| **Ready to run** | [Prebuilt binaries live in the repo](https://github.com/xiaoming-software/File2File-Desktop/tree/main/dist-tauri). |

---

## Download and run

You do not need Rust or Tauri installed. Open the prebuilt folder and take the file for your OS:

**https://github.com/xiaoming-software/File2File-Desktop/tree/main/dist-tauri**

| Platform | Path | Notes |
| --- | --- | --- |
| Windows 10/11 x64 | [`dist-tauri/windows-x64/File2File.exe`](https://github.com/xiaoming-software/File2File-Desktop/tree/main/dist-tauri/windows-x64) | WebView2 required (usually already on Win10/11) |
| macOS 11+ | [`dist-tauri/macos/File2File.app`](https://github.com/xiaoming-software/File2File-Desktop/tree/main/dist-tauri/macos) | If Gatekeeper blocks it, allow the app under Privacy & Security |
| Linux amd64 | [`.deb` in `dist-tauri/linux-amd64/`](https://github.com/xiaoming-software/File2File-Desktop/tree/main/dist-tauri/linux-amd64) | Debian / Ubuntu and similar |
| Linux arm64 | [`.deb` in `dist-tauri/linux-arm64/`](https://github.com/xiaoming-software/File2File-Desktop/tree/main/dist-tauri/linux-arm64) | ARM boards, 64-bit Raspberry Pi |

The personal drive also needs `mywebdisk-server` on the machine that holds the files. File2File is the client. Download a prebuilt server from:

**https://github.com/xiaoming-software/mywebdisk**

That repo includes macOS / Linux / Windows binaries and the source. When you use File2File as the drive client, you only need **mywebdisk-server**, not the separate MyWebDisk desktop app in the same repo.

---

## How to use

Every peer (and the NAS server) needs a Token from [webrpc.cn](https://webrpc.cn).

### 1. Get a Token

Open [https://webrpc.cn](https://webrpc.cn), register, and copy the **Token** and **password**.

### 2. Log in to File2File

1. Launch the downloaded app.
2. Enter Token and password.
3. **Auth passphrase** is optional. If both sides use the same one, unknown peers cannot connect.
4. Check **Save Token** if you want it filled next time, then log in.

The top bar shows Token, passphrase, login time, and live session count.

### 3. Chat and send files to another PC

1. Under **Chat sessions**, create a session with the peer Token (and their passphrase if they set one).
2. Select it, click **Connect**, wait until it shows connected.
3. The peer must be online.

| Action | How |
| --- | --- |
| Send text | Type and press Enter |
| Send a file | **Select file**, or drop a file on the window |
| Send a screenshot | **Screenshot** |
| Voice | **Voice**; the peer answers or declines |
| Remote control | **Remote control** on a computer session, after they accept |

### 4. Connect your personal drive

1. Download `mywebdisk-server` for your OS from [mywebdisk](https://github.com/xiaoming-software/mywebdisk). Start it on the machine you want to share: `--path` is the folder, `--token` is that machine’s webrpc Token.
2. In File2File, **Drive connections → New**, enter the **drive Token** (same as the server `--token`).
3. When connected, use the explorer: capacity, folders, upload, download, search.
4. Drive sessions cannot start remote desktop. Remote control is computer-to-computer only.

### 5. Control a Windows desktop

1. Connect a computer session as in step 3.
2. Click **Remote control**. The peer gets a prompt; after they accept, the screen appears.
3. Use the mouse. Click **Fullscreen** to enlarge.
4. Click **End** to stop.

If the peer is **macOS**, File2File reports that controlling a Mac desktop is not supported yet.

### 6. Sign out

**Log out** in the top-right disconnects and returns to login. Local session history stays on disk.

---

## Current limits

These are product limits in this version:

- Remote desktop has **no keyboard input** yet — mouse only.
- **macOS cannot be remotely controlled.** A Mac can still control a Windows PC.
- Remote desktop works on **computer sessions only**, not drive / NAS sessions.
- Some higher-privilege windows on the remote OS may not accept clicks. That is an OS rule, not a missing button in File2File.
- The personal drive needs `mywebdisk-server` running. If the server is down, the client cannot connect.

---

## Build from source

Most people should use the prebuilt files above. Developers need Rust (`cargo`) and [Tauri CLI 2](https://v2.tauri.app/). macOS also needs Xcode Command Line Tools. Cross-platform packaging needs Docker.

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

## FAQ

**How is this different from a normal cloud drive?**  
A typical cloud drive stores files on a vendor’s machines. File2File chat transfers go device to device. The personal-drive feature talks to **your** `mywebdisk-server` and a folder you choose.

**Do I have to compile it?**  
No. Use the binaries in [dist-tauri](https://github.com/xiaoming-software/File2File-Desktop/tree/main/dist-tauri).

**Does it work without a public IP?**  
Yes. webrpc does not require you to expose a port or own a static public address.

**What if I lose the Token?**  
Follow the process on [webrpc.cn](https://webrpc.cn). Do not share the Token or passphrase with people you do not trust.

**Why does controlling a Mac show “not supported”?**  
This release does not allow a Mac to be the controlled desktop. Controlling Windows is unchanged.

**Can I type on the remote machine?**  
Not yet. Mouse only; keyboard input is on the roadmap.

**The drive will not connect.**  
Download and start `mywebdisk-server` for your OS from [mywebdisk](https://github.com/xiaoming-software/mywebdisk). Confirm the service is ready, Token and passphrase match, both sides are logged into webrpc, and `--path` exists and is writable.

---

## Stack

| Layer | Tech |
| --- | --- |
| Desktop shell | [Tauri 2](https://v2.tauri.app/) (Rust) |
| UI | HTML / CSS / JavaScript |
| P2P channel | [webrpc](https://webrpc.cn) SDK |
| Voice | cpal capture/playback + AEC3 echo cancellation |
| Remote video | Screen capture + H.264, drawn on a canvas |

---

## Roadmap

- Keyboard input for remote desktop
- macOS as a controlled desktop
- Live chat translation
- Video calls
- More session and transfer polish

---

## Links

- File2File prebuilt downloads: [dist-tauri](https://github.com/xiaoming-software/File2File-Desktop/tree/main/dist-tauri)
- Personal NAS server mywebdisk (source and prebuilt binaries): [xiaoming-software/mywebdisk](https://github.com/xiaoming-software/mywebdisk)
- Get a Token: [https://webrpc.cn](https://webrpc.cn)
- Chinese readme: [README.md](README.md)
- Issues and ideas: open an Issue in this repository

---

## Topics

File2File, P2P file transfer, peer-to-peer large files, webrpc, personal NAS, personal cloud drive, remote desktop, remote PC control, cross-platform desktop client, free remote control, no public IP

> File2File only moves data between peers. It does not host your files. Keep your Token and passphrase private. Remote desktop shows the other screen — only connect to people you trust.
