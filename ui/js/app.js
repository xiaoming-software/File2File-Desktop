(function () {
  const TITLE_MAX_BYTES = 30;
  const COPY_MIN_LINES = 5;

  const IMAGE_EXT = ["png", "jpg", "jpeg", "gif", "webp", "bmp", "svg"];
  const VIDEO_EXT = ["mp4", "mov", "avi", "mkv", "webm", "m4v"];
  const OFFICE_EXT = ["doc", "docx", "xls", "xlsx", "ppt", "pptx", "pdf", "csv", "odt", "ods", "odp"];
  const AUDIO_EXT = ["mp3", "wav", "flac", "aac", "m4a", "ogg", "wma"];
  const KIND_BADGE = {
    image: "IMG",
    video: "VID",
    office: "DOC",
    audio: "AUD",
    other: "FILE",
  };

  const appEl = document.getElementById("app");
  const form = document.getElementById("login-form");
  const tokenInput = document.getElementById("token");
  const tokenCacheBtn = document.getElementById("token-cache-btn");
  const tokenDropdown = document.getElementById("token-dropdown");
  const tokenDropdownList = document.getElementById("token-dropdown-list");
  const passwordInput = document.getElementById("password");
  const passphraseInput = document.getElementById("passphrase");
  const saveTokenInput = document.getElementById("save-token");
  const loginBtn = document.getElementById("btn-login");
  const errorEl = document.getElementById("login-error");
  const pageLogin = document.getElementById("page-login");
  const pageSessions = document.getElementById("page-sessions");

  const infoToken = document.getElementById("info-token");
  const infoPassphrase = document.getElementById("info-passphrase");
  const infoLoginTime = document.getElementById("info-login-time");
  const infoSessionCount = document.getElementById("info-session-count");
  const sessionListEl = document.getElementById("session-list");
  const panelUnselected = document.getElementById("panel-unselected");
  const panelConnect = document.getElementById("panel-connect");
  const chatMain = document.getElementById("chat-main");
  const chatPane = document.getElementById("chat-pane");
  const chatDropMask = document.getElementById("chat-drop-mask");
  const connectPeer = document.getElementById("connect-peer");
  const connectPeerPass = document.getElementById("connect-peer-pass");
  const connectBtn = document.getElementById("btn-connect");
  const connectError = document.getElementById("connect-error");
  const chatTitle = document.getElementById("chat-title");
  const chatSubtitle = document.getElementById("chat-subtitle");
  const chatStatusDot = document.getElementById("chat-status-dot");
  const chatConnLabel = document.getElementById("chat-conn-label");
  const chatLog = document.getElementById("chat-log");
  const composer = document.getElementById("composer");
  const composerInput = document.getElementById("composer-input");
  const fileInput = document.getElementById("file-input");
  const pendingFileEl = document.getElementById("pending-file");
  const pendingFileName = document.getElementById("pending-file-name");
  const modalRoot = document.getElementById("modal-root");
  const modalNew = document.getElementById("modal-new-session");
  const modalRemark = document.getElementById("modal-remark");
  const modalConfirm = document.getElementById("modal-confirm");
  const confirmTitle = document.getElementById("confirm-title");
  const confirmDesc = document.getElementById("confirm-desc");
  const confirmOkBtn = document.getElementById("btn-confirm-ok");
  const confirmCancelBtn = document.getElementById("btn-confirm-cancel");
  const bubbleMenu = document.getElementById("bubble-menu");
  const newPeerToken = document.getElementById("new-peer-token");
  const newPeerPass = document.getElementById("new-peer-pass");
  const newSessionError = document.getElementById("new-session-error");
  const remarkInput = document.getElementById("remark-input");
  const mediaOverlay = document.getElementById("media-overlay");
  const mediaOverlayImage = document.getElementById("media-overlay-image");
  const mediaOverlayVideo = document.getElementById("media-overlay-video");

  const state = {
    user: null,
    sessions: [],
    selectedId: null,
    menuSessionId: null,
    remarkSessionId: null,
    deleteSessionId: null,
    clearSessionId: null,
    deleteMessageId: null,
    confirmKind: "",
    pendingFile: null,
    nextId: 1,
    ticks: {},
    connectTimer: null,
    savedAccounts: [],
    connectFillId: null,
    sdkSessionCount: 0,
    sessionsReady: false,
    pendingHellos: [],
    pendingTexts: [],
    inflightFiles: {},
    sendQueue: [],
    sendActiveId: null,
    screenshotBusy: false,
    voiceMuted: false,
    voiceTimer: null,
    voiceStartedAt: 0,
  };

  function isTauri() {
    return !!(window.__TAURI__ || window.__TAURI_INTERNALS__);
  }

  function tauriInvoke(cmd, args) {
    try {
      if (
        window.__TAURI_INTERNALS__ &&
        typeof window.__TAURI_INTERNALS__.invoke === "function"
      ) {
        return window.__TAURI_INTERNALS__.invoke(cmd, args || {});
      }
      if (
        window.__TAURI__ &&
        window.__TAURI__.core &&
        typeof window.__TAURI__.core.invoke === "function"
      ) {
        return window.__TAURI__.core.invoke(cmd, args || {});
      }
    } catch (_) {
      /* fall through */
    }
    return Promise.reject(new Error("webrpc-unavailable"));
  }

  function bindSessionSizeListener() {
    try {
      if (window.__TAURI__ && window.__TAURI__.event && window.__TAURI__.event.listen) {
        window.__TAURI__.event.listen("webrpc-session-size", function (event) {
          applySdkSessionCount(Number(event.payload) || 0);
        });
        window.__TAURI__.event.listen("webrpc-peer-hello", function (event) {
          onPeerHello(event.payload || {});
        });
        window.__TAURI__.event.listen("webrpc-peer-text", function (event) {
          onPeerText(event.payload || {});
        });
        window.__TAURI__.event.listen("webrpc-file-event", function (event) {
          onFileEvent(event.payload || {});
        });
        window.__TAURI__.event.listen("webrpc-session-dead", function (event) {
          onSessionDead(event.payload);
        });
        window.__TAURI__.event.listen("screenshot-done", function (event) {
          onScreenshotDone(event.payload);
        });
        window.__TAURI__.event.listen("webrpc-voice-state", function (event) {
          onVoiceState(event.payload || {});
        });
      }
    } catch (_) {
      /* ignore */
    }
  }

  function applySdkSessionCount(count) {
    if (!state.user) {
      state.sdkSessionCount = 0;
      infoSessionCount.textContent = "0";
      return;
    }
    const n = Number(count);
    state.sdkSessionCount = Number.isFinite(n) ? n : 0;
    infoSessionCount.textContent = String(state.sdkSessionCount);
  }

  function tauriWindow() {
    try {
      return window.__TAURI__.window.getCurrentWindow();
    } catch (_) {
      return null;
    }
  }

  function initTauriShell() {
    if (!isTauri()) return;
    document.documentElement.classList.add("is-tauri");

    function bindWin(selector, action) {
      document.querySelectorAll(selector).forEach(function (el) {
        el.addEventListener("click", function (event) {
          event.preventDefault();
          event.stopPropagation();
          const win = tauriWindow();
          if (!win) return;
          action(win);
        });
      });
    }

    bindWin(".tl-close, .win-btn-close", function (win) {
      win.close();
    });
    bindWin(".tl-min, .titlebar-win-controls .win-btn:nth-child(1)", function (win) {
      win.minimize();
    });
    bindWin(".tl-max, .titlebar-win-controls .win-btn:nth-child(2)", function (win) {
      win.isMaximized().then(function (max) {
        if (max) win.unmaximize();
        else win.maximize();
      });
    });

    document.addEventListener("click", function (event) {
      const link = event.target.closest('a[href^="http"]');
      if (!link || !window.__TAURI__ || !window.__TAURI__.opener) return;
      event.preventDefault();
      window.__TAURI__.opener.openUrl(link.href);
    });
    bindFileDrop();
    document.addEventListener("dragover", function (event) {
      event.preventDefault();
      if (event.dataTransfer) event.dataTransfer.dropEffect = "copy";
    });
    document.addEventListener("drop", function (event) {
      event.preventDefault();
    });
  }

  function bindFileDrop() {
    var lastDropAt = 0;

    function takeDrop(paths, position) {
      var list = normalizeDropPaths(paths);
      if (!list.length) return;
      var now = Date.now();
      if (now - lastDropAt < 400) return;
      lastDropAt = now;
      hideDropMask();
      handleDroppedPaths(list, position);
    }

    function fromPayload(payload, fallbackType) {
      payload = payload || {};
      if (payload.payload && (payload.payload.paths || payload.payload.type)) {
        payload = payload.payload;
      }
      var type = String(payload.type || fallbackType || "").toLowerCase();
      if (type === "enter" || type === "over") {
        if (canDropSendNow()) showDropMask();
        else hideDropMask();
        return;
      }
      if (type === "leave" || type === "cancel") {
        hideDropMask();
        return;
      }
      if (type === "drop" || payload.paths) {
        takeDrop(payload.paths, payload.position);
      }
    }

    function listenTauri(name, kind) {
      window.__TAURI__.event.listen(name, function (event) {
        fromPayload(event.payload || {}, kind);
      });
    }

    try {
      if (window.__TAURI__ && window.__TAURI__.window && window.__TAURI__.window.getCurrentWindow) {
        var win = window.__TAURI__.window.getCurrentWindow();
        if (win && typeof win.onDragDropEvent === "function") {
          win.onDragDropEvent(function (event) {
            fromPayload(event && event.payload, "");
          });
        }
      }
    } catch (_) {}
    try {
      if (window.__TAURI__ && window.__TAURI__.webview && window.__TAURI__.webview.getCurrentWebview) {
        var webview = window.__TAURI__.webview.getCurrentWebview();
        if (webview && typeof webview.onDragDropEvent === "function") {
          webview.onDragDropEvent(function (event) {
            fromPayload(event && event.payload, "");
          });
        }
      }
    } catch (_) {}
    try {
      if (window.__TAURI__ && window.__TAURI__.event && window.__TAURI__.event.listen) {
        listenTauri("tauri://drag-enter", "enter");
        listenTauri("tauri://drag-over", "over");
        listenTauri("tauri://drag-leave", "leave");
        listenTauri("tauri://drag-drop", "drop");
      }
    } catch (_) {}

    document.addEventListener("dragenter", function (event) {
      event.preventDefault();
      if (canDropSendNow()) showDropMask();
    });
    document.addEventListener("dragleave", function (event) {
      if (event.relatedTarget && document.documentElement.contains(event.relatedTarget)) return;
      hideDropMask();
    });
    document.addEventListener("drop", function (event) {
      event.preventDefault();
      hideDropMask();
      var files = event.dataTransfer && event.dataTransfer.files;
      if (!files || !files.length) return;
      var paths = [];
      for (var i = 0; i < files.length; i++) {
        var path = files[i].path || "";
        if (path) paths.push(path);
      }
      if (paths.length) takeDrop(paths, { x: event.clientX, y: event.clientY });
    });
  }

  function normalizeDropPaths(raw) {
    if (!raw) return [];
    if (typeof raw === "string") return [raw];
    if (!Array.isArray(raw)) return [];
    return raw
      .map(function (item) {
        if (!item) return "";
        if (typeof item === "string") return item.trim();
        if (typeof item.path === "string") return item.path.trim();
        return String(item).trim();
      })
      .filter(Boolean);
  }

  function canDropSendNow() {
    const session = findSession(state.selectedId);
    return !!(session && session.connected && chatMain && chatMain.classList.contains("is-visible"));
  }

  function showDropMask() {
    if (chatDropMask) chatDropMask.hidden = false;
  }

  function hideDropMask() {
    if (chatDropMask) chatDropMask.hidden = true;
  }

  function handleDroppedPaths(paths, _position) {
    const list = normalizeDropPaths(paths);
    if (!list.length) return;

    const session = findSession(state.selectedId);
    if (!session) {
      openInfoPrompt("无法发送文件", "请先选择并连接一个会话，再拖入文件发送。");
      return;
    }
    if (!session.connected) {
      openInfoPrompt("无法发送文件", "请先连接当前会话，再拖入文件发送。");
      return;
    }

    tauriInvoke("inspect_paths", { paths: list })
      .then(function (infos) {
        const items = Array.isArray(infos) ? infos : [];
        if (!items.length) return;
        const hasFolder = items.some(function (item) {
          return item && item.kind === "directory";
        });
        if (hasFolder) {
          openInfoPrompt(
            "无法发送文件夹",
            "不支持直接发送文件夹。请打开文件夹，选中其中的文件后再拖入会话。"
          );
          return;
        }
        const files = items.filter(function (item) {
          return item && item.kind === "file";
        });
        if (!files.length) {
          openInfoPrompt("无法发送文件", "拖入的内容不是可发送的文件，请重新选择后再试。");
          return;
        }
        sendDroppedFiles(session, files);
      })
      .catch(function () {
        openInfoPrompt("无法发送文件", "读取拖入的文件失败，请重新选择后再试。");
      });
  }

  function sendDroppedFiles(session, files) {
    if (!session || !session.connected || !files || !files.length) return;
    const base = Date.now();
    files.forEach(function (info, index) {
      const msg = fileMsg("me", classifyFile(info.name), info.name, Number(info.size) || 0, base + index, {
        status: "sending",
        transferred: 0,
        elapsedMs: 0,
        speedBps: 0,
        filePath: info.path || "",
      });
      session.messages.push(msg);
      persistChatAppend(session, msg);
      sendFileMessage(session, msg);
    });
    if (state.selectedId === session.id && session.chatsLoaded) renderChat();
  }

  function isScreenshotHotkey(event) {
    if (!event || event.repeat) return false;
    const isA = event.code === "KeyA" || String(event.key || "").toLowerCase() === "a";
    if (!isA) return false;
    const macWechat = event.ctrlKey && event.metaKey && !event.shiftKey && !event.altKey;
    const winWechat = event.altKey && !event.metaKey && !event.ctrlKey && !event.shiftKey;
    return macWechat || winWechat;
  }

  function startScreenshot(hideApp) {
    hideShotMenu();
    if (!isTauri() || state.screenshotBusy) return;
    const session = findSession(state.selectedId);
    if (!session) {
      openInfoPrompt("无法截图", "请先选择并连接一个会话，再使用截图。");
      return;
    }
    if (!session.connected) {
      openInfoPrompt("无法截图", "请先连接当前会话，再使用截图。");
      return;
    }
    state.screenshotBusy = true;
    tauriInvoke("screenshot_start", { hideApp: !!hideApp, hide_app: !!hideApp })
      .catch(function (err) {
        var msg = "";
        if (typeof err === "string") msg = err;
        else if (err && err.message) msg = String(err.message);
        else msg = String(err || "");
        if (!msg || msg === "[object Object]" || msg === "undefined") {
          msg = "截图失败，请稍后重试。";
        }
        openInfoPrompt("无法截图", msg);
        tauriInvoke("screenshot_cancel").catch(function () {});
      })
      .then(function () {
        state.screenshotBusy = false;
      });
  }

  function hideShotMenu() {
    const menu = document.getElementById("shot-menu");
    const caret = document.getElementById("btn-shot-caret");
    if (menu) menu.hidden = true;
    if (caret) caret.setAttribute("aria-expanded", "false");
  }

  function toggleShotMenu(event) {
    if (event) {
      event.preventDefault();
      event.stopPropagation();
    }
    const menu = document.getElementById("shot-menu");
    const caret = document.getElementById("btn-shot-caret");
    if (!menu || !caret || caret.disabled) return;
    const open = menu.hidden;
    menu.hidden = !open;
    caret.setAttribute("aria-expanded", open ? "true" : "false");
  }

  function onScreenshotDone(path) {
    const filePath = String(path || "").trim();
    if (!filePath) return;
    const session = findSession(state.selectedId);
    if (!session || !session.connected) {
      openInfoPrompt("无法发送截图", "当前会话未连接，截图未发送。");
      return;
    }
    tauriInvoke("file_stat", { path: filePath })
      .then(function (info) {
        if (!info || !info.path) return;
        sendDroppedFiles(session, [info]);
      })
      .catch(function () {
        openInfoPrompt("无法发送截图", "读取截图失败，请重新截取后再试。");
      });
  }

  function showVoiceError(err) {
    var msg = "";
    if (typeof err === "string") msg = err;
    else if (err && err.message) msg = String(err.message);
    else msg = String(err || "");
    if (!msg || msg === "[object Object]" || msg === "undefined") {
      msg = "语音通话失败，请稍后重试。";
    }
    openInfoPrompt("无法语音通话", msg);
  }

  function startVoiceCall() {
    const session = findSession(state.selectedId);
    if (!session) {
      openInfoPrompt("无法语音通话", "请先选择并连接一个会话。");
      return;
    }
    if (!session.connected || !session.rpcSessionId) {
      openInfoPrompt("无法语音通话", "请先连接当前会话，再发起语音通话。");
      return;
    }
    tauriInvoke("voice_invite", { sessionId: session.rpcSessionId }).catch(showVoiceError);
  }

  function voicePeerLabel(sessionId, fallback) {
    const session = state.sessions.find(function (item) {
      return item.rpcSessionId === sessionId;
    });
    if (session) return session.remark || session.peerToken || fallback || "对方";
    return fallback || "对方";
  }

  function formatVoiceTimer(startedAt) {
    const start = Number(startedAt) || 0;
    const sec = Math.max(0, Math.floor((Date.now() - start) / 1000));
    const mm = String(Math.floor(sec / 60)).padStart(2, "0");
    const ss = String(sec % 60).padStart(2, "0");
    return mm + ":" + ss;
  }

  function stopVoiceTimer() {
    if (state.voiceTimer) {
      clearInterval(state.voiceTimer);
      state.voiceTimer = null;
    }
  }

  function startVoiceTimer(startedAt) {
    stopVoiceTimer();
    state.voiceStartedAt = Number(startedAt) || Date.now();
    const el = document.getElementById("voice-timer");
    function tick() {
      if (el) el.textContent = formatVoiceTimer(state.voiceStartedAt);
    }
    tick();
    state.voiceTimer = setInterval(tick, 1000);
  }

  function hideVoiceUi() {
    stopVoiceTimer();
    const root = document.getElementById("voice-root");
    if (root) root.hidden = true;
  }

  function onVoiceState(payload) {
    const phase = String((payload && payload.phase) || "idle");
    const root = document.getElementById("voice-root");
    const title = document.getElementById("voice-title");
    const sub = document.getElementById("voice-sub");
    const timer = document.getElementById("voice-timer");
    const accept = document.getElementById("voice-accept");
    const reject = document.getElementById("voice-reject");
    const cancel = document.getElementById("voice-cancel");
    const hangup = document.getElementById("voice-hangup");
    const mute = document.getElementById("voice-mute");
    if (!root) return;
    if (phase === "idle") {
      hideVoiceUi();
      state.voiceMuted = false;
      return;
    }
    const sessionId = Number(payload.sessionId) || 0;
    const name = voicePeerLabel(sessionId, payload.peerToken);
    root.hidden = false;
    root.classList.toggle("is-active", phase === "active");
    state.voiceMuted = !!payload.muted;
    if (mute) {
      mute.classList.toggle("is-on", state.voiceMuted);
      mute.textContent = state.voiceMuted ? "取消静音" : "静音";
    }
    if (phase === "outgoing") {
      title.textContent = "正在呼叫";
      sub.textContent = name;
      timer.hidden = true;
      accept.hidden = true;
      reject.hidden = true;
      cancel.hidden = false;
      hangup.hidden = true;
      mute.hidden = true;
      stopVoiceTimer();
    } else if (phase === "incoming") {
      title.textContent = "邀请你语音通话";
      sub.textContent = name;
      timer.hidden = true;
      accept.hidden = false;
      reject.hidden = false;
      cancel.hidden = true;
      hangup.hidden = true;
      mute.hidden = true;
      stopVoiceTimer();
    } else {
      title.textContent = "语音通话中";
      sub.textContent = name;
      timer.hidden = false;
      accept.hidden = true;
      reject.hidden = true;
      cancel.hidden = true;
      hangup.hidden = false;
      mute.hidden = false;
      startVoiceTimer(payload.startedAt);
    }
  }

  function resizeTauriWindow(width, height) {
    if (!isTauri()) return;
    const win = tauriWindow();
    if (!win || !window.__TAURI__.dpi) return;
    const size = new window.__TAURI__.dpi.LogicalSize(width, height);
    win.setSize(size);
    win.center();
  }

  loadSavedAccounts();
  bindTokenDropdown();
  [tokenInput, passwordInput, passphraseInput].forEach(function (el) {
    el.addEventListener("input", hideLoginError);
  });
  bindPasswordToggle("toggle-password", passwordInput);
  bindPasswordToggle("toggle-passphrase", passphraseInput);
  bindPasswordToggle("toggle-connect-pass", connectPeerPass);
  bindWorkspaceEvents();
  initTauriShell();
  bindSessionSizeListener();

  form.addEventListener("submit", function (event) {
    event.preventDefault();
    submitLogin();
  });

  function persistAccount(credentials) {
    if (!saveTokenInput.checked) {
      return Promise.resolve();
    }
    return tauriInvoke("saved_accounts_upsert", {
      token: credentials.token,
      password: credentials.password,
      passphrase: credentials.passphrase || "",
    }).then(function (accounts) {
      applySavedAccounts(accounts);
    });
  }

  function loadSavedAccounts() {
    return tauriInvoke("saved_accounts_list")
      .then(applySavedAccounts)
      .catch(function () {
        applySavedAccounts([]);
      });
  }

  function applySavedAccounts(accounts) {
    state.savedAccounts = Array.isArray(accounts) ? accounts : [];
    renderTokenDropdown();
  }

  function bindTokenDropdown() {
    tokenCacheBtn.addEventListener("click", function (event) {
      event.preventDefault();
      event.stopPropagation();
      if (tokenDropdown.hidden) {
        openTokenDropdown();
      } else {
        closeTokenDropdown();
      }
    });

    tokenDropdownList.addEventListener("click", function (event) {
      const del = event.target.closest("[data-delete-index]");
      if (del) {
        event.preventDefault();
        event.stopPropagation();
        deleteSavedAccount(Number(del.getAttribute("data-delete-index")));
        return;
      }
      const pick = event.target.closest("[data-pick-index]");
      if (!pick) return;
      event.preventDefault();
      fillFromSavedAccount(Number(pick.getAttribute("data-pick-index")));
      closeTokenDropdown();
    });

    document.addEventListener("click", function (event) {
      if (tokenDropdown.hidden) return;
      if (event.target.closest(".token-field")) return;
      closeTokenDropdown();
    });
  }

  function openTokenDropdown() {
    renderTokenDropdown();
    tokenDropdown.hidden = false;
    tokenCacheBtn.setAttribute("aria-expanded", "true");
  }

  function closeTokenDropdown() {
    tokenDropdown.hidden = true;
    tokenCacheBtn.setAttribute("aria-expanded", "false");
  }

  function renderTokenDropdown() {
    const accounts = state.savedAccounts;
    if (!accounts.length) {
      tokenDropdownList.innerHTML =
        '<li class="token-dropdown-empty">暂无已保存的 Token</li>';
      return;
    }

    tokenDropdownList.innerHTML = accounts
      .map(function (item, index) {
        const safe = escapeHtml(item.token || "");
        return (
          '<li class="token-dropdown-item">' +
          '<button type="button" class="token-dropdown-pick" data-pick-index="' +
          index +
          '" title="' +
          safe +
          '">' +
          safe +
          "</button>" +
          '<button type="button" class="token-dropdown-del" data-delete-index="' +
          index +
          '">删除</button>' +
          "</li>"
        );
      })
      .join("");
  }

  function fillFromSavedAccount(index) {
    const found = state.savedAccounts[index];
    if (!found) return;
    tokenInput.value = found.token || "";
    passwordInput.value = found.password || "";
    passphraseInput.value = found.passphrase || "";
    hideLoginError();
  }

  function deleteSavedAccount(index) {
    const found = state.savedAccounts[index];
    if (!found) return;
    tauriInvoke("saved_accounts_delete", { token: found.token })
      .then(applySavedAccounts)
      .catch(function () {});
  }

  function escapeHtml(value) {
    return String(value)
      .replace(/&/g, "&amp;")
      .replace(/"/g, "&quot;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;");
  }

  function bindPasswordToggle(buttonId, input) {
    const btn = document.getElementById(buttonId);
    btn.addEventListener("click", function () {
      const hidden = input.type === "password";
      input.type = hidden ? "text" : "password";
      btn.textContent = hidden ? "隐藏" : "显示";
      btn.setAttribute("aria-label", hidden ? "隐藏" : "显示");
    });
  }

  function setLoggingIn(busy) {
    loginBtn.disabled = busy;
    loginBtn.textContent = busy ? "登录中..." : "登录";
  }

  function showLoginError(message) {
    errorEl.textContent = message;
    errorEl.hidden = false;
  }

  function hideLoginError() {
    errorEl.hidden = true;
  }

  function withTimeout(promise, ms) {
    return new Promise(function (resolve, reject) {
      var timer = setTimeout(function () {
        reject(new Error("login-timeout"));
      }, ms);
      promise.then(
        function (value) {
          clearTimeout(timer);
          resolve(value);
        },
        function (err) {
          clearTimeout(timer);
          reject(err);
        }
      );
    });
  }

  function authenticate(credentials) {
    return withTimeout(
      tauriInvoke("webrpc_login", {
        token: credentials.token,
        password: credentials.password,
        passphrase: credentials.passphrase || "",
      }),
      18000
    );
  }

  function submitLogin() {
    if (loginBtn.disabled) return;

    const credentials = {
      token: tokenInput.value.trim(),
      password: passwordInput.value.trim(),
      passphrase: passphraseInput.value.trim(),
    };

    if (!credentials.token || !credentials.password) {
      showLoginError("Token 和密码不能为空");
      return;
    }

    closeTokenDropdown();
    hideLoginError();
    setLoggingIn(true);

    authenticate(credentials)
      .then(function () {
        return persistAccount(credentials).catch(function () {});
      })
      .then(function () {
        passwordInput.value = "";
        passphraseInput.value = "";
        enterWorkspace(credentials);
      })
      .catch(function () {
        showLoginError("登陆失败,请检查网络或者token与密码是否正确");
        setLoggingIn(false);
      });
  }

  function enterWorkspace(credentials) {
    state.user = {
      token: credentials.token,
      passphrase: credentials.passphrase,
      loginAt: Date.now(),
    };
    state.sessions = [];
    state.selectedId = null;
    state.pendingFile = null;
    state.connectFillId = null;
    state.sessionsReady = false;
    state.pendingHellos = [];
    state.pendingTexts = [];
    state.inflightFiles = {};
    applySdkSessionCount(0);

    pageLogin.classList.remove("is-active");
    pageLogin.hidden = true;
    pageSessions.hidden = false;
    pageSessions.classList.add("is-active");
    appEl.classList.add("is-workspace");
    resizeTauriWindow(1120, 720);

    renderAccount();
    renderWorkspace();
    loadSavedSessions().then(function () {
      renderAccount();
      renderWorkspace();
    });
  }

  function logout() {
    tauriInvoke("webrpc_logout").catch(function () {});

    stopAllTicks();
    clearConnectTimer();
    revokePreviews(state.sessions);
    state.user = null;
    state.sessions = [];
    state.selectedId = null;
    state.pendingFile = null;
    state.connectFillId = null;
    state.sessionsReady = false;
    state.pendingHellos = [];
    state.pendingTexts = [];
    state.inflightFiles = {};
    state.sendQueue = [];
    state.sendActiveId = null;
    hideDropMask();
    closeMediaOverlay();
    hideVoiceUi();
    hideBubbleMenu();
    applySdkSessionCount(0);
    hideMenu();
    closeModal();

    pageSessions.classList.remove("is-active");
    pageSessions.hidden = true;
    pageLogin.hidden = false;
    pageLogin.classList.add("is-active");
    appEl.classList.remove("is-workspace");
    setLoggingIn(false);
    resizeTauriWindow(960, 620);
    closeTokenDropdown();
    loadSavedAccounts();
  }

  function uid() {
    state.nextId += 1;
    return "id-" + Date.now().toString(36) + "-" + state.nextId.toString(36);
  }

  function ownerToken() {
    return state.user && state.user.token ? state.user.token : "";
  }

  function hydrateSession(item) {
    return {
      id: uid(),
      peerToken: item.peerToken || "",
      remark: item.remark || "",
      connected: false,
      connecting: false,
      connectError: "",
      peerPass: item.peerPass || "",
      rpcSessionId: 0,
      messages: [],
      chatsLoaded: false,
      chatsLoading: null,
      chatEpoch: 0,
    };
  }

  function loadSavedSessions() {
    if (!ownerToken()) {
      state.sessions = [];
      state.selectedId = null;
      return Promise.resolve();
    }
    return tauriInvoke("saved_sessions_list", { ownerToken: ownerToken() })
      .then(function (items) {
        state.sessions = (Array.isArray(items) ? items : []).map(hydrateSession);
        state.selectedId = state.sessions[0] ? state.sessions[0].id : null;
        state.connectFillId = null;
        state.sessionsReady = true;
        flushPendingHellos();
        flushPendingTexts();
      })
      .catch(function () {
        state.sessions = [];
        state.selectedId = null;
        state.sessionsReady = true;
        flushPendingHellos();
        flushPendingTexts();
      });
  }

  function persistSessionCreate(session) {
    return tauriInvoke("saved_sessions_create", {
      ownerToken: ownerToken(),
      peerToken: session.peerToken,
      peerPass: session.peerPass || "",
      remark: session.remark || "",
    }).catch(function (err) {
      if (err && err.message === "webrpc-unavailable") return;
      return Promise.reject(err);
    });
  }

  function persistSessionUpdate(session) {
    if (!session) return Promise.resolve();
    return tauriInvoke("saved_sessions_update", {
      ownerToken: ownerToken(),
      peerToken: session.peerToken,
      peerPass: session.peerPass || "",
      remark: session.remark || "",
    }).catch(function (err) {
      if (err && err.message === "webrpc-unavailable") return;
    });
  }

  function persistSessionDelete(peerToken) {
    return tauriInvoke("saved_sessions_delete", {
      ownerToken: ownerToken(),
      peerToken: peerToken,
    }).catch(function (err) {
      if (err && err.message === "webrpc-unavailable") return;
    });
  }

  function persistableMessage(msg) {
    finalizeCompleteFile(msg);
    return {
      id: msg.id,
      from: msg.from,
      type: msg.type,
      content: msg.content || "",
      title: msg.title || "",
      time: msg.time || 0,
      status: msg.status || "",
      size: msg.size || 0,
      transferred: msg.transferred || 0,
      elapsedMs: msg.elapsedMs || 0,
      speedBps: msg.speedBps || 0,
      filePath: msg.filePath || "",
    };
  }

  function isFileTerminal(status) {
    return status === "sent" || status === "received" || status === "failed";
  }

  function isFileSuccess(status) {
    return status === "sent" || status === "received";
  }

  function finalizeCompleteFile(msg) {
    if (!msg || msg.type === "text") return;
    if (!isFileSuccess(msg.status)) return;
    const size = Number(msg.size) || 0;
    if (size > 0) msg.transferred = size;
    const elapsed = Math.max(Number(msg.elapsedMs) || 0, 1);
    if (!(Number(msg.elapsedMs) > 0)) msg.elapsedMs = elapsed;
    msg.speedBps = Math.round((Number(msg.transferred) / elapsed) * 1000);
  }

  function persistChatAppend(session, msg) {
    if (!session || !ownerToken() || !msg) return;
    tauriInvoke("saved_chats_append", {
      ownerToken: ownerToken(),
      peerToken: session.peerToken,
      message: persistableMessage(msg),
    }).catch(function (err) {
      if (err && err.message === "webrpc-unavailable") return;
    });
  }

  function persistChatUpdate(session, msg) {
    if (!session || !ownerToken() || !msg) return;
    tauriInvoke("saved_chats_update", {
      ownerToken: ownerToken(),
      peerToken: session.peerToken,
      message: persistableMessage(msg),
    }).catch(function (err) {
      if (err && err.message === "webrpc-unavailable") return;
    });
  }

  function persistChatClear(session) {
    if (!session || !ownerToken()) return Promise.resolve();
    return tauriInvoke("saved_chats_clear", {
      ownerToken: ownerToken(),
      peerToken: session.peerToken,
    }).catch(function (err) {
      if (err && err.message === "webrpc-unavailable") return;
    });
  }

  function persistChatDelete(peerToken) {
    if (!ownerToken() || !peerToken) return Promise.resolve();
    return tauriInvoke("saved_chats_delete", {
      ownerToken: ownerToken(),
      peerToken: peerToken,
    }).catch(function (err) {
      if (err && err.message === "webrpc-unavailable") return;
    });
  }

  function persistChatDeleteMessage(session, msgId) {
    if (!session || !ownerToken() || !msgId) return Promise.resolve();
    return tauriInvoke("saved_chats_delete_message", {
      ownerToken: ownerToken(),
      peerToken: session.peerToken,
      messageId: msgId,
    });
  }

  function hydrateMessage(item) {
    item = item || {};
    const msg = {
      id: item.id || uid(),
      from: item.from === "me" ? "me" : "peer",
      type: item.type || "text",
      content: item.content || "",
      title: item.title || "",
      time: Number(item.time) || Date.now(),
      status: item.status || "received",
      size: Number(item.size) || 0,
      transferred: Number(item.transferred) || 0,
      elapsedMs: Number(item.elapsedMs) || 0,
      speedBps: Number(item.speedBps) || 0,
      previewUrl: "",
      filePath: item.filePath || "",
    };
    finalizeCompleteFile(msg);
    return msg;
  }

  function unloadSessionChats(session) {
    if (!session) return;
    session.chatEpoch = (session.chatEpoch || 0) + 1;
    session.chatsLoading = null;
    session.chatsLoaded = false;
    revokePreviews([session]);
    (session.messages || []).forEach(function (msg) {
      stopTick(msg.id);
    });
    session.messages = [];
  }

  function loadSessionChats(session) {
    if (!session || session.chatsLoaded) return Promise.resolve();
    if (session.chatsLoading) return session.chatsLoading;
    const epoch = session.chatEpoch || 0;
    session.chatsLoading = tauriInvoke("saved_chats_load", {
      ownerToken: ownerToken(),
      peerToken: session.peerToken,
    })
      .then(function (items) {
        if ((session.chatEpoch || 0) !== epoch) return;
        const fromDisk = (Array.isArray(items) ? items : []).map(hydrateMessage);
        const seen = {};
        fromDisk.forEach(function (msg) {
          seen[msg.id] = true;
        });
        const extras = (session.messages || []).filter(function (msg) {
          return msg && msg.id && !seen[msg.id];
        });
        session.messages = fromDisk.concat(extras);
        session.chatsLoaded = true;
      })
      .catch(function (err) {
        if ((session.chatEpoch || 0) !== epoch) return;
        session.chatsLoaded = false;
        console.error("load chats failed", invokeErrorText(err));
      })
      .then(function () {
        if ((session.chatEpoch || 0) === epoch) {
          session.chatsLoading = null;
        }
      });
    return session.chatsLoading;
  }

  function selectSession(id) {
    if (state.selectedId && state.selectedId !== id) {
      unloadSessionChats(findSession(state.selectedId));
    }
    state.selectedId = id;
  }

  function invokeErrorText(err) {
    if (err == null) return "";
    if (typeof err === "string") return err;
    return String(err.message || err);
  }

  function onPeerHello(payload) {
    if (!state.user) return;
    if (!state.sessionsReady) {
      state.pendingHellos.push(payload);
      return;
    }
    applyPeerHello(payload);
  }

  function flushPendingHellos() {
    const queued = state.pendingHellos.splice(0, state.pendingHellos.length);
    queued.forEach(applyPeerHello);
  }

  function onPeerText(payload) {
    if (!state.user) return;
    if (!state.sessionsReady) {
      state.pendingTexts.push(payload);
      return;
    }
    applyPeerText(payload);
  }

  function flushPendingTexts() {
    const queued = state.pendingTexts.splice(0, state.pendingTexts.length);
    queued.forEach(applyPeerText);
  }

  function applyPeerText(payload) {
    if (!state.user || !payload) return;
    const sessionId = Number(payload.sessionId) || 0;
    if (!sessionId) return;
    const session = state.sessions.find(function (item) {
      return item.rpcSessionId === sessionId;
    });
    if (!session) return;
    const text = payload.text == null ? "" : String(payload.text);
    const msg = textMsg("peer", text, Date.now(), "received");
    persistChatAppend(session, msg);
    const viewing = state.selectedId === session.id && session.connected;
    if (viewing) {
      session.messages.push(msg);
      if (session.chatsLoaded) renderChat();
    }
  }

  function onFileEvent(payload) {
    if (!state.user || !payload) return;
    const sessionId = Number(payload.sessionId) || 0;
    if (!sessionId) return;
    const session = state.sessions.find(function (item) {
      return item.rpcSessionId === sessionId;
    });
    if (!session) return;
    const from = payload.from === "me" ? "me" : "peer";
    const fileName = String(payload.fileName || "");
    const msgId = String(payload.msgId || "");
    const status = String(payload.status || "");
    const terminal = isFileTerminal(status);
    let msg = null;
    if (msgId && state.inflightFiles[msgId]) {
      msg = state.inflightFiles[msgId].msg;
    }
    if (!msg && msgId && session.chatsLoaded) {
      msg = session.messages.find(function (item) {
        return item.id === msgId;
      });
    }
    if (!msg && from === "peer" && fileName && session.chatsLoaded) {
      msg = session.messages.find(function (item) {
        return item.id === "file-in-" + fileName;
      });
    }
    if (!msg && fileName && session.chatsLoaded) {
      msg = session.messages.find(function (item) {
        return (
          item.title === fileName &&
          item.from === from &&
          item.type !== "text" &&
          (item.status === "sending" || item.status === "receiving")
        );
      });
    }
    if (!msg && fileName && session.chatsLoaded && isFileSuccess(status)) {
      msg = session.messages.slice().reverse().find(function (item) {
        return item.title === fileName && item.from === from && item.type !== "text";
      });
    }
    if (!msg) {
      if (from !== "peer" || !fileName) return;
      if (!session.chatsLoaded) return;
      msg = fileMsg("peer", classifyFile(fileName), fileName, Number(payload.size) || 0, Date.now(), {
        status: status || "receiving",
        transferred: Number(payload.transferred) || 0,
        elapsedMs: Number(payload.elapsedMs) || 0,
        speedBps: Number(payload.speedBps) || 0,
        filePath: payload.filePath || "",
      });
      msg.id = "file-in-" + fileName;
      finalizeCompleteFile(msg);
      persistChatAppend(session, msg);
      if (session.chatsLoaded || state.selectedId === session.id) {
        session.messages.push(msg);
      }
    } else if (!applyFilePayload(msg, payload, status)) {
      return;
    } else {
      persistChatUpdate(session, msg);
    }
    if (terminal && msg) {
      delete state.inflightFiles[msg.id];
      if (msg.from === "me") finishQueuedFileSend(msg.id);
    }
    if (state.selectedId === session.id && session.chatsLoaded) {
      renderChat();
    }
  }

  function applyFilePayload(msg, payload, status) {
    const incomingTerminal = isFileTerminal(status);
    if (msg.status === "sent" && status !== "sent") {
      return false;
    }
    if (msg.status === "received" && status !== "received" && status !== "receiving") {
      return false;
    }
    if (msg.status === "failed" && !incomingTerminal && status !== "sending") {
      return false;
    }
    if (Number(payload.size) > 0) msg.size = Number(payload.size);
    if (status === "failed") {
      msg.status = "failed";
      if (Number(payload.transferred) > 0) msg.transferred = Number(payload.transferred);
      if (Number(payload.elapsedMs) > 0) msg.elapsedMs = Number(payload.elapsedMs);
      if (Number(payload.speedBps) > 0) msg.speedBps = Number(payload.speedBps);
    } else {
      if (payload.transferred != null) {
        const next = Number(payload.transferred) || 0;
        const cur = Number(msg.transferred) || 0;
        if (incomingTerminal || next >= cur) {
          msg.transferred = next;
        }
      }
      if (payload.elapsedMs != null) msg.elapsedMs = Number(payload.elapsedMs) || 0;
      if (payload.speedBps != null) msg.speedBps = Number(payload.speedBps) || 0;
      if (status) msg.status = status;
    }
    if (payload.filePath) msg.filePath = payload.filePath;
    finalizeCompleteFile(msg);
    return true;
  }

  function failSessionTransfers(session) {
    if (!session) return;
    state.sendQueue = state.sendQueue.filter(function (job) {
      if (!job || job.localId !== session.id) return true;
      if (job.msg && job.msg.status === "sending") {
        job.msg.status = "failed";
        persistChatUpdate(session, job.msg);
      }
      return false;
    });
    if (state.sendActiveId) {
      const active = (session.messages || []).find(function (item) {
        return item.id === state.sendActiveId;
      });
      if (active) state.sendActiveId = null;
    }
    session.messages.forEach(function (msg) {
      if (msg.status === "sending") {
        stopTick(msg.id);
        msg.status = "failed";
        persistChatUpdate(session, msg);
      }
    });
    Object.keys(state.inflightFiles).forEach(function (id) {
      const item = state.inflightFiles[id];
      if (!item || item.localId !== session.id) return;
      delete state.inflightFiles[id];
      if (isFileSuccess(item.msg.status)) return;
      item.msg.status = "failed";
      persistChatUpdate(session, item.msg);
    });
    pumpSendQueue();
  }

  function applyPeerHello(payload) {
    if (!state.user || !payload) return;
    const token = String(payload.token || "").trim();
    const sessionId = Number(payload.sessionId) || 0;
    const permission = String(payload.permission || "").trim();
    if (!token || sessionId <= 0) return;
    if (token === state.user.token) return;

    let session = state.sessions.find(function (item) {
      return item.peerToken === token;
    });
    const existed = !!session;
    if (!session) {
      session = {
        id: uid(),
        peerToken: token,
        remark: "",
        connected: false,
        connecting: false,
        connectError: "",
        peerPass: permission,
        rpcSessionId: 0,
        messages: [],
        chatsLoaded: false,
        chatsLoading: null,
        chatEpoch: 0,
      };
      state.sessions.unshift(session);
    }

    session.connected = true;
    session.connecting = false;
    session.connectError = "";
    session.rpcSessionId = sessionId;
    session.peerPass = permission;

    if (existed) {
      persistSessionUpdate(session);
    } else {
      persistSessionCreate(session).catch(function (err) {
        if (invokeErrorText(err).indexOf("session-exists") >= 0) {
          persistSessionUpdate(session);
        }
      });
    }

    renderSessionList();
    if (state.selectedId === session.id) {
      renderChat();
    }
  }

  function onSessionDead(sessionId) {
    const sid = Number(sessionId) || 0;
    if (!sid || !state.user) return;
    const session = state.sessions.find(function (item) {
      return item.rpcSessionId === sid;
    });
    if (!session) return;
    session.connected = false;
    session.connecting = false;
    session.rpcSessionId = 0;
    session.connectError = "对端已断开连接，请重新连接。";
    failSessionTransfers(session);
    renderSessionList();
    if (state.selectedId === session.id) {
      state.connectFillId = null;
      renderChat();
    }
  }

  function textMsg(from, content, time, status) {
    return {
      id: uid(),
      from: from,
      type: "text",
      content: content,
      title: "",
      time: time,
      status: status,
      size: 0,
      transferred: 0,
      elapsedMs: 0,
      speedBps: 0,
      previewUrl: "",
      filePath: "",
    };
  }

  function fileMsg(from, type, title, size, time, extra) {
    extra = extra || {};
    return {
      id: uid(),
      from: from,
      type: type,
      content: "",
      title: title,
      time: time,
      status: extra.status || "sent",
      size: size,
      transferred: extra.transferred != null ? extra.transferred : size,
      elapsedMs: extra.elapsedMs || 0,
      speedBps: extra.speedBps || 0,
      previewUrl: extra.previewUrl || "",
      filePath: extra.filePath || "",
    };
  }

  function bindWorkspaceEvents() {
    document.getElementById("btn-logout").addEventListener("click", logout);
    document.getElementById("btn-new-session").addEventListener("click", function () {
      openModal("new");
    });
    document.getElementById("btn-create-session").addEventListener("click", createSession);
    document.getElementById("btn-save-remark").addEventListener("click", saveRemark);
    confirmOkBtn.addEventListener("click", onConfirmOk);
    connectBtn.addEventListener("click", connectSelected);
    document.getElementById("btn-voice").addEventListener("click", startVoiceCall);
    document.getElementById("voice-accept").addEventListener("click", function () {
      tauriInvoke("voice_accept").catch(showVoiceError);
    });
    document.getElementById("voice-reject").addEventListener("click", function () {
      tauriInvoke("voice_reject").catch(showVoiceError);
    });
    document.getElementById("voice-cancel").addEventListener("click", function () {
      tauriInvoke("voice_hangup").catch(showVoiceError);
    });
    document.getElementById("voice-hangup").addEventListener("click", function () {
      tauriInvoke("voice_hangup").catch(showVoiceError);
    });
    document.getElementById("voice-mute").addEventListener("click", function () {
      const next = !state.voiceMuted;
      tauriInvoke("voice_set_mute", { muted: next }).catch(showVoiceError);
    });
    document.getElementById("btn-file").addEventListener("click", pickPendingFile);
    document.getElementById("btn-shot").addEventListener("click", function () {
      startScreenshot(false);
    });
    document.getElementById("btn-shot-caret").addEventListener("click", toggleShotMenu);
    document.getElementById("shot-menu").addEventListener("click", function (event) {
      const item = event.target.closest("[data-shot-hide]");
      if (!item) return;
      startScreenshot(true);
    });
    document.getElementById("pending-file-clear").addEventListener("click", clearPendingFile);
    fileInput.addEventListener("change", onFileChosen);
    composer.addEventListener("submit", function (event) {
      event.preventDefault();
      sendComposer();
    });
    composerInput.addEventListener("keydown", function (event) {
      if (event.key === "Enter" && !event.shiftKey) {
        event.preventDefault();
        sendComposer();
      }
    });

    sessionListEl.addEventListener("click", onSessionListClick);
    chatLog.addEventListener("click", onChatLogClick);
    chatLog.addEventListener("contextmenu", onChatLogContextMenu);
    chatLog.addEventListener("scroll", hideBubbleMenu);
    if (bubbleMenu) {
      bubbleMenu.addEventListener("click", onBubbleMenuClick);
      bubbleMenu.addEventListener("contextmenu", function (event) {
        event.preventDefault();
      });
    }
    window.addEventListener("resize", function () {
      hideBubbleMenu();
      refreshCopyButtons();
    });
    document.addEventListener("keydown", function (event) {
      if (event.key === "Escape") {
        hideBubbleMenu();
        hideShotMenu();
        closeMediaOverlay();
      }
      if (isScreenshotHotkey(event)) {
        const session = findSession(state.selectedId);
        if (!session || !session.connected) return;
        event.preventDefault();
        startScreenshot();
      }
    });
    if (mediaOverlay) {
      mediaOverlay.addEventListener("click", function (event) {
        if (event.target.closest("[data-close-media]")) {
          event.preventDefault();
          closeMediaOverlay();
        }
      });
    }
    chatLog.addEventListener(
      "error",
      function (event) {
        const img = event.target;
        if (!img || img.tagName !== "IMG" || !img.closest(".file-preview")) return;
        const preview = img.closest(".file-preview");
        if (!preview) return;
        preview.outerHTML =
          '<div class="file-icon is-image">' + KIND_BADGE.image + "</div>";
      },
      true
    );
    document.addEventListener("click", function (event) {
      if (state.menuSessionId) {
        if (!(event.target.closest(".session-dropdown") || event.target.closest(".session-gear"))) {
          hideMenu();
        }
      }
      if (bubbleMenu && !bubbleMenu.hidden && !event.target.closest("#bubble-menu")) {
        hideBubbleMenu();
      }
      const shotSplit = document.getElementById("shot-split");
      if (shotSplit && !shotSplit.contains(event.target)) {
        hideShotMenu();
      }
    });
    modalRoot.querySelectorAll("[data-close-modal]").forEach(function (el) {
      el.addEventListener("click", closeModal);
    });
    newPeerToken.addEventListener("keydown", function (event) {
      if (event.key === "Enter") createSession();
    });
    newPeerToken.addEventListener("input", function () {
      newSessionError.hidden = true;
    });
    newPeerPass.addEventListener("input", function () {
      newSessionError.hidden = true;
    });
    remarkInput.addEventListener("keydown", function (event) {
      if (event.key === "Enter") saveRemark();
    });
  }

  function onSessionListClick(event) {
    const actionBtn = event.target.closest(".session-dropdown [data-action]");
    if (actionBtn) {
      event.stopPropagation();
      const row = actionBtn.closest(".session-row");
      const id = row && row.getAttribute("data-id");
      handleMenuAction(id, actionBtn.getAttribute("data-action"));
      return;
    }

    const row = event.target.closest(".session-row");
    if (!row) return;
    const id = row.getAttribute("data-id");
    const gear = event.target.closest(".session-gear");

    if (gear) {
      event.stopPropagation();
      selectSession(id);
      state.menuSessionId = state.menuSessionId === id ? null : id;
      renderWorkspace();
      return;
    }

    selectSession(id);
    state.menuSessionId = null;
    renderWorkspace();
  }

  function handleMenuAction(id, action) {
    hideMenu();
    if (!id) return;
    if (action === "remark") {
      const session = findSession(id);
      state.remarkSessionId = id;
      remarkInput.value = session ? session.remark : "";
      openModal("remark");
    } else if (action === "close") {
      closeSession(id);
    } else if (action === "clear") {
      openConfirm("clear", id);
    } else if (action === "delete") {
      openConfirm("delete", id);
    }
  }

  function renderAccount() {
    if (!state.user) return;
    infoToken.textContent = state.user.token;
    infoToken.title = state.user.token;
    infoPassphrase.textContent = state.user.passphrase ? state.user.passphrase : "未设置";
    infoPassphrase.title = infoPassphrase.textContent;
    infoLoginTime.textContent = formatDateTime(state.user.loginAt);
    infoSessionCount.textContent = String(state.sdkSessionCount);
  }

  function renderWorkspace() {
    renderAccount();
    renderSessionList();
    renderChat();
  }

  function renderSessionList() {
    if (!state.sessions.length) {
      sessionListEl.innerHTML = '<div class="session-list-empty">暂无会话<br />点击「新建会话」连接对端</div>';
      return;
    }
    sessionListEl.innerHTML = state.sessions
      .map(function (session) {
        const title = session.remark || session.peerToken;
        const sub = session.remark ? session.peerToken : "未备注";
        const active = session.id === state.selectedId ? " is-active" : "";
        const on = session.connected ? " is-on" : "";
        const menuOpen = session.id === state.menuSessionId;
        const closeBtn = session.connected
          ? '<button type="button" data-action="close">关闭会话</button>'
          : "";
        const menu = menuOpen
          ? '<div class="session-dropdown">' +
            '<button type="button" data-action="remark">设置备注</button>' +
            closeBtn +
            '<button type="button" data-action="clear">清空内容</button>' +
            '<button type="button" class="is-danger" data-action="delete">删除会话</button>' +
            "</div>"
          : "";
        return (
          '<div class="session-row' +
          (menuOpen ? " is-menu-open" : "") +
          '" data-id="' +
          escapeHtml(session.id) +
          '">' +
          '<div class="session-item' +
          active +
          '">' +
          '<i class="status-dot' +
          on +
          '"></i>' +
          '<div class="session-item-body">' +
          '<span class="session-item-title">' +
          escapeHtml(title) +
          "</span>" +
          '<span class="session-item-sub">' +
          escapeHtml(sub) +
          "</span>" +
          "</div>" +
          '<button class="session-gear" type="button" aria-label="会话设置">⚙</button>' +
          "</div>" +
          menu +
          "</div>"
        );
      })
      .join("");
  }

  function showPanel(name) {
    panelUnselected.classList.toggle("is-visible", name === "unselected");
    panelConnect.classList.toggle("is-visible", name === "connect");
    chatMain.classList.toggle("is-visible", name === "chat");
  }

  function renderChat() {
    const session = findSession(state.selectedId);
    if (!session) {
      showPanel("unselected");
      return;
    }

    if (!session.connected) {
      if (session.chatsLoaded || session.chatsLoading || (session.messages && session.messages.length)) {
        unloadSessionChats(session);
      }
      showPanel("connect");
      const name = session.remark || session.peerToken;
      connectPeer.textContent = "对方 Token：" + session.peerToken;
      if (state.connectFillId !== session.id) {
        state.connectFillId = session.id;
        connectPeerPass.type = "password";
        connectPeerPass.value = session.peerPass || "";
        const toggle = document.getElementById("toggle-connect-pass");
        if (toggle) {
          toggle.textContent = "显示";
          toggle.setAttribute("aria-label", "显示认证口令");
        }
      }
      connectPeerPass.disabled = !!session.connecting;
      connectBtn.disabled = !!session.connecting;
      connectBtn.textContent = session.connecting ? "连接中..." : "连接";
      if (session.connectError) {
        connectError.hidden = false;
        connectError.textContent = session.connectError;
      } else {
        connectError.hidden = true;
      }
      connectPeer.setAttribute("title", name);
      return;
    }

    showPanel("chat");
    chatTitle.textContent = session.remark || session.peerToken;
    chatSubtitle.textContent = session.remark ? session.peerToken : "未备注";
    chatStatusDot.classList.add("is-on");
    chatConnLabel.textContent = "已连接";
    chatConnLabel.classList.add("is-on");
    composerInput.disabled = false;
    document.getElementById("btn-file").disabled = false;
    document.getElementById("btn-shot").disabled = false;
    document.getElementById("btn-shot-caret").disabled = false;
    document.getElementById("btn-voice").disabled = false;
    document.getElementById("btn-send").disabled = false;
    composerInput.placeholder = "输入消息，Enter 发送，Shift+Enter 换行";
    if (!session.chatsLoaded) {
      chatLog.innerHTML = "";
      loadSessionChats(session).then(function () {
        if (state.selectedId === session.id && session.connected && session.chatsLoaded) {
          renderChat();
        }
      });
      renderPendingFile();
      return;
    }
    chatLog.innerHTML = session.messages.map(renderMessage).join("");
    chatLog.scrollTop = chatLog.scrollHeight;
    renderPendingFile();
    hideBubbleMenu();
    window.requestAnimationFrame(function () {
      refreshCopyButtons();
      refreshFolderButtons();
    });
  }

  function renderMessage(msg) {
    const mine = msg.from === "me";
    const rowClass = "msg-row" + (mine ? " is-me" : "");
    const inner = msg.type === "text" ? renderTextInner(msg) : renderFileInner(msg);
    const bubbleClass =
      "msg-bubble" +
      (msg.type === "text" ? " is-text" : " is-file") +
      (mine && msg.status === "failed" ? " has-resend" : "");
    const bubbleAttrs = ' data-msg-id="' + escapeHtml(msg.id) + '"';
    return (
      '<div class="' +
      rowClass +
      '"><div class="msg-body"><div class="' +
      bubbleClass +
      '"' +
      bubbleAttrs +
      ">" +
      inner +
      "</div>" +
      renderMeta(msg, mine) +
      "</div></div>"
    );
  }

  function renderTextInner(msg) {
    return (
      '<button type="button" class="msg-copy" data-copy-msg="' +
      escapeHtml(msg.id) +
      '" aria-label="复制全文">复制</button>' +
      '<button type="button" class="msg-resend" data-resend-msg="' +
      escapeHtml(msg.id) +
      '" aria-label="重新发送">重发</button>' +
      '<div class="msg-text">' +
      escapeHtml(msg.content) +
      "</div>"
    );
  }

  function visualLineCount(el) {
    const style = window.getComputedStyle(el);
    let lineHeight = parseFloat(style.lineHeight);
    if (!lineHeight || Number.isNaN(lineHeight)) {
      lineHeight = parseFloat(style.fontSize) * 1.55;
    }
    const height = el.getBoundingClientRect().height;
    if (height <= 0 || lineHeight <= 0) return 0;
    return Math.round(height / lineHeight);
  }

  function refreshCopyButtons() {
    if (!chatLog) return;
    chatLog.querySelectorAll(".msg-bubble.is-text").forEach(function (bubble) {
      const textEl = bubble.querySelector(".msg-text");
      bubble.classList.toggle("has-copy", !!(textEl && visualLineCount(textEl) >= COPY_MIN_LINES));
    });
  }

  function onChatLogContextMenu(event) {
    const bubble = event.target.closest(".msg-bubble");
    if (!bubble || !chatLog.contains(bubble)) return;
    event.preventDefault();
    event.stopPropagation();
    const msgId = bubble.getAttribute("data-msg-id");
    if (!msgId) return;
    showBubbleMenu(msgId, event.clientX, event.clientY);
  }

  function showBubbleMenu(msgId, x, y) {
    if (!bubbleMenu) return;
    state.deleteMessageId = msgId;
    bubbleMenu.hidden = false;
    const pad = 8;
    const rect = bubbleMenu.getBoundingClientRect();
    const width = rect.width || 120;
    const height = rect.height || 40;
    let left = x;
    let top = y;
    if (left + width + pad > window.innerWidth) left = Math.max(pad, window.innerWidth - width - pad);
    if (top + height + pad > window.innerHeight) top = Math.max(pad, window.innerHeight - height - pad);
    bubbleMenu.style.left = left + "px";
    bubbleMenu.style.top = top + "px";
  }

  function hideBubbleMenu() {
    if (!bubbleMenu || bubbleMenu.hidden) return;
    bubbleMenu.hidden = true;
  }

  function onBubbleMenuClick(event) {
    const btn = event.target.closest("[data-bubble-action]");
    if (!btn) return;
    event.preventDefault();
    event.stopPropagation();
    const msgId = state.deleteMessageId;
    hideBubbleMenu();
    if (btn.getAttribute("data-bubble-action") === "delete") {
      requestDeleteMessage(msgId);
    }
  }

  function requestDeleteMessage(msgId) {
    const msg = findChatMessage(msgId);
    if (!msg) return;
    if (msg.status === "sending" || msg.status === "receiving") {
      openBusyDeletePrompt(msg);
      return;
    }
    openDeleteMessageConfirm(msg);
  }

  function openInfoPrompt(title, desc) {
    state.confirmKind = "info";
    confirmTitle.textContent = title;
    confirmDesc.textContent = desc;
    confirmOkBtn.textContent = "知道了";
    confirmOkBtn.className = "btn-primary";
    if (confirmCancelBtn) confirmCancelBtn.hidden = true;
    openModal("confirm");
  }

  function openBusyDeletePrompt(msg) {
    state.confirmKind = "busy-message";
    state.deleteMessageId = msg.id;
    confirmTitle.textContent = "无法删除";
    confirmDesc.textContent =
      msg.type === "text"
        ? "消息正在发送中，请稍后再删除。"
        : msg.from === "me"
          ? "文件正在发送中，请等待发送完成后再删除这条记录。"
          : "文件正在接收中，请等待接收完成后再删除这条记录。";
    confirmOkBtn.textContent = "知道了";
    confirmOkBtn.className = "btn-primary";
    if (confirmCancelBtn) confirmCancelBtn.hidden = true;
    openModal("confirm");
  }

  function openDeleteMessageConfirm(msg) {
    state.confirmKind = "delete-message";
    state.deleteMessageId = msg.id;
    confirmTitle.textContent = "删除这条消息";
    if (msg.type === "text") {
      confirmDesc.textContent = "确定删除这条消息吗？删除后无法从会话中恢复。";
    } else if (msg.from === "me") {
      confirmDesc.textContent =
        "确定删除这条发送记录吗？只会从会话中移除这条气泡，不会删除你电脑上的原始文件。";
    } else {
      confirmDesc.textContent =
        "确定删除这条接收记录吗？会话中的气泡和本地缓存文件都会被删除，删除后无法恢复。";
    }
    confirmOkBtn.textContent = "删除";
    confirmOkBtn.className = "btn-danger";
    if (confirmCancelBtn) confirmCancelBtn.hidden = false;
    openModal("confirm");
  }

  function onChatLogClick(event) {
    const folderBtn = event.target.closest(".msg-folder");
    if (folderBtn) {
      event.preventDefault();
      event.stopPropagation();
      revealMessageFile(folderBtn.getAttribute("data-folder-msg"));
      return;
    }
    const resendBtn = event.target.closest(".msg-resend");
    if (resendBtn) {
      event.preventDefault();
      resendMessage(resendBtn.getAttribute("data-resend-msg"));
      return;
    }
    const copyBtn = event.target.closest(".msg-copy");
    if (copyBtn) {
      event.preventDefault();
      copyMessageText(copyBtn.getAttribute("data-copy-msg"), copyBtn);
      return;
    }
    const preview = event.target.closest("[data-open-media]");
    if (preview) {
      event.preventDefault();
      openMediaOverlay(preview.getAttribute("data-open-media"));
    }
  }

  function resendMessage(msgId) {
    const session = findSession(state.selectedId);
    if (!session || !session.connected) return;
    const msg = session.messages.find(function (item) {
      return item.id === msgId && item.from === "me";
    });
    if (!msg || msg.status !== "failed") return;
    msg.status = "sending";
    msg.transferred = 0;
    msg.elapsedMs = 0;
    msg.speedBps = 0;
    persistChatUpdate(session, msg);
    renderChat();
    if (msg.type === "text") {
      sendTextMessage(session, msg);
      return;
    }
    sendFileMessage(session, msg);
  }

  function sendTextMessage(session, msg) {
    const localId = session.id;
    const rpcId = session.rpcSessionId || 0;
    return tauriInvoke("webrpc_send_data", { sessionId: rpcId, text: msg.content || "" })
      .then(function (ok) {
        msg.status = ok ? "sent" : "failed";
        persistChatUpdate(session, msg);
        if (state.selectedId === localId && session.chatsLoaded) renderChat();
      })
      .catch(function () {
        msg.status = "failed";
        persistChatUpdate(session, msg);
        if (state.selectedId === localId && session.chatsLoaded) renderChat();
      });
  }

  function copyMessageText(msgId, btn) {
    const session = findSession(state.selectedId);
    if (!session) return;
    const msg = session.messages.find(function (item) {
      return item.id === msgId && item.type === "text";
    });
    if (!msg) return;
    copyText(msg.content || "").then(function () {
      btn.textContent = "已复制";
      btn.classList.add("is-done");
      window.setTimeout(function () {
        if (!btn.isConnected) return;
        btn.textContent = "复制";
        btn.classList.remove("is-done");
      }, 1200);
    }).catch(function () {});
  }

  function copyText(text) {
    if (navigator.clipboard && navigator.clipboard.writeText) {
      return navigator.clipboard.writeText(text).catch(function () {
        return copyTextFallback(text);
      });
    }
    return copyTextFallback(text);
  }

  function copyTextFallback(text) {
    return new Promise(function (resolve, reject) {
      const ta = document.createElement("textarea");
      ta.value = text;
      ta.setAttribute("readonly", "");
      ta.style.position = "fixed";
      ta.style.left = "-9999px";
      document.body.appendChild(ta);
      ta.select();
      try {
        if (document.execCommand("copy")) resolve();
        else reject(new Error("copy-failed"));
      } catch (err) {
        reject(err);
      }
      document.body.removeChild(ta);
    });
  }

  function renderFileInner(msg) {
    const badge = KIND_BADGE[msg.type] || "FILE";
    const previewSrc = imagePreviewSrc(msg);
    const openable = canOpenLocalFile(msg);
    const folderBtn =
      '<button type="button" class="msg-folder" data-folder-msg="' +
      escapeHtml(msg.id) +
      '" aria-label="打开所在目录">' +
      '<svg viewBox="0 0 24 24" aria-hidden="true"><path fill="currentColor" d="M10 4H4c-1.1 0-2 .9-2 2v12c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V8c0-1.1-.9-2-2-2h-8l-2-2z"></path></svg>' +
      "</button>";
    let visual;
    if (msg.type === "image" && previewSrc) {
      visual =
        '<button type="button" class="file-preview" data-open-media="' +
        escapeHtml(msg.id) +
        '"' +
        (openable ? "" : " disabled") +
        ' aria-label="查看图片">' +
        '<img alt="" src="' +
        escapeHtml(previewSrc) +
        '" />' +
        "</button>";
    } else if (msg.type === "video") {
      visual =
        '<div class="file-icon is-video"' +
        (openable ? ' data-open-media="' + escapeHtml(msg.id) + '"' : "") +
        '><span class="video-glyph"></span></div>';
    } else {
      visual =
        '<div class="file-icon is-' +
        escapeHtml(msg.type) +
        '">' +
        badge +
        "</div>";
    }
    const busy = msg.status === "sending" || msg.status === "receiving";
    const percent = msg.size ? Math.min(100, Math.round((msg.transferred / msg.size) * 100)) : 0;
    const movedLabel = msg.from === "me" ? "已发送" : "已接收";
    const xfer =
      '<div class="file-xfer">' +
      movedLabel +
      " " +
      formatSize(msg.transferred) +
      " / " +
      formatSize(msg.size) +
      "<br />耗时 " +
      formatDuration(msg.elapsedMs) +
      " · 平均 " +
      formatSpeed(msg.speedBps) +
      "</div>";
    const bar = busy
      ? '<div class="xfer-bar"><span style="width:' + percent + '%"></span></div>'
      : "";
    const cardClass =
      "file-card" +
      (msg.type === "image" && previewSrc ? " is-image" : "") +
      (openable ? " is-openable" : "");
    return (
      folderBtn +
      '<button type="button" class="msg-resend" data-resend-msg="' +
      escapeHtml(msg.id) +
      '" aria-label="重新发送">重发</button>' +
      '<div class="' +
      cardClass +
      '">' +
      visual +
      '<div class="file-info">' +
      '<div class="file-title" title="' +
      escapeHtml(msg.title) +
      '">' +
      escapeHtml(truncateBytes(msg.title, TITLE_MAX_BYTES)) +
      "</div>" +
      '<div class="file-size">' +
      formatSize(msg.size) +
      "</div>" +
      xfer +
      bar +
      "</div></div>"
    );
  }

  function renderMeta(msg, mine) {
    const time = '<span>' + formatClock(msg.time) + "</span>";
    if (!mine && msg.type === "text") {
      return '<div class="msg-meta">' + time + "</div>";
    }
    const status = statusLabel(msg);
    return '<div class="msg-meta">' + time + status + "</div>";
  }

  function statusLabel(msg) {
    if (msg.type !== "text") {
      if (msg.from === "me") {
        if (msg.status === "sent") {
          return '<span class="msg-status is-sent">发送成功</span>';
        }
        if (msg.status === "failed") {
          return '<span class="msg-status is-failed">发送失败</span>';
        }
        if (msg.status === "sending") {
          return '<span class="msg-status is-busy">发送中</span>';
        }
      } else {
        if (msg.status === "received") {
          return '<span class="msg-status is-sent">接收成功</span>';
        }
        if (msg.status === "receiving") {
          return '<span class="msg-status is-busy">接收中</span>';
        }
        if (msg.status === "failed") {
          return '<span class="msg-status is-failed">接收失败</span>';
        }
      }
      return "";
    }
    if (msg.status === "sent" || msg.status === "received") {
      return '<span class="msg-status is-sent">已发送</span>';
    }
    if (msg.status === "failed") {
      return '<span class="msg-status is-failed">发送失败</span>';
    }
    if (msg.status === "sending") {
      return '<span class="msg-status is-busy">发送中</span>';
    }
    if (msg.status === "receiving") {
      return '<span class="msg-status is-busy">接收中</span>';
    }
    return "";
  }

  function hideMenu() {
    if (!state.menuSessionId) return;
    state.menuSessionId = null;
    renderSessionList();
  }

  function clearConnectTimer() {
    if (state.connectTimer) {
      window.clearTimeout(state.connectTimer);
      state.connectTimer = null;
    }
  }

  function connectSelected() {
    const session = findSession(state.selectedId);
    if (!session || session.connected || session.connecting) return;
    session.peerPass = connectPeerPass.value.trim();
    session.connectError = "";
    session.connecting = true;
    renderWorkspace();

    const localId = session.id;
    tauriInvoke("webrpc_open_session", {
      peerToken: session.peerToken,
      passphrase: session.peerPass || "",
    })
      .then(function (sessionId) {
        const sid = Number(sessionId) || 0;
        const current = findSession(localId);
        if (!current || !current.connecting) {
          if (sid) {
            tauriInvoke("webrpc_close_session", { sessionId: sid }).catch(function () {});
          }
          return;
        }
        if (sid <= 0) {
          current.connecting = false;
          current.connected = false;
          current.rpcSessionId = 0;
          current.connectError =
            "连接失败。请确认对方 Token 是否在线，以及当前网络是否可达。";
          renderWorkspace();
          return;
        }
        current.connecting = false;
        current.connected = true;
        current.rpcSessionId = sid;
        current.connectError = "";
        persistSessionUpdate(current);
        renderWorkspace();
      })
      .catch(function (err) {
        const current = findSession(localId);
        if (!current) return;
        current.connecting = false;
        current.connected = false;
        current.rpcSessionId = 0;
        current.connectError =
          invokeErrorText(err).indexOf("handshake-send-failed") >= 0
            ? "会话通信异常，通知消息未能送达，连接已关闭。请检查网络后重试。"
            : "连接失败。请确认对方 Token 是否在线，以及当前网络是否可达。";
        renderWorkspace();
      });
  }

  function releaseRpcSession(session) {
    const sid = session && session.rpcSessionId ? session.rpcSessionId : 0;
    if (session) session.rpcSessionId = 0;
    if (!sid) return Promise.resolve();
    return tauriInvoke("webrpc_close_session", { sessionId: sid }).catch(function () {});
  }

  function openModal(name) {
    modalRoot.hidden = false;
    modalNew.hidden = name !== "new";
    modalRemark.hidden = name !== "remark";
    modalConfirm.hidden = name !== "confirm";
    newSessionError.hidden = true;
    if (name === "new") {
      newPeerToken.value = "";
      newPeerPass.value = "";
      window.setTimeout(function () {
        newPeerToken.focus();
      }, 0);
    }
    if (name === "remark") {
      window.setTimeout(function () {
        remarkInput.focus();
        remarkInput.select();
      }, 0);
    }
  }

  function openConfirm(kind, sessionId) {
    state.confirmKind = kind;
    confirmOkBtn.className = "btn-danger";
    if (confirmCancelBtn) confirmCancelBtn.hidden = false;
    if (kind === "clear") {
      state.clearSessionId = sessionId;
      confirmTitle.textContent = "清空内容";
      confirmDesc.textContent =
        "将清空本会话在本地的聊天记录（含文本和文件预览）。不会断开连接，也不会删除会话、备注或 Token。";
      confirmOkBtn.textContent = "清空";
    } else {
      state.deleteSessionId = sessionId;
      confirmTitle.textContent = "删除会话";
      confirmDesc.textContent = "删除后将从列表和本地缓存中移除。";
      confirmOkBtn.textContent = "删除";
    }
    openModal("confirm");
  }

  function closeModal() {
    modalRoot.hidden = true;
    modalNew.hidden = true;
    modalRemark.hidden = true;
    modalConfirm.hidden = true;
  }

  function onConfirmOk() {
    if (state.confirmKind === "busy-message" || state.confirmKind === "info") {
      state.confirmKind = "";
      state.deleteMessageId = null;
      closeModal();
      return;
    }
    if (state.confirmKind === "delete-message") {
      confirmDeleteMessage();
      return;
    }
    if (state.confirmKind === "clear") {
      confirmClear();
      return;
    }
    confirmDelete();
  }

  function confirmDeleteMessage() {
    const session = findSession(state.selectedId);
    const msgId = state.deleteMessageId;
    const msg = findChatMessage(msgId);
    state.confirmKind = "";
    closeModal();
    if (!session || !msg) {
      state.deleteMessageId = null;
      return;
    }
    if (msg.status === "sending" || msg.status === "receiving") {
      openBusyDeletePrompt(msg);
      return;
    }
    persistChatDeleteMessage(session, msg.id)
      .then(function () {
        if (mediaOverlay && !mediaOverlay.hidden) closeMediaOverlay();
        stopTick(msg.id);
        if (msg.previewUrl && msg.previewUrl.indexOf("blob:") === 0) {
          URL.revokeObjectURL(msg.previewUrl);
        }
        delete state.inflightFiles[msg.id];
        session.messages = (session.messages || []).filter(function (item) {
          return item.id !== msg.id;
        });
        state.deleteMessageId = null;
        if (state.selectedId === session.id && session.chatsLoaded) renderChat();
      })
      .catch(function (err) {
        const text = invokeErrorText(err);
        state.deleteMessageId = null;
        if (text.indexOf("transfer-in-progress") >= 0) {
          openBusyDeletePrompt(msg);
          return;
        }
        if (state.selectedId === session.id && session.chatsLoaded) renderChat();
      });
  }

  function confirmClear() {
    const id = state.clearSessionId || state.selectedId;
    const session = findSession(id);
    state.clearSessionId = null;
    state.confirmKind = "";
    closeModal();
    if (!session) return;
    session.messages.forEach(function (msg) {
      stopTick(msg.id);
      if (msg.previewUrl && msg.previewUrl.indexOf("blob:") === 0) {
        URL.revokeObjectURL(msg.previewUrl);
      }
    });
    session.messages = [];
    persistChatClear(session);
    renderWorkspace();
  }

  function createSession() {
    const token = newPeerToken.value.trim();
    const pass = newPeerPass.value.trim();
    if (!token) {
      newSessionError.textContent = "请输入对方 Token";
      newSessionError.hidden = false;
      return;
    }
    if (
      state.sessions.some(function (item) {
        return item.peerToken === token;
      })
    ) {
      newSessionError.textContent = "该会话已存在";
      newSessionError.hidden = false;
      return;
    }

    const session = {
      id: uid(),
      peerToken: token,
      remark: "",
      connected: false,
      connecting: false,
      connectError: "",
      peerPass: pass,
      rpcSessionId: 0,
      messages: [],
      chatsLoaded: false,
      chatsLoading: null,
      chatEpoch: 0,
    };

    persistSessionCreate(session)
      .then(function () {
        state.sessions.unshift(session);
        selectSession(session.id);
        state.connectFillId = null;
        closeModal();
        renderWorkspace();
      })
      .catch(function (err) {
        const text = String((err && err.message) || err || "");
        newSessionError.textContent =
          text.indexOf("session-exists") >= 0 ? "该会话已存在" : "保存会话失败";
        newSessionError.hidden = false;
      });
  }

  function saveRemark() {
    const session = findSession(state.remarkSessionId);
    if (session) {
      session.remark = remarkInput.value.trim();
      persistSessionUpdate(session);
    }
    closeModal();
    renderWorkspace();
  }

  function closeSession(id) {
    const session = findSession(id);
    if (!session) return;
    releaseRpcSession(session);
    session.connected = false;
    session.connecting = false;
    session.connectError = "";
    failSessionTransfers(session);
    renderWorkspace();
  }

  function confirmDelete() {
    const id = state.deleteSessionId || state.selectedId;
    const index = state.sessions.findIndex(function (s) {
      return s.id === id;
    });
    if (index < 0) {
      state.deleteSessionId = null;
      state.confirmKind = "";
      closeModal();
      return;
    }
    const removed = state.sessions[index];
    releaseRpcSession(removed).then(function () {
      const still = state.sessions.findIndex(function (s) {
        return s.id === id;
      });
      if (still >= 0) {
        state.sessions.splice(still, 1);
      }
      persistSessionDelete(removed.peerToken);
      persistChatDelete(removed.peerToken);
      revokePreviews([removed]);
      removed.messages.forEach(function (msg) {
        stopTick(msg.id);
      });
      if (state.selectedId === id) {
        state.connectFillId = null;
        selectSession(state.sessions[0] ? state.sessions[0].id : null);
      }
      state.deleteSessionId = null;
      state.confirmKind = "";
      closeModal();
      renderWorkspace();
    });
  }

  function pickPendingFile() {
    function applyPicked(path) {
      if (!path) return;
      tauriInvoke("file_stat", { path: path })
        .then(function (info) {
          if (!info || !info.path) return;
          if (state.pendingFile && state.pendingFile.previewUrl) {
            URL.revokeObjectURL(state.pendingFile.previewUrl);
          }
          state.pendingFile = {
            path: info.path,
            name: info.name,
            size: info.size,
            type: classifyFile(info.name),
            previewUrl: "",
          };
          renderPendingFile();
        })
        .catch(function () {});
    }
    try {
      if (window.__TAURI__ && window.__TAURI__.dialog && window.__TAURI__.dialog.open) {
        window.__TAURI__.dialog.open({ multiple: false }).then(function (path) {
          if (Array.isArray(path)) path = path[0];
          applyPicked(path);
        });
        return;
      }
    } catch (_) {}
    fileInput.click();
  }

  function sendFileMessage(session, msg) {
    if (!session || !msg || !msg.filePath) {
      if (msg) {
        msg.status = "failed";
        persistChatUpdate(session, msg);
        if (state.selectedId === session.id && session.chatsLoaded) renderChat();
      }
      return;
    }
    state.sendQueue.push({ localId: session.id, msg: msg });
    pumpSendQueue();
  }

  function pumpSendQueue() {
    if (state.sendActiveId) return;
    while (state.sendQueue.length) {
      const job = state.sendQueue.shift();
      if (!job || !job.msg) continue;
      const session = findSession(job.localId);
      if (!session || !session.connected || job.msg.status === "failed") {
        if (job.msg.status === "sending") {
          job.msg.status = "failed";
          if (session) persistChatUpdate(session, job.msg);
        }
        continue;
      }
      state.sendActiveId = job.msg.id;
      startQueuedFileSend(session, job.msg);
      return;
    }
  }

  function startQueuedFileSend(session, msg) {
    state.inflightFiles[msg.id] = { localId: session.id, msg: msg };
    tauriInvoke("webrpc_send_file", {
      sessionId: session.rpcSessionId || 0,
      path: msg.filePath,
      fileName: msg.title,
      size: msg.size,
      msgId: msg.id,
    }).catch(function () {
      msg.status = "failed";
      persistChatUpdate(session, msg);
      delete state.inflightFiles[msg.id];
      finishQueuedFileSend(msg.id);
      if (state.selectedId === session.id && session.chatsLoaded) renderChat();
    });
  }

  function finishQueuedFileSend(msgId) {
    if (state.sendActiveId !== msgId) return;
    state.sendActiveId = null;
    pumpSendQueue();
  }

  function onFileChosen() {
    const file = fileInput.files && fileInput.files[0];
    fileInput.value = "";
    if (!file) return;
    if (state.pendingFile && state.pendingFile.previewUrl) {
      URL.revokeObjectURL(state.pendingFile.previewUrl);
    }
    const type = classifyFile(file.name);
    state.pendingFile = {
      name: file.name,
      size: file.size,
      type: type,
      previewUrl: type === "image" ? URL.createObjectURL(file) : "",
    };
    renderPendingFile();
  }

  function renderPendingFile() {
    if (!state.pendingFile) {
      pendingFileEl.classList.remove("is-visible");
      pendingFileName.textContent = "";
      return;
    }
    pendingFileEl.classList.add("is-visible");
    pendingFileName.textContent =
      truncateBytes(state.pendingFile.name, TITLE_MAX_BYTES) +
      " · " +
      formatSize(state.pendingFile.size);
  }

  function clearPendingFile() {
    if (state.pendingFile && state.pendingFile.previewUrl) {
      URL.revokeObjectURL(state.pendingFile.previewUrl);
    }
    state.pendingFile = null;
    renderPendingFile();
  }

  function sendComposer() {
    const session = findSession(state.selectedId);
    if (!session || !session.connected) return;
    const text = composerInput.value.trim();
    const pending = state.pendingFile;
    if (!text && !pending) return;

    if (text) {
      const sent = textMsg("me", text, Date.now(), "sending");
      session.messages.push(sent);
      persistChatAppend(session, sent);
      sendTextMessage(session, sent);
    }
    if (pending) {
      const msg = fileMsg("me", pending.type, pending.name, pending.size, Date.now(), {
        status: "sending",
        transferred: 0,
        elapsedMs: 0,
        speedBps: 0,
        filePath: pending.path || "",
      });
      session.messages.push(msg);
      persistChatAppend(session, msg);
      sendFileMessage(session, msg);
      state.pendingFile = null;
    }
    composerInput.value = "";
    renderWorkspace();
  }

  function startTransfer(sessionId, msg, outbound) {
    const started = Date.now();
    stopTick(msg.id);
    state.ticks[msg.id] = window.setInterval(function () {
      const session = findSession(sessionId);
      if (!session) {
        stopTick(msg.id);
        return;
      }
      const elapsed = Date.now() - started;
      const chunk = Math.max(32 * 1024, Math.floor(msg.size / 18));
      msg.transferred = Math.min(msg.size, msg.transferred + chunk);
      msg.elapsedMs = elapsed;
      msg.speedBps = elapsed ? Math.round((msg.transferred / elapsed) * 1000) : 0;
      if (msg.transferred >= msg.size) {
        msg.status = outbound ? "sent" : "received";
        stopTick(msg.id);
        persistChatUpdate(session, msg);
      }
      if (state.selectedId === sessionId && session.chatsLoaded) {
        renderChat();
        renderAccount();
      } else {
        renderAccount();
      }
    }, 180);
  }

  function resumeActiveTransfers() {
    state.sessions.forEach(function (session) {
      session.messages.forEach(function (msg) {
        if (msg.status === "sending" || msg.status === "receiving") {
          startTransfer(session.id, msg, msg.from === "me");
        }
      });
    });
  }

  function stopTick(id) {
    if (state.ticks[id]) {
      window.clearInterval(state.ticks[id]);
      delete state.ticks[id];
    }
  }

  function stopAllTicks() {
    Object.keys(state.ticks).forEach(stopTick);
  }

  function revokePreviews(sessions) {
    sessions.forEach(function (session) {
      session.messages.forEach(function (msg) {
        if (msg.previewUrl && msg.previewUrl.indexOf("blob:") === 0) {
          URL.revokeObjectURL(msg.previewUrl);
        }
      });
    });
  }

  function findSession(id) {
    return state.sessions.find(function (s) {
      return s.id === id;
    });
  }

  function convertFileSrc(path) {
    if (!path) return "";
    try {
      if (window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.convertFileSrc) {
        return window.__TAURI__.core.convertFileSrc(path);
      }
      if (window.__TAURI_INTERNALS__ && typeof window.__TAURI_INTERNALS__.convertFileSrc === "function") {
        return window.__TAURI_INTERNALS__.convertFileSrc(path);
      }
    } catch (_) {}
    return "";
  }

  function isTransferComplete(msg) {
    return !!msg && (msg.status === "sent" || msg.status === "received");
  }

  function imagePreviewSrc(msg) {
    if (!msg || msg.type !== "image" || !msg.filePath) return "";
    if (msg.from === "peer" && msg.status !== "received") return "";
    return convertFileSrc(msg.filePath);
  }

  function canOpenLocalFile(msg) {
    if (!msg || !msg.filePath) return false;
    if (msg.type === "image") return !!imagePreviewSrc(msg);
    if (msg.type === "video") return isTransferComplete(msg);
    return false;
  }

  function findChatMessage(msgId) {
    const session = findSession(state.selectedId);
    if (!session || !msgId) return null;
    return (session.messages || []).find(function (item) {
      return item.id === msgId;
    }) || null;
  }

  function refreshFolderButtons() {
    if (!chatLog) return;
    const session = findSession(state.selectedId);
    if (!session || !session.chatsLoaded) return;
    chatLog.querySelectorAll(".msg-bubble.is-file").forEach(function (bubble) {
      const msg = (session.messages || []).find(function (item) {
        return item.id === bubble.getAttribute("data-msg-id");
      });
      if (!msg || msg.type === "text" || !isTransferComplete(msg) || !msg.filePath) {
        bubble.classList.remove("has-folder");
        return;
      }
      tauriInvoke("file_stat", { path: msg.filePath })
        .then(function () {
          if (!bubble.isConnected) return;
          bubble.classList.add("has-folder");
        })
        .catch(function () {
          if (!bubble.isConnected) return;
          bubble.classList.remove("has-folder");
        });
    });
  }

  function revealMessageFile(msgId) {
    const msg = findChatMessage(msgId);
    if (!msg || !isTransferComplete(msg) || !msg.filePath) return;
    tauriInvoke("file_stat", { path: msg.filePath })
      .then(function () {
        return tauriInvoke("reveal_in_dir", { path: msg.filePath });
      })
      .catch(function () {
        refreshFolderButtons();
      });
  }

  function openMediaOverlay(msgId) {
    const msg = findChatMessage(msgId);
    if (!msg || !canOpenLocalFile(msg)) return;
    const src = convertFileSrc(msg.filePath);
    if (!src || !mediaOverlay) return;
    closeMediaOverlay();
    if (msg.type === "image") {
      mediaOverlayImage.hidden = false;
      mediaOverlayVideo.hidden = true;
      mediaOverlayImage.alt = msg.title || "";
      mediaOverlayImage.src = src;
    } else if (msg.type === "video") {
      mediaOverlayImage.hidden = true;
      mediaOverlayVideo.hidden = false;
      mediaOverlayVideo.src = src;
      mediaOverlayVideo.play().catch(function () {});
    } else {
      return;
    }
    mediaOverlay.hidden = false;
  }

  function closeMediaOverlay() {
    if (!mediaOverlay) return;
    mediaOverlay.hidden = true;
    if (mediaOverlayVideo) {
      mediaOverlayVideo.pause();
      mediaOverlayVideo.removeAttribute("src");
      mediaOverlayVideo.load();
    }
    if (mediaOverlayImage) {
      mediaOverlayImage.removeAttribute("src");
      mediaOverlayImage.alt = "";
    }
  }

  function classifyFile(name) {
    const ext = (name.split(".").pop() || "").toLowerCase();
    if (IMAGE_EXT.indexOf(ext) >= 0) return "image";
    if (VIDEO_EXT.indexOf(ext) >= 0) return "video";
    if (OFFICE_EXT.indexOf(ext) >= 0) return "office";
    if (AUDIO_EXT.indexOf(ext) >= 0) return "audio";
    return "other";
  }

  function truncateBytes(str, maxBytes) {
    const encoder = new TextEncoder();
    let out = "";
    for (const ch of str) {
      const next = out + ch;
      if (encoder.encode(next).length > maxBytes) {
        return out + "…";
      }
      out = next;
    }
    return out;
  }

  function formatSize(bytes) {
    return formatUnit(bytes, false);
  }

  function formatSpeed(bps) {
    return formatUnit(bps, true) + "/s";
  }

  function formatUnit(n, isRate) {
    const value = Number(n) || 0;
    if (value < 1024) return Math.round(value) + " B";
    if (value < 1024 * 1024) return trimNum(value / 1024) + " KB";
    if (value < 1024 * 1024 * 1024) return trimNum(value / (1024 * 1024)) + " MB";
    return trimNum(value / (1024 * 1024 * 1024)) + " GB" + (isRate ? "" : "");
  }

  function trimNum(n) {
    return n >= 10 ? n.toFixed(1) : n.toFixed(2);
  }

  function formatDuration(ms) {
    const total = Math.max(0, Math.round((Number(ms) || 0) / 1000));
    const m = Math.floor(total / 60);
    const s = total % 60;
    return String(m).padStart(2, "0") + ":" + String(s).padStart(2, "0");
  }

  function formatClock(ts) {
    const d = new Date(ts);
    return pad(d.getHours()) + ":" + pad(d.getMinutes()) + ":" + pad(d.getSeconds());
  }

  function formatDateTime(ts) {
    const d = new Date(ts);
    return (
      d.getFullYear() +
      "-" +
      pad(d.getMonth() + 1) +
      "-" +
      pad(d.getDate()) +
      " " +
      pad(d.getHours()) +
      ":" +
      pad(d.getMinutes()) +
      ":" +
      pad(d.getSeconds())
    );
  }

  function pad(n) {
    return String(n).padStart(2, "0");
  }

  function escapeHtml(value) {
    return String(value)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;");
  }
})();
