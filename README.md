<p align="center">
  <img src="ui/assets/file2file_logo.png" width="96" alt="File2File">
</p>

<h1 align="center">File2File</h1>

<p align="center">
  <strong>专为大文件而生的 P2P 传输桌面客户端</strong><br>
  点对点直连 · 默认加密 · 免费使用 · 跨平台
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

## 简介

**File2File** 是一款基于 [webrpc](https://webrpc.cn) 的桌面端点对点传输工具。用 Token 登录后，即可与对端建立直连会话：发消息、传文件、截图、语音通话，全程走加密通道，**不需要公网 IP，也不依赖中心化网盘**。

它不是又一个网盘客户端。文件从你的设备直接送到对方设备，通道默认加密，体积从截图到数 GB 镜像都能传。适合跨设备、跨网络、跨系统交换大文件。

---

## 界面预览

<p align="center">
  <img src="assets/loginp.png" alt="登录界面" width="880">
</p>
<p align="center"><em>登录：使用 webrpc Token 接入网络</em></p>

<p align="center">
  <img src="assets/sms.png" alt="聊天与文件传输" width="880">
</p>
<p align="center"><em>会话：文字、截图与大文件在同一条加密通道里传输（示例：2.76 GB，平均 3.93 MB/s）</em></p>

<p align="center">
  <img src="assets/yuying.png" alt="语音通话" width="880">
</p>
<p align="center"><em>语音：来电弹窗，一键接听或拒绝</em></p>

---

## 为什么选 File2File

| 亮点 | 说明 |
| --- | --- |
| **P2P 直连** | 两端打通后直传，无需公网 IP，也不必把文件先上传到云端。 |
| **默认加密** | 传输通道默认加密，Token + 可选认证口令，适合敏感文件。 |
| **完全免费** | 客户端免费，不向用户收取传输费用。 |
| **多平台** | macOS、Windows、Linux（x86_64 / ARM64）同一套体验。 |
| **聊天 + 传文件** | 会话里既能发消息，也能发截图和任意文件，进度、耗时、均速一目了然。 |
| **为大文件而生** | 针对大体积传输设计，GB 级镜像、安装包、素材包都能稳定送完。 |
| **语音通话** | 已连接的会话可直接发起语音，回声消除，接听 / 拒绝 / 静音。 |

---

## 功能一览

- **会话管理**：新建会话、备注、历史记录本地保存，下次登录可继续。
- **即时消息**：Enter 发送，Shift+Enter 换行。
- **文件传输**：选择文件或拖拽到窗口即可发送；接收进度实时显示。
- **截图发送**：框选屏幕后直接发到当前会话；可选择先隐藏本窗口再截。
- **语音通话**：邀请、接听、拒绝、挂断、静音。
- **账号缓存**：可选保存 Token，下次一键填入。

---

## 如何使用

两端都需要安装 File2File，并各自拥有 [webrpc.cn](https://webrpc.cn) 的 Token。

### 1. 获取 Token

打开 [https://webrpc.cn](https://webrpc.cn) 注册并获取 Token 与密码。

### 2. 登录

1. 启动 File2File。
2. 填写 **Token** 和 **密码**。
3. **认证口令**可选：双方约定同一口令后，只有知道口令的对端才能连上你。
4. 需要下次免填时勾选 **保存 Token**，然后点 **登录**。

登录成功后，顶栏会显示 Token、口令、登录时间和当前连接会话数。

### 3. 新建并连接会话

1. 左侧点 **新建会话**，填入对方的 Token（以及对方的认证口令，若对方设置了）。
2. 选中该会话，点 **连接**，等待状态变为 **已连接**。
3. 对方也需要在线；连接失败时请确认对方 Token 是否在线、网络是否可达。

### 4. 发消息、传文件

连接成功后即可使用底部输入栏：

| 操作 | 做法 |
| --- | --- |
| 发文字 | 输入后按 Enter |
| 发文件 | 点 **选择文件**，或把文件拖进窗口 |
| 发截图 | 点 **截图**（macOS 快捷键 `⌃⌘A`），可选「隐藏当前窗口后截图」 |
| 语音通话 | 点 **语音**，对方弹出接听 / 拒绝 |

传输卡片会显示已传大小、耗时和平均速度。收到的图片可直接预览。

### 5. 退出

右上角 **退出登录** 会断开当前会话并回到登录页。本地会话记录仍会保留。

---

## 平台与安装

| 平台 | 产物 | 说明 |
| --- | --- | --- |
| macOS 11+ | `File2File.app` | 本机编译 |
| Windows x64 | `File2File.exe` | 需系统已安装 WebView2（Win10/11 通常自带） |
| Linux amd64 | `.deb` | Debian / Ubuntu 等 |
| Linux arm64 | `.deb` | 如 Apple Silicon 上的 Linux、树莓派 64 位 |

预编译包位于 `dist-tauri/`（自行构建后生成）。

---

## 从源码构建

环境：Rust（含 `cargo`）、[Tauri CLI 2](https://v2.tauri.app/)。macOS 还需 Xcode Command Line Tools。跨平台打包需要 Docker。

```bash
# 仅编译当前平台
./build-tauri.sh

# 全平台：本机编 macOS，Docker 编 Linux / Windows
./build-all.sh

# 只编某一个目标
./build-all.sh macos
./build-all.sh linux-arm64
./build-all.sh linux-amd64
./build-all.sh windows
```

产物目录：`dist-tauri/`。

```text
dist-tauri/
├── macos/File2File.app
├── linux-amd64/*.deb
├── linux-arm64/*.deb
└── windows-x64/File2File.exe
```

开发调试：

```bash
cd src-tauri
cargo tauri dev
```

---

## 后续规划

- 聊天实时翻译
- 视频通话
- 更多会话与传输体验优化

---

## 技术栈

| 层 | 技术 |
| --- | --- |
| 桌面壳 | [Tauri 2](https://v2.tauri.app/)（Rust） |
| 界面 | HTML / CSS / JavaScript |
| P2P 通道 | [webrpc](https://webrpc.cn) SDK |
| 语音 | cpal 采集播放 + AEC3 回声消除 |

---

## 相关链接

- 获取 Token：[https://webrpc.cn](https://webrpc.cn)
- 问题与建议：欢迎在本仓库提交 Issue

> File2File 只做直连传输，不托管你的文件。请自行保管 Token 与口令，不要分享给不可信的人。
