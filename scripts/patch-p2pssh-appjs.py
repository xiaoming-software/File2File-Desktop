#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Re-apply P2P SSH UI hooks onto a clean UTF-8 js/app.js."""

from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
APP_JS = ROOT / "js" / "app.js"


def must_replace(src: str, old: str, new: str, label: str) -> str:
    count = src.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected 1 match, got {count}")
    return src.replace(old, new, 1)


def main() -> None:
    src = APP_JS.read_text(encoding="utf-8")
    if "登录" not in src:
        raise SystemExit("app.js missing Chinese before patch; abort")
    if "isP2pSsh" in src:
        raise SystemExit("app.js already patched; abort")

    src = must_replace(
        src,
        """  const driveListEl = document.getElementById("drive-list");
  const panelUnselected = document.getElementById("panel-unselected");
  const panelConnect = document.getElementById("panel-connect");
  const panelDrive = document.getElementById("panel-drive");
  const chatMain = document.getElementById("chat-main");""",
        """  const driveListEl = document.getElementById("drive-list");
  const p2psshListEl = document.getElementById("p2pssh-list");
  const panelUnselected = document.getElementById("panel-unselected");
  const panelConnect = document.getElementById("panel-connect");
  const panelDrive = document.getElementById("panel-drive");
  const panelP2pssh = document.getElementById("panel-p2pssh");
  const chatMain = document.getElementById("chat-main");
  const p2psshTitle = document.getElementById("p2pssh-title");
  const p2psshSubtitle = document.getElementById("p2pssh-subtitle");
  const p2psshCmd = document.getElementById("p2pssh-cmd");
  const p2psshTabs = document.getElementById("p2pssh-tabs");
  const p2psshTermHost = document.getElementById("p2pssh-term-host");
  const p2psshTermMap = {};
  const p2psshTermPending = {};""",
        "dom refs",
    )

    src = must_replace(
        src,
        """  const modalNewDrive = document.getElementById("modal-new-drive");""",
        """  const modalNewDrive = document.getElementById("modal-new-drive");
  const modalNewP2pssh = document.getElementById("modal-new-p2pssh");
  const newP2psshToken = document.getElementById("new-p2pssh-token");
  const newP2psshPass = document.getElementById("new-p2pssh-pass");
  const newP2psshRemark = document.getElementById("new-p2pssh-remark");
  const newP2psshError = document.getElementById("new-p2pssh-error");""",
        "modal refs",
    )

    src = must_replace(
        src,
        """    sessions: [],
    drives: [],
    selectedId: null,""",
        """    sessions: [],
    drives: [],
    p2pssh: [],
    selectedId: null,""",
        "state",
    )

    src = must_replace(
        src,
        """  const MYWEBDISK_URL = "https://github.com/xiaoming-software/mywebdisk";
  const DRIVE_SERVER_HINT_KEY =
    "若尚未部署网盘服务端，请先{link}，在家里的 NAS 或电脑上安装并保持运行。然后输入服务端 webrpc Token；认证口令可留空，连接时仍可修改。";""",
        """  const MYWEBDISK_URL = "https://github.com/xiaoming-software/mywebdisk";
  const P2PSSH_SERVER_URL = "https://github.com/xiaoming-software/p2pssh-server";
  const DRIVE_SERVER_HINT_KEY =
    "若尚未部署网盘服务端，请先{link}，在家里的 NAS 或电脑上安装并保持运行。然后输入服务端 webrpc Token；认证口令可留空，连接时仍可修改。";
  const P2PSSH_SERVER_HINT_KEY =
    "若尚未部署 SSH 转发服务端，请先{link}，在目标设备上安装并保持运行。然后输入服务端 webrpc Token；认证口令可留空，连接时仍可修改。";""",
        "urls",
    )

    src = must_replace(
        src,
        """  function driveServerHintHtml() {
    return t(DRIVE_SERVER_HINT_KEY, {
      link:
        '<a href="' +
        MYWEBDISK_URL +
        '" target="_blank" rel="noopener noreferrer">' +
        t("下载 MyWebDisk") +
        "</a>",
    });
  }""",
        """  function driveServerHintHtml() {
    return t(DRIVE_SERVER_HINT_KEY, {
      link:
        '<a href="' +
        MYWEBDISK_URL +
        '" target="_blank" rel="noopener noreferrer">' +
        t("下载 MyWebDisk") +
        "</a>",
    });
  }

  function p2psshServerHintHtml() {
    return t(P2PSSH_SERVER_HINT_KEY, {
      link:
        '<a href="' +
        P2PSSH_SERVER_URL +
        '" target="_blank" rel="noopener noreferrer">' +
        t("下载 p2pssh-server") +
        "</a>",
    });
  }""",
        "hint",
    )

    src = must_replace(
        src,
        """        window.__TAURI__.event.listen("webrpc-session-dead", function (event) {""",
        """        window.__TAURI__.event.listen("p2pssh-disconnected", function (event) {
          onP2psshDisconnected(event && event.payload);
        });
        window.__TAURI__.event.listen("p2pssh-term-data", function (event) {
          onP2psshTermData(event && event.payload);
        });
        window.__TAURI__.event.listen("p2pssh-term-exit", function (event) {
          onP2psshTermExit(event && event.payload);
        });
        window.__TAURI__.event.listen("webrpc-session-dead", function (event) {""",
        "events",
    )

    src = must_replace(
        src,
        """    state.sessions = [];
    state.drives = [];
    state.selectedId = null;
    state.pendingFile = null;
    state.connectFillId = null;
    state.sessionsReady = false;
    state.pendingHellos = [];
    state.pendingTexts = [];
    state.inflightFiles = {};
    applySdkSessionCount(0);""",
        """    state.sessions = [];
    state.drives = [];
    state.p2pssh = [];
    state.selectedId = null;
    state.pendingFile = null;
    state.connectFillId = null;
    state.sessionsReady = false;
    state.pendingHellos = [];
    state.pendingTexts = [];
    state.inflightFiles = {};
    applySdkSessionCount(0);""",
        "enter clear",
    )

    src = must_replace(
        src,
        """    Promise.all([loadSavedSessions(), loadSavedDrives()]).then(function () {
      if (!state.selectedId) {
        const first = state.sessions[0] || state.drives[0];
        state.selectedId = first ? first.id : null;
      }
      renderAccount();
      renderWorkspace();
    });""",
        """    Promise.all([loadSavedSessions(), loadSavedDrives(), loadSavedP2pSsh()]).then(function () {
      if (!state.selectedId) {
        const first = state.sessions[0] || state.drives[0] || state.p2pssh[0];
        state.selectedId = first ? first.id : null;
      }
      renderAccount();
      renderWorkspace();
    });""",
        "enter load",
    )

    src = must_replace(
        src,
        """    state.sessions = [];
    state.drives = [];
    state.selectedId = null;
    state.pendingFile = null;
    state.connectFillId = null;
    state.sessionsReady = false;
    state.pendingHellos = [];
    state.pendingTexts = [];
    state.inflightFiles = {};
    state.sendQueue = [];
    state.sendActiveId = null;""",
        """    state.sessions = [];
    state.drives = [];
    state.p2pssh = [];
    disposeAllP2psshTerms();
    state.selectedId = null;
    state.pendingFile = null;
    state.connectFillId = null;
    state.sessionsReady = false;
    state.pendingHellos = [];
    state.pendingTexts = [];
    state.inflightFiles = {};
    state.sendQueue = [];
    state.sendActiveId = null;""",
        "logout clear",
    )

    src = must_replace(
        src,
        """  function isDrive(item) {
    return !!(item && item.kind === "drive");
  }

  function loadSavedSessions() {""",
        """  function isDrive(item) {
    return !!(item && item.kind === "drive");
  }

  function isP2pSsh(item) {
    return !!(item && item.kind === "p2pssh");
  }

  function hydrateP2pSsh(item) {
    return {
      id: item.id || uid(),
      kind: "p2pssh",
      peerToken: item.peerToken || "",
      remark: item.remark || "",
      connected: false,
      connecting: false,
      connectError: "",
      peerPass: item.peerPass || "",
      rpcSessionId: 0,
      localPort: 0,
      sshCommand: "",
      terms: [],
      activeTermId: null,
      termSeq: 0,
    };
  }

  function loadSavedP2pSsh() {
    if (!ownerToken()) {
      state.p2pssh = [];
      return Promise.resolve();
    }
    return tauriInvoke("saved_p2pssh_list", { ownerToken: ownerToken() })
      .then(function (items) {
        state.p2pssh = (Array.isArray(items) ? items : []).map(hydrateP2pSsh);
      })
      .catch(function () {
        state.p2pssh = [];
      });
  }

  function persistP2pSshCreate(item) {
    return tauriInvoke("saved_p2pssh_create", {
      ownerToken: ownerToken(),
      peerToken: item.peerToken,
      peerPass: item.peerPass || "",
      remark: item.remark || "",
    });
  }

  function persistP2pSshUpdate(item) {
    if (!item) return Promise.resolve();
    return tauriInvoke("saved_p2pssh_update", {
      ownerToken: ownerToken(),
      id: item.id,
      peerToken: item.peerToken,
      peerPass: item.peerPass || "",
      remark: item.remark || "",
    }).catch(function (err) {
      if (err && err.message === "webrpc-unavailable") return;
    });
  }

  function persistP2pSshDelete(id) {
    return tauriInvoke("saved_p2pssh_delete", {
      ownerToken: ownerToken(),
      id: id,
    }).catch(function (err) {
      if (err && err.message === "webrpc-unavailable") return;
    });
  }

  function loadSavedSessions() {""",
        "hydrate",
    )

    src = must_replace(
        src,
        """  function persistItemUpdate(item) {
    if (isDrive(item)) return persistDriveUpdate(item);
    return persistSessionUpdate(item);
  }""",
        """  function persistItemUpdate(item) {
    if (isDrive(item)) return persistDriveUpdate(item);
    if (isP2pSsh(item)) return persistP2pSshUpdate(item);
    return persistSessionUpdate(item);
  }""",
        "persistItemUpdate",
    )

    src = must_replace(
        src,
        """    document.getElementById("btn-new-drive").addEventListener("click", function () {
      openModal("new-drive");
    });
    bindSidebarSplit();
    bindNasExplorer();
    document.getElementById("btn-create-session").addEventListener("click", createSession);
    document.getElementById("btn-create-drive").addEventListener("click", createDrive);""",
        """    document.getElementById("btn-new-drive").addEventListener("click", function () {
      openModal("new-drive");
    });
    const btnNewP2pssh = document.getElementById("btn-new-p2pssh");
    if (btnNewP2pssh) {
      btnNewP2pssh.addEventListener("click", function () {
        openModal("new-p2pssh");
      });
    }
    bindSidebarSplit();
    bindNasExplorer();
    bindP2psshPanel();
    document.getElementById("btn-create-session").addEventListener("click", createSession);
    document.getElementById("btn-create-drive").addEventListener("click", createDrive);
    const btnCreateP2pssh = document.getElementById("btn-create-p2pssh");
    if (btnCreateP2pssh) btnCreateP2pssh.addEventListener("click", createP2pSsh);""",
        "bind buttons",
    )

    src = must_replace(
        src,
        """    if (driveListEl) driveListEl.addEventListener("click", onSessionListClick);""",
        """    if (driveListEl) driveListEl.addEventListener("click", onSessionListClick);
    if (p2psshListEl) p2psshListEl.addEventListener("click", onSessionListClick);""",
        "list click",
    )

    src = must_replace(
        src,
        """      if (remarkTitle) remarkTitle.textContent = isDrive(session) ? t("设置网盘备注") : t("设置会话备注");
      remarkInput.placeholder = isDrive(session) ? t("例如：家里 NAS") : t("例如：小明");""",
        """      if (remarkTitle) {
        remarkTitle.textContent = isDrive(session)
          ? t("设置网盘备注")
          : isP2pSsh(session)
            ? t("设置 SSH 备注")
            : t("设置会话备注");
      }
      remarkInput.placeholder = isDrive(session)
        ? t("例如：家里 NAS")
        : isP2pSsh(session)
          ? t("例如：家里 Linux")
          : t("例如：小明");""",
        "remark",
    )

    src = must_replace(
        src,
        """      openConfirm(isDrive(session) ? "delete-drive" : "delete", id);""",
        """      openConfirm(
        isDrive(session) ? "delete-drive" : isP2pSsh(session) ? "delete-p2pssh" : "delete",
        id
      );""",
        "confirm kind",
    )

    src = must_replace(
        src,
        """  function renderWorkspace() {
    renderAccount();
    renderSessionList();
    renderDriveList();
    renderChat();
  }""",
        """  function renderWorkspace() {
    renderAccount();
    renderSessionList();
    renderDriveList();
    renderP2pSshList();
    renderChat();
  }""",
        "renderWorkspace",
    )

    src = must_replace(
        src,
        """  function renderDriveList() {
    if (!driveListEl) return;
    renderPeerList(driveListEl, state.drives, {
      empty: t("暂无连接<br />点击「新建连接」连接家里的 NAS"),
      closeLabel: t("关闭连接"),
      deleteLabel: t("删除连接"),
      showClear: false,
      glyph: "drive",
    });
  }""",
        """  function renderDriveList() {
    if (!driveListEl) return;
    renderPeerList(driveListEl, state.drives, {
      empty: t("暂无连接<br />点击「新建连接」连接家里的 NAS"),
      closeLabel: t("关闭连接"),
      deleteLabel: t("删除连接"),
      showClear: false,
      glyph: "drive",
    });
  }

  function renderP2pSshList() {
    if (!p2psshListEl) return;
    renderPeerList(p2psshListEl, state.p2pssh, {
      empty: t("暂无隧道<br />点击「新建隧道」连接远程 SSH"),
      closeLabel: t("关闭隧道"),
      deleteLabel: t("删除隧道"),
      showClear: false,
      glyph: "ssh",
    });
  }""",
        "renderP2pSshList",
    )

    src = must_replace(
        src,
        """        const glyphClass = opts.glyph === "drive" ? "drive-glyph" : "chat-glyph";""",
        """        const glyphClass =
          opts.glyph === "drive" ? "drive-glyph" : opts.glyph === "ssh" ? "ssh-glyph" : "chat-glyph";""",
        "glyph",
    )

    src = must_replace(
        src,
        """  function showPanel(name) {
    panelUnselected.classList.toggle("is-visible", name === "unselected");
    panelConnect.classList.toggle("is-visible", name === "connect");
    if (panelDrive) panelDrive.classList.toggle("is-visible", name === "drive");
    chatMain.classList.toggle("is-visible", name === "chat");
  }""",
        """  function showPanel(name) {
    panelUnselected.classList.toggle("is-visible", name === "unselected");
    panelConnect.classList.toggle("is-visible", name === "connect");
    if (panelDrive) panelDrive.classList.toggle("is-visible", name === "drive");
    if (panelP2pssh) panelP2pssh.classList.toggle("is-visible", name === "p2pssh");
    chatMain.classList.toggle("is-visible", name === "chat");
  }""",
        "showPanel",
    )

    src = must_replace(
        src,
        """      if (
        !isDrive(session) &&
        (session.chatsLoaded || session.chatsLoading || (session.messages && session.messages.length))
      ) {
        unloadSessionChats(session);
      }
      showPanel("connect");
      if (connectMark) {
        connectMark.classList.toggle("is-drive", isDrive(session));
        connectMark.classList.toggle("is-chat", !isDrive(session));
      }
      if (connectTitle) {
        connectTitle.textContent = isDrive(session) ? t("请先连接网盘") : t("请先建立 P2P 连接");
      }
      if (connectDesc) {
        if (isDrive(session)) {
          connectDesc.innerHTML = driveServerHintHtml();
        } else {
          connectDesc.textContent = t(
            "当前会话尚未与对端连通，连接成功后即可收发消息和文件。"
          );
        }
      }
      const name = session.remark || session.peerToken;
      connectPeer.textContent = (isDrive(session) ? t("网盘 Token：") : t("对方 Token：")) + session.peerToken;""",
        """      if (
        !isDrive(session) &&
        !isP2pSsh(session) &&
        (session.chatsLoaded || session.chatsLoading || (session.messages && session.messages.length))
      ) {
        unloadSessionChats(session);
      }
      showPanel("connect");
      if (connectMark) {
        connectMark.classList.toggle("is-drive", isDrive(session));
        connectMark.classList.toggle("is-ssh", isP2pSsh(session));
        connectMark.classList.toggle("is-chat", !isDrive(session) && !isP2pSsh(session));
      }
      if (connectTitle) {
        connectTitle.textContent = isDrive(session)
          ? t("请先连接网盘")
          : isP2pSsh(session)
            ? t("请先连接 P2P SSH")
            : t("请先建立 P2P 连接");
      }
      if (connectDesc) {
        if (isDrive(session)) {
          connectDesc.innerHTML = driveServerHintHtml();
        } else if (isP2pSsh(session)) {
          connectDesc.innerHTML = p2psshServerHintHtml();
        } else {
          connectDesc.textContent = t(
            "当前会话尚未与对端连通，连接成功后即可收发消息和文件。"
          );
        }
      }
      const name = session.remark || session.peerToken;
      connectPeer.textContent =
        (isDrive(session) ? t("网盘 Token：") : isP2pSsh(session) ? t("SSH Token：") : t("对方 Token：")) +
        session.peerToken;""",
        "connect texts",
    )

    src = must_replace(
        src,
        """    if (isDrive(session)) {
      showPanel("drive");
      if (!session.nasWatching) startNasWatch(session);
      renderNasExplorer(session);
      return;
    }

    showPanel("chat");""",
        """    if (isDrive(session)) {
      showPanel("drive");
      if (!session.nasWatching) startNasWatch(session);
      renderNasExplorer(session);
      return;
    }

    if (isP2pSsh(session)) {
      showPanel("p2pssh");
      renderP2pSshPanel(session);
      return;
    }

    showPanel("chat");""",
        "connected panel",
    )

    src = must_replace(
        src,
        """  function hideMenu() {
    if (!state.menuSessionId) return;
    state.menuSessionId = null;
    renderSessionList();
    renderDriveList();
  }""",
        """  function hideMenu() {
    if (!state.menuSessionId) return;
    state.menuSessionId = null;
    renderSessionList();
    renderDriveList();
    renderP2pSshList();
  }""",
        "hideMenu",
    )

    src = must_replace(
        src,
        """  function connectSelected() {
    const session = findItem(state.selectedId);
    if (!session || session.connected || session.connecting) return;
    session.peerPass = connectPeerPass.value.trim();
    session.connectError = "";
    session.connecting = true;
    renderWorkspace();

    const localId = session.id;
    const failText = isDrive(session)
      ? t("连接失败。请确认网盘 Token 是否在线，以及当前网络是否可达。")
      : t("连接失败。请确认对方 Token 是否在线，以及当前网络是否可达。");
    tauriInvoke("webrpc_open_session", {""",
        """  function connectSelected() {
    const session = findItem(state.selectedId);
    if (!session || session.connected || session.connecting) return;
    session.peerPass = connectPeerPass.value.trim();
    session.connectError = "";
    session.connecting = true;
    renderWorkspace();

    const localId = session.id;
    if (isP2pSsh(session)) {
      const failText = t(
        "连接失败。请确认 p2pssh-server Token 是否在线，以及当前网络是否可达。"
      );
      tauriInvoke("p2pssh_connect", {
        ownerToken: ownerToken(),
        id: session.id,
        peerPass: session.peerPass || "",
      })
        .then(function (info) {
          const current = findItem(localId);
          if (!current || !current.connecting) {
            if (info && info.id) {
              tauriInvoke("p2pssh_disconnect", { id: info.id }).catch(function () {});
            }
            return;
          }
          if (!info || !info.localPort) {
            current.connecting = false;
            current.connected = false;
            current.rpcSessionId = 0;
            current.connectError = failText;
            renderWorkspace();
            return;
          }
          current.connecting = false;
          current.connected = true;
          current.rpcSessionId = Number(info.webrpcSessionId) || 0;
          current.localPort = Number(info.localPort) || 0;
          current.sshCommand = info.sshCommand || ("ssh -p " + current.localPort + " user@127.0.0.1");
          current.connectError = "";
          persistItemUpdate(current);
          renderWorkspace();
        })
        .catch(function (err) {
          const current = findItem(localId);
          if (!current) return;
          current.connecting = false;
          current.connected = false;
          current.rpcSessionId = 0;
          current.localPort = 0;
          current.sshCommand = "";
          current.connectError =
            invokeErrorText(err).indexOf("handshake-send-failed") >= 0
              ? t("会话通信异常，通知消息未能送达，连接已关闭。请检查网络后重试。")
              : failText;
          renderWorkspace();
        });
      return;
    }

    const failText = isDrive(session)
      ? t("连接失败。请确认网盘 Token 是否在线，以及当前网络是否可达。")
      : t("连接失败。请确认对方 Token 是否在线，以及当前网络是否可达。");
    tauriInvoke("webrpc_open_session", {""",
        "connectSelected",
    )

    src = must_replace(
        src,
        """  function releaseRpcSession(session) {
    if (isDrive(session)) session.nasWatching = false;
    const sid = session && session.rpcSessionId ? session.rpcSessionId : 0;
    if (session) session.rpcSessionId = 0;
    if (!sid) return Promise.resolve();
    return tauriInvoke("webrpc_close_session", { sessionId: sid }).catch(function () {});
  }""",
        """  function releaseRpcSession(session) {
    if (isDrive(session)) session.nasWatching = false;
    if (isP2pSsh(session)) {
      disposeP2psshTermsForEntry(session);
      session.connected = false;
      session.localPort = 0;
      session.sshCommand = "";
      const id = session.id;
      session.rpcSessionId = 0;
      return tauriInvoke("p2pssh_disconnect", { id: id }).catch(function () {});
    }
    const sid = session && session.rpcSessionId ? session.rpcSessionId : 0;
    if (session) session.rpcSessionId = 0;
    if (!sid) return Promise.resolve();
    return tauriInvoke("webrpc_close_session", { sessionId: sid }).catch(function () {});
  }""",
        "releaseRpcSession",
    )

    src = must_replace(
        src,
        """    if (modalNewDrive) modalNewDrive.hidden = name !== "new-drive";
    modalRemark.hidden = name !== "remark";""",
        """    if (modalNewDrive) modalNewDrive.hidden = name !== "new-drive";
    if (modalNewP2pssh) modalNewP2pssh.hidden = name !== "new-p2pssh";
    modalRemark.hidden = name !== "remark";""",
        "openModal hide",
    )

    src = must_replace(
        src,
        """    if (newDriveError) newDriveError.hidden = true;
    if (name === "new") {""",
        """    if (newDriveError) newDriveError.hidden = true;
    if (newP2psshError) newP2psshError.hidden = true;
    if (name === "new") {""",
        "openModal err",
    )

    src = must_replace(
        src,
        """    if (name === "new-drive" && newDriveToken) {
      newDriveToken.value = "";
      if (newDrivePass) newDrivePass.value = "";
      window.setTimeout(function () {
        newDriveToken.focus();
      }, 0);
    }
    if (name === "remark") {""",
        """    if (name === "new-drive" && newDriveToken) {
      newDriveToken.value = "";
      if (newDrivePass) newDrivePass.value = "";
      window.setTimeout(function () {
        newDriveToken.focus();
      }, 0);
    }
    if (name === "new-p2pssh" && newP2psshToken) {
      newP2psshToken.value = "";
      if (newP2psshPass) newP2psshPass.value = "";
      if (newP2psshRemark) newP2psshRemark.value = "";
      window.setTimeout(function () {
        newP2psshToken.focus();
      }, 0);
    }
    if (name === "remark") {""",
        "openModal new-p2pssh",
    )

    src = must_replace(
        src,
        """    } else if (kind === "delete-drive") {
      state.deleteSessionId = sessionId;
      confirmTitle.textContent = t("删除连接");
      confirmDesc.textContent = t("删除后将从「网盘连接」列表和本地缓存中移除。不会影响聊天会话。");
      confirmOkBtn.textContent = t("删除");
    } else {""",
        """    } else if (kind === "delete-drive") {
      state.deleteSessionId = sessionId;
      confirmTitle.textContent = t("删除连接");
      confirmDesc.textContent = t("删除后将从「网盘连接」列表和本地缓存中移除。不会影响聊天会话。");
      confirmOkBtn.textContent = t("删除");
    } else if (kind === "delete-p2pssh") {
      state.deleteSessionId = sessionId;
      confirmTitle.textContent = t("删除隧道");
      confirmDesc.textContent = t("删除后将从「P2P SSH」列表和本地缓存中移除。");
      confirmOkBtn.textContent = t("删除");
    } else {""",
        "openConfirm",
    )

    src = must_replace(
        src,
        """    if (modalNewDrive) modalNewDrive.hidden = true;
    modalRemark.hidden = true;""",
        """    if (modalNewDrive) modalNewDrive.hidden = true;
    if (modalNewP2pssh) modalNewP2pssh.hidden = true;
    modalRemark.hidden = true;""",
        "closeModal",
    )

    src = must_replace(
        src,
        """      (!modalNewDrive || modalNewDrive.hidden) &&
      modalRemark.hidden &&""",
        """      (!modalNewDrive || modalNewDrive.hidden) &&
      (!modalNewP2pssh || modalNewP2pssh.hidden) &&
      modalRemark.hidden &&""",
        "register modal check",
    )

    src = must_replace(
        src,
        """  function createDrive() {
    if (!newDriveToken || !newDriveError) return;""",
        """  function createP2pSsh() {
    if (!newP2psshToken || !newP2psshError) return;
    const token = newP2psshToken.value.trim();
    const pass = newP2psshPass ? newP2psshPass.value.trim() : "";
    const remark = newP2psshRemark ? newP2psshRemark.value.trim() : "";
    if (!token) {
      newP2psshError.textContent = t("请输入服务端 Token");
      newP2psshError.hidden = false;
      return;
    }
    if (
      state.p2pssh.some(function (item) {
        return item.peerToken === token;
      })
    ) {
      newP2psshError.textContent = t("该 P2P SSH 已存在");
      newP2psshError.hidden = false;
      return;
    }
    const draft = hydrateP2pSsh({
      peerToken: token,
      peerPass: pass,
      remark: remark,
    });
    persistP2pSshCreate(draft)
      .then(function (items) {
        const list = (Array.isArray(items) ? items : []).map(hydrateP2pSsh);
        state.p2pssh = list;
        const created =
          list.find(function (item) {
            return item.peerToken === token;
          }) || list[0];
        if (created) selectSession(created.id);
        state.connectFillId = null;
        closeModal();
        renderWorkspace();
      })
      .catch(function (err) {
        const text = String((err && err.message) || err || "");
        newP2psshError.textContent =
          text.indexOf("p2pssh-exists") >= 0 ? t("该 P2P SSH 已存在") : t("保存 P2P SSH 失败");
        newP2psshError.hidden = false;
      });
  }

  function createDrive() {
    if (!newDriveToken || !newDriveError) return;""",
        "createP2pSsh",
    )

    src = must_replace(
        src,
        """    const list = isDrive(item) ? state.drives : state.sessions;
    releaseRpcSession(item).then(function () {
      const still = list.findIndex(function (s) {
        return s.id === id;
      });
      if (still >= 0) {
        list.splice(still, 1);
      }
      if (isDrive(item)) {
        persistDriveDelete(item.peerToken);
      } else {
        persistSessionDelete(item.peerToken);
        persistChatDelete(item.peerToken);
        revokePreviews([item]);
        (item.messages || []).forEach(function (msg) {
          stopTick(msg.id);
        });
      }
      if (state.selectedId === id) {
        state.connectFillId = null;
        const next = state.sessions[0] || state.drives[0];
        selectSession(next ? next.id : null);
      }""",
        """    const list = isDrive(item) ? state.drives : isP2pSsh(item) ? state.p2pssh : state.sessions;
    releaseRpcSession(item).then(function () {
      const still = list.findIndex(function (s) {
        return s.id === id;
      });
      if (still >= 0) {
        list.splice(still, 1);
      }
      if (isDrive(item)) {
        persistDriveDelete(item.peerToken);
      } else if (isP2pSsh(item)) {
        persistP2pSshDelete(item.id);
      } else {
        persistSessionDelete(item.peerToken);
        persistChatDelete(item.peerToken);
        revokePreviews([item]);
        (item.messages || []).forEach(function (msg) {
          stopTick(msg.id);
        });
      }
      if (state.selectedId === id) {
        state.connectFillId = null;
        const next = state.sessions[0] || state.drives[0] || state.p2pssh[0];
        selectSession(next ? next.id : null);
      }""",
        "confirmDelete",
    )

    src = must_replace(
        src,
        """  function findItem(id) {
    return findSession(id) || findDrive(id) || null;
  }

  function findByRpcSessionId(sessionId) {
    const sid = Number(sessionId) || 0;
    if (!sid) return null;
    return (
      state.sessions.find(function (item) {
        return Number(item.rpcSessionId) === sid;
      }) ||
      state.drives.find(function (item) {
        return Number(item.rpcSessionId) === sid;
      }) ||
      null
    );
  }""",
        """  function findP2pSsh(id) {
    return state.p2pssh.find(function (s) {
      return s.id === id;
    });
  }

  function findItem(id) {
    return findSession(id) || findDrive(id) || findP2pSsh(id) || null;
  }

  function findByRpcSessionId(sessionId) {
    const sid = Number(sessionId) || 0;
    if (!sid) return null;
    return (
      state.sessions.find(function (item) {
        return Number(item.rpcSessionId) === sid;
      }) ||
      state.drives.find(function (item) {
        return Number(item.rpcSessionId) === sid;
      }) ||
      state.p2pssh.find(function (item) {
        return Number(item.rpcSessionId) === sid;
      }) ||
      null
    );
  }""",
        "findItem",
    )

    marker = "  function findSession(id) {"
    if marker not in src:
        raise SystemExit("findSession marker missing")

    term_block = r'''
  function b64EncodeUtf8Bytes(bytes) {
    let binary = "";
    const chunk = 0x8000;
    for (let i = 0; i < bytes.length; i += chunk) {
      binary += String.fromCharCode.apply(null, bytes.subarray(i, i + chunk));
    }
    return btoa(binary);
  }

  function b64DecodeToUint8(b64) {
    const binary = atob(b64 || "");
    const out = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i++) out[i] = binary.charCodeAt(i);
    return out;
  }

  function disposeP2psshTerm(termId) {
    const slot = p2psshTermMap[termId];
    if (!slot) return;
    try {
      if (slot.term) slot.term.dispose();
    } catch (_) {}
    delete p2psshTermMap[termId];
    delete p2psshTermPending[termId];
  }

  function disposeP2psshTermsForEntry(entry) {
    if (!entry) return;
    (entry.terms || []).forEach(function (term) {
      disposeP2psshTerm(term.termId);
      tauriInvoke("p2pssh_term_close", { termId: term.termId }).catch(function () {});
    });
    entry.terms = [];
    entry.activeTermId = null;
  }

  function disposeAllP2psshTerms() {
    Object.keys(p2psshTermMap).forEach(disposeP2psshTerm);
    (state.p2pssh || []).forEach(function (entry) {
      entry.terms = [];
      entry.activeTermId = null;
    });
  }

  function onP2psshDisconnected(payload) {
    const id = payload && payload.id;
    const entry = findP2pSsh(id);
    if (!entry) return;
    disposeP2psshTermsForEntry(entry);
    entry.connected = false;
    entry.connecting = false;
    entry.rpcSessionId = 0;
    entry.localPort = 0;
    entry.sshCommand = "";
    renderWorkspace();
  }

  function writeP2psshTermBytes(termId, bytes) {
    const slot = p2psshTermMap[termId];
    if (slot && slot.term) {
      try {
        slot.term.write(bytes);
      } catch (_) {}
      return;
    }
    if (!p2psshTermPending[termId]) p2psshTermPending[termId] = [];
    p2psshTermPending[termId].push(bytes);
  }

  function flushP2psshTermPending(termId) {
    const pending = p2psshTermPending[termId];
    if (!pending || !pending.length) return;
    delete p2psshTermPending[termId];
    pending.forEach(function (bytes) {
      writeP2psshTermBytes(termId, bytes);
    });
  }

  function onP2psshTermData(payload) {
    if (!payload || !payload.termId || !payload.data) return;
    writeP2psshTermBytes(payload.termId, b64DecodeToUint8(payload.data));
  }

  function onP2psshTermExit(payload) {
    const termId = payload && payload.termId;
    if (!termId) return;
    let touched = null;
    state.p2pssh.forEach(function (entry) {
      const term = (entry.terms || []).find(function (item) {
        return item.termId === termId;
      });
      if (!term) return;
      term.exited = true;
      touched = entry;
    });
    writeP2psshTermBytes(
      termId,
      new TextEncoder().encode("\r\n\u001b[90m[" + t("会话已结束") + "]\u001b[0m\r\n")
    );
    if (touched && state.selectedId === touched.id) {
      renderP2pSshPanel(touched);
    }
  }

  function renderP2pSshPanel(entry) {
    if (!entry || !panelP2pssh) return;
    if (p2psshTitle) p2psshTitle.textContent = entry.remark || t("P2P SSH");
    if (p2psshSubtitle) p2psshSubtitle.textContent = entry.peerToken || "—";
    if (p2psshCmd) {
      p2psshCmd.textContent =
        entry.sshCommand ||
        (entry.localPort ? "ssh -p " + entry.localPort + " user@127.0.0.1" : "ssh -p PORT user@127.0.0.1");
    }
    if (!p2psshTabs || !p2psshTermHost) return;
    const terms = entry.terms || [];
    if (!terms.length) {
      p2psshTabs.innerHTML = "";
      p2psshTermHost.innerHTML =
        '<div class="p2pssh-term-empty">' + escapeHtml(t("点击「新建终端」打开系统 ssh")) + "</div>";
      return;
    }
    p2psshTabs.innerHTML = terms
      .map(function (term) {
        const active = term.termId === entry.activeTermId ? " is-active" : "";
        const exited = term.exited ? " is-exited" : "";
        return (
          '<div class="p2pssh-tab' +
          active +
          exited +
          '" data-term-id="' +
          escapeHtml(term.termId) +
          '" role="tab">' +
          '<button type="button" class="p2pssh-tab-close" data-close-term="' +
          escapeHtml(term.termId) +
          '" aria-label="' +
          escapeHtml(t("关闭")) +
          '">\u00d7</button>' +
          '<button type="button" class="p2pssh-tab-label" data-term-id="' +
          escapeHtml(term.termId) +
          '">' +
          escapeHtml(term.title || t("终端")) +
          (term.exited ? " \u00b7 " + t("已结束") : "") +
          "</button>" +
          "</div>"
        );
      })
      .join("");

    const existing = {};
    Array.prototype.forEach.call(p2psshTermHost.querySelectorAll(".p2pssh-term-pane"), function (el) {
      existing[el.getAttribute("data-term-id")] = el;
    });
    const keep = {};
    terms.forEach(function (term) {
      keep[term.termId] = true;
      let pane = existing[term.termId];
      if (!pane) {
        pane = document.createElement("div");
        pane.className = "p2pssh-term-pane";
        pane.setAttribute("data-term-id", term.termId);
        p2psshTermHost.appendChild(pane);
        ensureP2psshXterm(entry, term.termId, pane);
      }
      pane.classList.toggle("is-active", term.termId === entry.activeTermId);
    });
    Object.keys(existing).forEach(function (tid) {
      if (!keep[tid]) {
        existing[tid].remove();
        disposeP2psshTerm(tid);
      }
    });
    const empty = p2psshTermHost.querySelector(".p2pssh-term-empty");
    if (empty) empty.remove();
    const active = p2psshTermMap[entry.activeTermId];
    if (active && active.fit) {
      try {
        active.fit.fit();
      } catch (_) {}
    }
    if (active && active.term && active.term.focus) {
      try {
        active.term.focus();
      } catch (_) {}
    }
  }

  function ensureP2psshXterm(entry, termId, pane) {
    if (p2psshTermMap[termId]) return;
    const TerminalCtor = window.Terminal;
    const FitAddonNs = window.FitAddon;
    if (!TerminalCtor) {
      pane.innerHTML =
        '<div class="p2pssh-term-empty">' + escapeHtml(t("终端组件未加载，请重新编译应用")) + "</div>";
      return;
    }
    const term = new TerminalCtor({
      convertEol: true,
      cursorBlink: true,
      disableStdin: false,
      fontSize: 13,
      fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", monospace',
      theme: {
        background: "#1e1e1e",
        foreground: "#d4d4d4",
        cursor: "#d4d4d4",
        selectionBackground: "rgba(255,255,255,0.18)",
      },
    });
    let fit = null;
    if (FitAddonNs && FitAddonNs.FitAddon) {
      fit = new FitAddonNs.FitAddon();
      term.loadAddon(fit);
    }
    term.open(pane);
    p2psshTermMap[termId] = { term: term, fit: fit, entryId: entry.id };
    flushP2psshTermPending(termId);
    term.onData(function (data) {
      const meta = (entry.terms || []).find(function (item) {
        return item.termId === termId;
      });
      if (meta && meta.exited) return;
      tauriInvoke("p2pssh_term_write", {
        termId: termId,
        data: b64EncodeUtf8Bytes(new TextEncoder().encode(data)),
      }).catch(function () {});
    });
    pane.addEventListener("mousedown", function () {
      try {
        term.focus();
      } catch (_) {}
    });
    window.requestAnimationFrame(function () {
      if (fit) {
        try {
          fit.fit();
        } catch (_) {}
      }
      try {
        term.focus();
      } catch (_) {}
      tauriInvoke("p2pssh_term_resize", {
        termId: termId,
        cols: term.cols || 80,
        rows: term.rows || 24,
      }).catch(function () {});
    });
  }

  function openP2psshTerminal(entry) {
    if (!entry || !entry.connected) return;
    let cols = 100;
    let rows = 28;
    if (p2psshTermHost && p2psshTermHost.clientWidth > 40 && p2psshTermHost.clientHeight > 40) {
      cols = Math.max(40, Math.floor((p2psshTermHost.clientWidth - 16) / 8));
      rows = Math.max(12, Math.floor((p2psshTermHost.clientHeight - 16) / 17));
    }
    const newTermBtn = document.getElementById("p2pssh-new-term");
    if (newTermBtn) newTermBtn.disabled = true;
    tauriInvoke("p2pssh_term_open", { id: entry.id, cols: cols, rows: rows })
      .then(function (termId) {
        if (!termId) return;
        entry.termSeq = (entry.termSeq || 0) + 1;
        entry.terms = entry.terms || [];
        entry.terms.push({
          termId: termId,
          title: t("终端") + " " + entry.termSeq,
          exited: false,
        });
        entry.activeTermId = termId;
        renderP2pSshPanel(entry);
      })
      .catch(function (err) {
        openInfoPrompt(t("新建终端失败"), invokeErrorText(err) || t("无法打开终端"));
      })
      .finally(function () {
        if (newTermBtn) newTermBtn.disabled = false;
      });
  }

  function closeP2psshTerminal(entry, termId) {
    if (!entry || !termId) return;
    tauriInvoke("p2pssh_term_close", { termId: termId }).catch(function () {});
    disposeP2psshTerm(termId);
    entry.terms = (entry.terms || []).filter(function (item) {
      return item.termId !== termId;
    });
    if (entry.activeTermId === termId) {
      entry.activeTermId = entry.terms.length ? entry.terms[entry.terms.length - 1].termId : null;
    }
    renderP2pSshPanel(entry);
  }

  function copyP2psshCommand() {
    const entry = findP2pSsh(state.selectedId);
    if (!entry || !p2psshCmd) return;
    const text =
      entry.sshCommand ||
      (entry.localPort ? "ssh -p " + entry.localPort + " user@127.0.0.1" : "");
    if (!text || !navigator.clipboard || !navigator.clipboard.writeText) return;
    navigator.clipboard.writeText(text).then(function () {
      p2psshCmd.classList.add("is-copied");
      const prev = p2psshCmd.getAttribute("title") || "";
      p2psshCmd.setAttribute("title", t("已复制"));
      window.setTimeout(function () {
        p2psshCmd.classList.remove("is-copied");
        p2psshCmd.setAttribute("title", prev || t("点击复制"));
      }, 1200);
    });
  }

  function bindP2psshPanel() {
    const newTermBtn = document.getElementById("p2pssh-new-term");
    const disconnectBtn = document.getElementById("p2pssh-disconnect");
    if (p2psshCmd) {
      p2psshCmd.addEventListener("click", copyP2psshCommand);
      p2psshCmd.addEventListener("keydown", function (event) {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          copyP2psshCommand();
        }
      });
    }
    if (newTermBtn) {
      newTermBtn.addEventListener("click", function () {
        openP2psshTerminal(findP2pSsh(state.selectedId));
      });
    }
    if (disconnectBtn) {
      disconnectBtn.addEventListener("click", function () {
        const entry = findP2pSsh(state.selectedId);
        if (!entry) return;
        releaseRpcSession(entry).then(function () {
          renderWorkspace();
        });
      });
    }
    if (p2psshTabs) {
      p2psshTabs.addEventListener("click", function (event) {
        const entry = findP2pSsh(state.selectedId);
        if (!entry) return;
        const closeEl = event.target.closest("[data-close-term]");
        if (closeEl) {
          event.stopPropagation();
          closeP2psshTerminal(entry, closeEl.getAttribute("data-close-term"));
          return;
        }
        const tab = event.target.closest(".p2pssh-tab");
        if (!tab) return;
        entry.activeTermId = tab.getAttribute("data-term-id");
        renderP2pSshPanel(entry);
      });
    }
    window.addEventListener("resize", function () {
      const entry = findP2pSsh(state.selectedId);
      if (!entry || !entry.activeTermId) return;
      const slot = p2psshTermMap[entry.activeTermId];
      if (!slot || !slot.fit || !slot.term) return;
      try {
        slot.fit.fit();
        tauriInvoke("p2pssh_term_resize", {
          termId: entry.activeTermId,
          cols: slot.term.cols || 80,
          rows: slot.term.rows || 24,
        }).catch(function () {});
      } catch (_) {}
    });
  }

'''
    src = src.replace(marker, term_block + marker, 1)

    # Post-fixes: never let p2pssh break chat/drive selection.
    src = must_replace(
        src,
        """  function selectSession(id) {
    if (state.selectedId && state.selectedId !== id) {
      const prev = findItem(state.selectedId);
      if (prev && !isDrive(prev)) unloadSessionChats(prev);
      clearNasSelection();
    }
    state.selectedId = id;
  }""",
        """  function selectSession(id) {
    if (state.selectedId && state.selectedId !== id) {
      const prev = findItem(state.selectedId);
      if (prev && prev.kind === "chat") unloadSessionChats(prev);
      clearNasSelection();
    }
    state.selectedId = id;
  }""",
        "selectSession-chat-only",
    )
    src = must_replace(
        src,
        """  function revokePreviews(sessions) {
    sessions.forEach(function (session) {
      session.messages.forEach(function (msg) {
        if (msg.previewUrl && msg.previewUrl.indexOf("blob:") === 0) {
          URL.revokeObjectURL(msg.previewUrl);
        }
      });
    });
  }""",
        """  function revokePreviews(sessions) {
    (sessions || []).forEach(function (session) {
      if (!session) return;
      (session.messages || []).forEach(function (msg) {
        if (msg && msg.previewUrl && msg.previewUrl.indexOf("blob:") === 0) {
          URL.revokeObjectURL(msg.previewUrl);
        }
      });
    });
  }""",
        "revokePreviews-safe",
    )

    if "登录" not in src:
        raise SystemExit("Chinese lost after patch; abort write")
    if src.count("???") > 20:
        raise SystemExit("too many ??? after patch; abort write")

    APP_JS.write_text(src, encoding="utf-8", newline="\n")
    print("patched", APP_JS, "login_ok", "登录" in src, "p2pssh", "isP2pSsh" in src)


if __name__ == "__main__":
    main()
