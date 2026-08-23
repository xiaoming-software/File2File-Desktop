(function () {
  const TITLE_MAX_BYTES = 30;
  const COPY_MIN_LINES = 5;

  const IMAGE_EXT = ["png", "jpg", "jpeg", "gif", "webp", "bmp", "svg"];
  const NAS_PREVIEW_MAX = 20 * 1024 * 1024;
  const NAS_VIDEO_MAX = 1024 * 1024 * 1024;
  const NAS_VIDEO_HINT_MS = 60 * 1000;
  const NAS_VIDEO_MIN_BYTES = 64 * 1024;
  const NAS_AUDIO_MAX = 100 * 1024 * 1024;
  const NAS_TEXT_MAX = 100 * 1024 * 1024;
  const NAS_OFFICE_MAX = 100 * 1024 * 1024;
  const VIDEO_EXT = ["mp4", "mov", "avi", "mkv", "webm", "m4v"];
  const OFFICE_EXT = ["doc", "docx", "xls", "xlsx", "ppt", "pptx", "pdf", "csv", "odt", "ods", "odp"];
  const AUDIO_EXT = ["mp3", "wav", "flac", "aac", "m4a", "ogg", "wma"];
  const TEXT_EXT = ["txt", "log", "md", "ini", "conf", "cfg", "properties", "env"];
  const CODE_EXT = [
    "js", "ts", "tsx", "jsx", "json", "html", "htm", "css", "scss", "less",
    "rs", "go", "py", "java", "c", "cc", "cpp", "cxx", "h", "hpp",
    "sh", "bash", "zsh", "vue", "xml", "yml", "yaml", "toml", "sql",
    "kt", "kts", "swift", "rb", "php", "lua", "m", "mm",
  ];
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
  const driveListEl = document.getElementById("drive-list");
  const panelUnselected = document.getElementById("panel-unselected");
  const panelConnect = document.getElementById("panel-connect");
  const panelDrive = document.getElementById("panel-drive");
  const chatMain = document.getElementById("chat-main");
  const connectMark = document.getElementById("connect-mark");
  const connectTitle = document.getElementById("connect-title");
  const connectDesc = document.getElementById("connect-desc");
  const nasTitle = document.getElementById("nas-title");
  const nasPath = document.getElementById("nas-path");
  const nasPathUp = document.getElementById("nas-path-up");
  const nasPathRefresh = document.getElementById("nas-path-refresh");
  const nasSearchInput = document.getElementById("nas-search");
  const nasSearchDrop = document.getElementById("nas-search-drop");
  const nasDiskTotal = document.getElementById("nas-disk-total");
  const nasDiskFree = document.getElementById("nas-disk-free");
  const nasDiskUsed = document.getElementById("nas-disk-used");
  const nasFileNum = document.getElementById("nas-file-num");
  const nasMeterBar = document.getElementById("nas-meter-bar");
  const nasStatsHint = document.getElementById("nas-stats-hint");
  const nasFiles = document.getElementById("nas-files");
  const nasFileBody = document.getElementById("nas-file-body");
  const nasViewListBtn = document.getElementById("nas-view-list");
  const nasViewGridBtn = document.getElementById("nas-view-grid");
  const nasUploadBtn = document.getElementById("nas-upload");
  const nasTaskToggle = document.getElementById("nas-task-toggle");
  const nasTaskBadge = document.getElementById("nas-task-badge");
  const nasTasksEl = document.getElementById("nas-tasks");
  const nasTaskList = document.getElementById("nas-task-list");
  const nasTaskClear = document.getElementById("nas-task-clear");
  const nasTaskClose = document.getElementById("nas-task-close");
  const nasUploadInput = document.getElementById("nas-upload-input");
  const nasCtxDownload = document.getElementById("nas-ctx-download");
  const nasCtxMove = document.getElementById("nas-ctx-move");
  const nasCtxRename = document.getElementById("nas-ctx-rename");
  const nasCtxZip = document.getElementById("nas-ctx-zip");
  const nasCtxDelete = document.getElementById("nas-ctx-delete");
  const nasCtx = document.getElementById("nas-ctx");
  const modalNasCreate = document.getElementById("modal-nas-create");
  const nasCreateTitle = document.getElementById("nas-create-title");
  const nasCreateHint = document.getElementById("nas-create-hint");
  const nasCreateLabel = document.getElementById("nas-create-label");
  const nasCreateInput = document.getElementById("nas-create-input");
  const nasCreateError = document.getElementById("nas-create-error");
  const nasCreateOk = document.getElementById("nas-create-ok");
  const nasCreateCtl = { kind: "file", seq: 0, busy: false };
  const nasMoveCtl = { seq: 0, busy: false, source: "drag", blocked: [] };
  const nasDeleteCtl = { seq: 0, busy: false, items: [], path: "/" };
  const nasRenameCtl = {
    active: false,
    busy: false,
    seq: 0,
    name: "",
    isDir: false,
    path: "/",
    draft: "",
    selStart: 0,
    selEnd: 0,
  };
  const nasSearchCtl = {
    seq: 0,
    timer: 0,
    keyword: "",
    results: [],
    truncated: false,
    loading: false,
    highlight: -1,
    pendingSelect: null,
  };
  const nasZipCtl = {
    seq: 0,
    busy: false,
    async: false,
    name: "",
    path: "/",
    zipName: "",
    progress: 0,
    selectName: "",
  };
  const modalNasZip = document.getElementById("modal-nas-zip");
  const nasZipTitle = document.getElementById("nas-zip-title");
  const nasZipDesc = document.getElementById("nas-zip-desc");
  const nasZipProgress = document.getElementById("nas-zip-progress");
  const nasZipSpinner = document.getElementById("nas-zip-spinner");
  const nasZipBar = document.getElementById("nas-zip-bar");
  const nasZipPct = document.getElementById("nas-zip-pct");
  const nasZipError = document.getElementById("nas-zip-error");
  const nasZipAsync = document.getElementById("nas-zip-async");
  const nasZipOk = document.getElementById("nas-zip-ok");
  const nasTaskCtl = { ctxName: "", ctxFile: false, ctxDir: false };
  const nasSelCtl = { names: [], timer: 0, pendingDeselect: "" };
  const modalNasMove = document.getElementById("modal-nas-move");
  const nasMoveHint = document.getElementById("nas-move-hint");
  const nasMovePath = document.getElementById("nas-move-path");
  const nasMoveUp = document.getElementById("nas-move-up");
  const nasMoveGo = document.getElementById("nas-move-go");
  const nasMoveList = document.getElementById("nas-move-list");
  const nasMoveError = document.getElementById("nas-move-error");
  const nasMoveOk = document.getElementById("nas-move-ok");
  const nasMovePicker = {
    open: false,
    name: "",
    fromPath: "/",
    isDir: false,
    path: "/",
    folders: [],
    loading: false,
    listError: "",
    seq: 0,
    items: [],
  };
  const nasDragCtl = {
    press: null,
    dragging: false,
    handled: false,
    internal: null,
    lastHit: null,
  };
  const nasMarquee = document.getElementById("nas-marquee");
  const nasDragGhost = document.getElementById("nas-drag-ghost");
  const nasDragGhostIcon = document.getElementById("nas-drag-ghost-icon");
  const nasDragGhostName = document.getElementById("nas-drag-ghost-name");
  const nasDragGhostTip = document.getElementById("nas-drag-ghost-tip");
  const modalNewDrive = document.getElementById("modal-new-drive");
  const newDriveToken = document.getElementById("new-drive-token");
  const newDrivePass = document.getElementById("new-drive-pass");
  const newDriveError = document.getElementById("new-drive-error");
  const chatPane = document.getElementById("chat-pane");
  const chatDropMask = document.getElementById("chat-drop-mask");
  const nasDropMask = document.getElementById("nas-drop-mask");
  const nasDropHint = document.getElementById("nas-drop-hint");
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
  const confirmFailList = document.getElementById("confirm-fail-list");
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
  const nasPreview = document.getElementById("nas-preview");
  const nasPreviewImage = document.getElementById("nas-preview-image");
  const nasPreviewStatus = document.getElementById("nas-preview-status");
  const nasPreviewLoading = document.getElementById("nas-preview-loading");
  const nasPreviewSpinner = document.getElementById("nas-preview-spinner");
  const nasPreviewVideo = document.getElementById("nas-preview-video");
  const nasPreviewAudio = document.getElementById("nas-preview-audio");
  const nasOfficeEl = document.getElementById("nas-office");
  const nasOfficeTitle = document.getElementById("nas-office-title");
  const nasOfficePath = document.getElementById("nas-office-path");
  const nasOfficeHint = document.getElementById("nas-office-hint");
  const nasOfficeLoading = document.getElementById("nas-office-loading");
  const nasOfficeStatus = document.getElementById("nas-office-status");
  const nasOfficeGuide = document.getElementById("nas-office-guide");
  const nasOfficeSync = document.getElementById("nas-office-sync");
  const nasOfficeExit = document.getElementById("nas-office-exit");
  const nasOfficeAsk = document.getElementById("nas-office-ask");
  const nasOfficeAskCancel = document.getElementById("nas-office-ask-cancel");
  const nasOfficeAskDiscard = document.getElementById("nas-office-ask-discard");
  const nasOfficeAskSync = document.getElementById("nas-office-ask-sync");
  const nasOfficeDock = document.getElementById("nas-office-dock");
  const nasOfficeDockList = document.getElementById("nas-office-dock-list");
  const NAS_OFFICE_STABLE_MS = 2500;
  let nasOfficeDocs = [];
  let nasOfficeNextId = 1;
  let nasOfficeFocusId = 0;
  let nasOfficeWatchTimer = 0;
  let nasOfficeAskForId = 0;
  const nasEditorEl = document.getElementById("nas-editor");
  const nasEditorTitle = document.getElementById("nas-editor-title");
  const nasEditorPath = document.getElementById("nas-editor-path");
  const nasEditorHint = document.getElementById("nas-editor-hint");
  const nasEditorBody = document.getElementById("nas-editor-body");
  const nasEditorSave = document.getElementById("nas-editor-save");
  const nasEditorExit = document.getElementById("nas-editor-exit");
  const nasEditorAsk = document.getElementById("nas-editor-ask");
  const nasEditorAskCancel = document.getElementById("nas-editor-ask-cancel");
  const nasEditorAskDiscard = document.getElementById("nas-editor-ask-discard");
  const nasEditorAskSave = document.getElementById("nas-editor-ask-save");
  const nasEditorCtl = {
    sessionId: 0,
    name: "",
    path: "",
    localPath: "",
    original: "",
    saveText: "",
    saving: false,
    closeAfterSave: false,
    saveTimer: 0,
  };
  const nasStreamCtl = {
    timer: 0,
    hintTimer: 0,
    seq: 0,
    path: "",
    lastSize: 0,
    playing: false,
    complete: false,
    attached: false,
  };

  const state = {
    user: null,
    sessions: [],
    drives: [],
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
    desktopFullscreen: false,
    desktopPhase: "idle",
    desktopSwallowUp: false,
    desktopLastClickAt: 0,
    desktopPendingMove: null,
    desktopLastPt: null,
    desktopHeld: { left: false, right: false, middle: false },
    desktopFrameW: 0,
    desktopFrameH: 0,
    desktopPendingJpeg: null,
    desktopPaintScheduled: false,
    desktopMoveTimer: null,
    voiceMuted: false,
    voiceTimer: null,
    voiceStartedAt: 0,
  };

  const SIDEBAR_SPLIT_KEY = "file2file.sidebarSplit";
  const SIDEBAR_SPLIT_MIN = 96;
  let sidebarSplitRatio = 0.5;
  let sidebarSplitDragging = false;
  const NAS_VIEW_KEY = "file2file.nasView";
  let nasViewMode = "list";

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
        window.__TAURI__.event.listen("webrpc-nas-info", function (event) {
          onNasInfo(event.payload || {});
        });
        window.__TAURI__.event.listen("webrpc-nas-path", function (event) {
          onNasPath(event.payload || {});
        });
        window.__TAURI__.event.listen("webrpc-nas-file", function (event) {
          onNasFile(event.payload || {});
        });
        window.__TAURI__.event.listen("webrpc-nas-put", function (event) {
          onNasPut(event.payload || {});
        });
        window.__TAURI__.event.listen("webrpc-nas-create", function (event) {
          onNasCreate(event.payload || {});
        });
        window.__TAURI__.event.listen("webrpc-nas-move", function (event) {
          onNasMove(event.payload || {});
        });
        window.__TAURI__.event.listen("webrpc-nas-delete", function (event) {
          onNasDelete(event.payload || {});
        });
        window.__TAURI__.event.listen("webrpc-nas-rename", function (event) {
          onNasRename(event.payload || {});
        });
        window.__TAURI__.event.listen("webrpc-nas-search", function (event) {
          onNasSearch(event.payload || {});
        });
        window.__TAURI__.event.listen("webrpc-nas-zip", function (event) {
          onNasZip(event.payload || {});
        });
        window.__TAURI__.event.listen("webrpc-nas-task", function (event) {
          onNasTask(event.payload || {});
        });
        window.__TAURI__.event.listen("screenshot-done", function (event) {
          onScreenshotDone(event.payload);
        });
        window.__TAURI__.event.listen("webrpc-voice-state", function (event) {
          onVoiceState(event.payload || {});
        });
        window.__TAURI__.event.listen("webrpc-desktop-state", function (event) {
          onDesktopState(event.payload || {});
        });
        window.__TAURI__.event.listen("webrpc-desktop-frame", function (event) {
          onDesktopFrame(event.payload || {});
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
    if (/Windows/i.test(navigator.userAgent)) {
      document.documentElement.classList.add("is-windows");
    }

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
      flushOfficeDocsThen(function () {
        win.close();
      });
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

    document.addEventListener(
      "contextmenu",
      function (event) {
        event.preventDefault();
      },
      true
    );
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
        if (nasDragCtl.dragging) return;
        if (canDropNasNow() || canDropSendNow()) showDropMask(payload.position);
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
      if (nasDragCtl.dragging) return;
      if (canDropNasNow() || canDropSendNow()) showDropMask({ x: event.clientX, y: event.clientY });
    });
    document.addEventListener("dragleave", function (event) {
      if (event.relatedTarget && document.documentElement.contains(event.relatedTarget)) return;
      if (nasDragCtl.dragging) return;
      hideDropMask();
    });
    document.addEventListener("drop", function (event) {
      event.preventDefault();
      if (nasDragCtl.dragging) return;
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
    bindNasInternalDrag();
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

  function canDropNasNow() {
    const drive = selectedDrive();
    return !!(drive && drive.connected && panelDrive && panelDrive.classList.contains("is-visible"));
  }

  function showDropMask(position) {
    if (nasDragCtl.dragging) return;
    if (canDropNasNow()) {
      if (nasDropMask) nasDropMask.hidden = false;
      if (chatDropMask) chatDropMask.hidden = true;
      updateNasDropTarget(position, false);
      return;
    }
    if (canDropSendNow()) {
      if (chatDropMask) chatDropMask.hidden = false;
      if (nasDropMask) nasDropMask.hidden = true;
      clearNasDropTarget();
      return;
    }
    hideDropMask();
  }

  function hideDropMask() {
    if (chatDropMask) chatDropMask.hidden = true;
    if (nasDropMask) nasDropMask.hidden = true;
    clearNasDropTarget();
  }

  function isDragoutDummyPath(path) {
    return /[/\\]file2file_data[/\\]dragout[/\\]/.test(String(path || ""));
  }

  function handleDroppedPaths(paths, position) {
    const list = normalizeDropPaths(paths).filter(function (item) {
      return !isDragoutDummyPath(item);
    });
    if (!list.length) return;

    if (canDropNasNow()) {
      handleNasDroppedPaths(list, position);
      return;
    }

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

  function handleNasDroppedPaths(paths, position) {
    const drive = selectedDrive();
    if (!drive || !drive.connected || !drive.rpcSessionId) {
      openInfoPrompt("无法上传文件", "请先连接网盘，再拖入文件上传。");
      return;
    }
    const hit = hitNasDrop(position);
    if (!hit) return;
    tauriInvoke("inspect_paths", { paths: paths })
      .then(function (infos) {
        const items = Array.isArray(infos) ? infos : [];
        if (!items.length) return;
        const hasFolder = items.some(function (item) {
          return item && item.kind === "directory";
        });
        if (hasFolder) {
          openInfoPrompt("无法上传文件夹", "不支持直接上传文件夹。请打开文件夹后，选中其中的文件再拖入。");
          return;
        }
        const files = items.filter(function (item) {
          return item && item.kind === "file" && !isDragoutDummyPath(item.path);
        });
        if (!files.length) {
          openInfoPrompt("无法上传文件", "拖入的内容不是可上传的文件，请重新选择后再试。");
          return;
        }
        return tauriInvoke("nas_task_upload_many", {
          ownerToken: ownerToken(),
          peerToken: drive.peerToken,
          sessionId: drive.rpcSessionId,
          items: files.map(function (file) {
            return { localPath: file.path, nasPath: hit.dir };
          }),
        }).then(function (tasks) {
          drive.nasTaskPanelOpen = true;
          applyNasTasks(drive, tasks, false);
        });
      })
      .catch(function () {
        openInfoPrompt("无法上传文件", "读取拖入的文件失败，请重新选择后再试。");
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

  function showDesktopError(err) {
    var msg = "";
    if (typeof err === "string") msg = err;
    else if (err && err.message) msg = String(err.message);
    else msg = String(err || "");
    if (!msg || msg === "[object Object]" || msg === "undefined") {
      msg = "远程控制失败，请稍后重试。";
    }
    openInfoPrompt("无法远程控制", msg);
  }

  function startDesktopControl() {
    const session = findSession(state.selectedId);
    if (!session) {
      openInfoPrompt("无法远程控制", "请先选择并连接一个会话。");
      return;
    }
    if (isDrive(session)) {
      openInfoPrompt("无法远程控制", "远程控制只支持电脑会话。");
      return;
    }
    if (!session.connected || !session.rpcSessionId) {
      openInfoPrompt("无法远程控制", "请先连接当前会话，再发起远程控制。");
      return;
    }
    tauriInvoke("desktop_invite", { sessionId: session.rpcSessionId }).catch(showDesktopError);
  }

  function formatDesktopStats(payload) {
    const w = Number(payload && payload.width) || 0;
    const h = Number(payload && payload.height) || 0;
    const fps = Number(payload && payload.fps) || 0;
    const kbps = Number(payload && payload.kbps) || 0;
    const kBps = Math.round(kbps / 8);
    const size = w && h ? w + "×" + h : "—";
    return size + " · " + fps + " fps · " + kBps + " KB/s";
  }

  function setDesktopFullscreen(on) {
    state.desktopFullscreen = !!on;
    const root = document.getElementById("desktop-root");
    const btn = document.getElementById("desktop-full");
    if (root) root.classList.toggle("is-full", state.desktopFullscreen);
    if (btn) btn.textContent = state.desktopFullscreen ? "退出全屏" : "全屏";
    requestAnimationFrame(layoutDesktopCanvas);
  }

  function onDesktopState(payload) {
    const phase = String((payload && payload.phase) || "idle");
    const root = document.getElementById("desktop-root");
    const title = document.getElementById("desktop-title");
    const sub = document.getElementById("desktop-sub");
    const stats = document.getElementById("desktop-stats");
    const stage = document.getElementById("desktop-stage");
    const frame = document.getElementById("desktop-frame");
    const placeholder = document.getElementById("desktop-placeholder");
    const btn = document.getElementById("btn-desktop");
    const cancel = document.getElementById("desktop-cancel");
    const reject = document.getElementById("desktop-reject");
    const accept = document.getElementById("desktop-accept");
    const hangup = document.getElementById("desktop-stop");
    const full = document.getElementById("desktop-full");
    if (!root) return;
    state.desktopPhase = phase;
    if (phase !== "controlling") {
      state.desktopHeld = { left: false, right: false, middle: false };
      state.desktopSwallowUp = false;
    }
    if (stage) stage.classList.toggle("is-control", phase === "controlling");
    hideDesktopCursor();
    if (phase === "idle") {
      root.hidden = true;
      root.classList.remove("is-ring", "is-live");
      setDesktopFullscreen(false);
      if (frame) {
        frame.hidden = true;
        if (frame.getContext) {
          const ctx = frame.getContext("2d");
          if (ctx) ctx.clearRect(0, 0, frame.width, frame.height);
        }
      }
      state.desktopFrameW = 0;
      state.desktopFrameH = 0;
      state.desktopPendingJpeg = null;
      if (stage) stage.hidden = true;
      if (placeholder) placeholder.hidden = false;
      if (btn) btn.textContent = "远程控制";
      if (payload && payload.error) showDesktopError(payload.error);
      return;
    }
    const sessionId = Number(payload.sessionId) || 0;
    const name = voicePeerLabel(sessionId, payload.peerToken);
    root.hidden = false;
    root.classList.toggle("is-ring", phase === "outgoing" || phase === "incoming");
    root.classList.toggle("is-live", phase === "controlling" || phase === "controlled");
    if (btn) {
      btn.textContent = phase === "idle" ? "远程控制" : "结束控制";
    }
    if (cancel) cancel.hidden = phase !== "outgoing";
    if (reject) reject.hidden = phase !== "incoming";
    if (accept) accept.hidden = phase !== "incoming";
    if (hangup) hangup.hidden = phase !== "controlling" && phase !== "controlled";
    if (full) full.hidden = phase !== "controlling";
    if (phase !== "controlling") setDesktopFullscreen(false);
    if (stats) {
      stats.hidden = phase !== "controlling" && phase !== "controlled";
      stats.textContent = formatDesktopStats(payload);
    }
    if (phase === "outgoing") {
      if (title) title.textContent = "正在请求远程控制";
      if (sub) sub.textContent = name;
      if (stage) stage.hidden = true;
    } else if (phase === "incoming") {
      if (title) title.textContent = "请求远程控制你的电脑";
      if (sub) sub.textContent = name + " 想查看你的桌面";
      if (stage) stage.hidden = true;
    } else if (phase === "controlled") {
      if (title) title.textContent = "正在被远程控制";
      if (sub) sub.textContent = name + " 正在控制你的电脑，你也可以继续使用";
      if (stage) stage.hidden = true;
    } else {
      if (title) title.textContent = "对方桌面";
      if (sub) sub.textContent = name;
      if (stage) stage.hidden = false;
      if (placeholder) placeholder.hidden = !(state.desktopFrameW && state.desktopFrameH);
    }
  }

  function onDesktopFrame(payload) {
    const jpeg = payload && payload.jpeg;
    if (!jpeg) return;
    if (payload.width) state.desktopFrameW = payload.width;
    if (payload.height) state.desktopFrameH = payload.height;
    state.desktopPendingJpeg = jpeg;
    if (state.desktopPaintScheduled) return;
    state.desktopPaintScheduled = true;
    requestAnimationFrame(paintDesktopFrame);
  }

  function jpegBase64ToBlob(b64) {
    const bin = atob(b64);
    const arr = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) arr[i] = bin.charCodeAt(i);
    return new Blob([arr], { type: "image/jpeg" });
  }

  function paintDesktopFrame() {
    state.desktopPaintScheduled = false;
    const jpeg = state.desktopPendingJpeg;
    state.desktopPendingJpeg = null;
    const frame = document.getElementById("desktop-frame");
    const placeholder = document.getElementById("desktop-placeholder");
    const root = document.getElementById("desktop-root");
    const stage = document.getElementById("desktop-stage");
    if (!jpeg || !frame) return;
    if (root) root.hidden = false;
    if (stage) stage.hidden = false;
    frame.hidden = false;
    const blob = jpegBase64ToBlob(jpeg);
    const draw = function (bmp) {
      if (frame.width !== bmp.width) frame.width = bmp.width;
      if (frame.height !== bmp.height) frame.height = bmp.height;
      const ctx = frame.getContext("2d");
      if (ctx) ctx.drawImage(bmp, 0, 0);
      if (bmp.close) bmp.close();
      if (placeholder) placeholder.hidden = true;
      layoutDesktopCanvas();
      if (state.desktopPendingJpeg) {
        state.desktopPaintScheduled = true;
        requestAnimationFrame(paintDesktopFrame);
      }
    };
    if (typeof createImageBitmap === "function") {
      createImageBitmap(blob).then(draw).catch(function () {});
    } else {
      const img = new Image();
      img.onload = function () {
        draw(img);
        URL.revokeObjectURL(img.src);
      };
      img.src = URL.createObjectURL(blob);
    }
  }

  function hideDesktopCursor() {
    const cursor = document.getElementById("desktop-cursor");
    if (cursor) cursor.hidden = true;
  }

  function layoutDesktopCanvas() {
    const frame = document.getElementById("desktop-frame");
    const stage = document.getElementById("desktop-stage");
    const fw = state.desktopFrameW || (frame && frame.width) || 0;
    const fh = state.desktopFrameH || (frame && frame.height) || 0;
    if (!frame || !stage || !fw || !fh) return;
    const rect = stage.getBoundingClientRect();
    if (!rect.width || !rect.height) return;
    const scale = Math.min(rect.width / fw, rect.height / fh);
    const dw = Math.max(1, fw * scale);
    const dh = Math.max(1, fh * scale);
    frame.style.width = dw + "px";
    frame.style.height = dh + "px";
    frame.style.marginLeft = (rect.width - dw) / 2 + "px";
    frame.style.marginTop = (rect.height - dh) / 2 + "px";
  }

  function desktopNormFromEvent(event) {
    const frame = document.getElementById("desktop-frame");
    if (!frame) return null;
    const rect = frame.getBoundingClientRect();
    if (!rect.width || !rect.height) return null;
    const x = (event.clientX - rect.left) / rect.width;
    const y = (event.clientY - rect.top) / rect.height;
    if (x < 0 || x > 1 || y < 0 || y > 1) return null;
    return { x: Math.min(1, Math.max(0, x)), y: Math.min(1, Math.max(0, y)) };
  }

  function desktopButtonName(button) {
    if (button === 2) return "right";
    if (button === 1) return "middle";
    return "left";
  }

  function sendDesktopInput(payload) {
    if (state.desktopPhase !== "controlling") return;
    if (payload && payload.x != null && payload.y != null) {
      state.desktopLastPt = { x: payload.x, y: payload.y };
    }
    tauriInvoke("desktop_input", payload).catch(function () {});
  }

  function desktopPointFromEvent(event) {
    return desktopNormFromEvent(event) || state.desktopPendingMove || state.desktopLastPt;
  }

  function flushDesktopButtons(event) {
    const pt = desktopPointFromEvent(event);
    if (!pt) {
      state.desktopHeld = { left: false, right: false, middle: false };
      return;
    }
    ["left", "right", "middle"].forEach(function (btn) {
      if (!state.desktopHeld[btn]) return;
      state.desktopHeld[btn] = false;
      sendDesktopInput({ op: "up", btn: btn, x: pt.x, y: pt.y, dx: 0, dy: 0 });
    });
  }

  function placeDesktopCursor(event) {
    const stage = document.getElementById("desktop-stage");
    const cursor = document.getElementById("desktop-cursor");
    if (!stage || !cursor) return;
    const rect = stage.getBoundingClientRect();
    cursor.hidden = false;
    cursor.style.left = event.clientX - rect.left + "px";
    cursor.style.top = event.clientY - rect.top + "px";
  }

  function onDesktopPointerMove(event) {
    if (state.desktopPhase !== "controlling") return;
    placeDesktopCursor(event);
    const pt = desktopNormFromEvent(event);
    if (!pt) return;
    state.desktopPendingMove = pt;
    if (state.desktopMoveTimer) return;
    state.desktopMoveTimer = setTimeout(function () {
      state.desktopMoveTimer = null;
      const next = state.desktopPendingMove;
      state.desktopPendingMove = null;
      if (!next) return;
      sendDesktopInput({ op: "move", btn: "left", x: next.x, y: next.y, dx: 0, dy: 0 });
    }, 16);
  }

  function onDesktopPointerDown(event) {
    if (state.desktopPhase !== "controlling") return;
    const pt = desktopPointFromEvent(event);
    if (!pt) return;
    event.preventDefault();
    if (event.currentTarget && event.currentTarget.setPointerCapture) {
      try {
        event.currentTarget.setPointerCapture(event.pointerId);
      } catch (_) {}
    }
    const btn = desktopButtonName(event.button);
    const now = Date.now();
    if (btn === "left" && now - (state.desktopLastClickAt || 0) < 350) {
      state.desktopLastClickAt = 0;
      state.desktopSwallowUp = true;
      state.desktopHeld[btn] = false;
      sendDesktopInput({ op: "dblclick", btn: btn, x: pt.x, y: pt.y, dx: 0, dy: 0 });
      return;
    }
    state.desktopLastClickAt = btn === "left" ? now : 0;
    state.desktopSwallowUp = false;
    state.desktopHeld[btn] = true;
    sendDesktopInput({ op: "down", btn: btn, x: pt.x, y: pt.y, dx: 0, dy: 0 });
  }

  function onDesktopPointerUp(event) {
    if (state.desktopPhase !== "controlling") return;
    const btn = desktopButtonName(event.button);
    if (state.desktopSwallowUp) {
      state.desktopSwallowUp = false;
      state.desktopHeld[btn] = false;
      return;
    }
    const pt = desktopPointFromEvent(event);
    state.desktopHeld[btn] = false;
    if (!pt) return;
    sendDesktopInput({
      op: "up",
      btn: btn,
      x: pt.x,
      y: pt.y,
      dx: 0,
      dy: 0,
    });
  }

  function onDesktopPointerLeave(event) {
    flushDesktopButtons(event);
    hideDesktopCursor();
  }

  function onDesktopWheel(event) {
    if (state.desktopPhase !== "controlling") return;
    const pt = desktopNormFromEvent(event);
    if (!pt) return;
    event.preventDefault();
    const dy = event.deltaY === 0 ? 0 : event.deltaY > 0 ? -1 : 1;
    const dx = event.deltaX === 0 ? 0 : event.deltaX > 0 ? -1 : 1;
    sendDesktopInput({ op: "wheel", btn: "middle", x: pt.x, y: pt.y, dx: dx, dy: dy });
  }

  function bindDesktopPointer() {
    const stage = document.getElementById("desktop-stage");
    if (!stage) return;
    stage.addEventListener("pointermove", onDesktopPointerMove);
    stage.addEventListener("pointerdown", onDesktopPointerDown);
    stage.addEventListener("pointerup", onDesktopPointerUp);
    stage.addEventListener("pointercancel", onDesktopPointerUp);
    stage.addEventListener("pointerleave", onDesktopPointerLeave);
    stage.addEventListener("wheel", onDesktopWheel, { passive: false });
    stage.addEventListener("contextmenu", function (event) {
      event.preventDefault();
    });
    stage.addEventListener("dblclick", function (event) {
      event.preventDefault();
    });
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
    if (el) el.addEventListener("input", hideLoginError);
  });
  bindPasswordToggle("toggle-password", passwordInput);
  bindPasswordToggle("toggle-passphrase", passphraseInput);
  bindPasswordToggle("toggle-connect-pass", connectPeerPass);
  form.addEventListener("submit", function (event) {
    event.preventDefault();
    submitLogin();
  });
  try {
    bindWorkspaceEvents();
  } catch (err) {
    console.error("workspace bind failed", err);
  }
  initTauriShell();
  bindSessionSizeListener();

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
    if (!btn || !input) return;
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
    state.drives = [];
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
    window.requestAnimationFrame(function () {
      applySidebarSplit();
      window.requestAnimationFrame(applySidebarSplit);
    });

    renderAccount();
    renderWorkspace();
    Promise.all([loadSavedSessions(), loadSavedDrives()]).then(function () {
      if (!state.selectedId) {
        const first = state.sessions[0] || state.drives[0];
        state.selectedId = first ? first.id : null;
      }
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
    state.drives = [];
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
    closeNasPreview();
    forceCloseNasEditor();
    forceCloseNasOffice();
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

  function hydrateSession(item, kind) {
    return {
      id: uid(),
      kind: kind || "chat",
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

  function hydrateDrive(item) {
    const drive = hydrateSession(item, "drive");
    drive.nasInfo = null;
    drive.nasWatching = false;
    drive.nasPath = "/";
    drive.nasFiles = [];
    drive.nasLoading = false;
    drive.nasListError = "";
    drive.nasListSeq = 0;
    drive.nasTasks = [];
    drive.nasTasksLoaded = false;
    drive.nasTaskPanelOpen = false;
    return drive;
  }

  function isDrive(item) {
    return !!(item && item.kind === "drive");
  }

  function loadSavedSessions() {
    if (!ownerToken()) {
      state.sessions = [];
      return Promise.resolve();
    }
    return tauriInvoke("saved_sessions_list", { ownerToken: ownerToken() })
      .then(function (items) {
        state.sessions = (Array.isArray(items) ? items : []).map(function (item) {
          return hydrateSession(item, "chat");
        });
        state.connectFillId = null;
        state.sessionsReady = true;
        flushPendingHellos();
        flushPendingTexts();
      })
      .catch(function () {
        state.sessions = [];
        state.sessionsReady = true;
        flushPendingHellos();
        flushPendingTexts();
      });
  }

  function loadSavedDrives() {
    if (!ownerToken()) {
      state.drives = [];
      return Promise.resolve();
    }
    return tauriInvoke("saved_drives_list", { ownerToken: ownerToken() })
      .then(function (items) {
        state.drives = (Array.isArray(items) ? items : []).map(hydrateDrive);
      })
      .catch(function () {
        state.drives = [];
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

  function persistDriveCreate(drive) {
    return tauriInvoke("saved_drives_create", {
      ownerToken: ownerToken(),
      peerToken: drive.peerToken,
      peerPass: drive.peerPass || "",
      remark: drive.remark || "",
    }).catch(function (err) {
      if (err && err.message === "webrpc-unavailable") return;
      return Promise.reject(err);
    });
  }

  function persistDriveUpdate(drive) {
    if (!drive) return Promise.resolve();
    return tauriInvoke("saved_drives_update", {
      ownerToken: ownerToken(),
      peerToken: drive.peerToken,
      peerPass: drive.peerPass || "",
      remark: drive.remark || "",
    }).catch(function (err) {
      if (err && err.message === "webrpc-unavailable") return;
    });
  }

  function persistDriveDelete(peerToken) {
    tauriInvoke("nas_task_wipe", {
      ownerToken: ownerToken(),
      peerToken: peerToken,
    }).catch(function () {});
    return tauriInvoke("saved_drives_delete", {
      ownerToken: ownerToken(),
      peerToken: peerToken,
    }).catch(function (err) {
      if (err && err.message === "webrpc-unavailable") return;
    });
  }

  function persistItemUpdate(item) {
    if (isDrive(item)) return persistDriveUpdate(item);
    return persistSessionUpdate(item);
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
      const prev = findItem(state.selectedId);
      if (prev && !isDrive(prev)) unloadSessionChats(prev);
      clearNasSelection();
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

    let session = findByRpcSessionId(sessionId);
    if (!session) {
      session = state.drives.find(function (item) {
        return item.peerToken === token && (item.connecting || item.connected);
      });
    }
    if (!session) {
      session = state.sessions.find(function (item) {
        return item.peerToken === token;
      });
    }
    const existed = !!session;
    if (!session) {
      session = {
        id: uid(),
        kind: "chat",
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
    if (isDrive(session)) startNasWatch(session);

    if (existed) {
      persistItemUpdate(session);
    } else {
      persistSessionCreate(session).catch(function (err) {
        if (invokeErrorText(err).indexOf("session-exists") >= 0) {
          persistSessionUpdate(session);
        }
      });
    }

    renderWorkspace();
    if (state.selectedId === session.id) {
      renderChat();
    }
  }

  function onSessionDead(sessionId) {
    const sid = Number(sessionId) || 0;
    if (!sid || !state.user) return;
    const session = findByRpcSessionId(sid);
    if (!session) return;
    session.connected = false;
    session.connecting = false;
    session.nasWatching = false;
    session.rpcSessionId = 0;
    session.connectError = isDrive(session)
      ? "网盘已断开连接，请重新连接。"
      : "对端已断开连接，请重新连接。";
    if (isDrive(session)) {
      closeNasPreview();
      if (nasEditorCtl.sessionId === sid) forceCloseNasEditor();
      closeOfficeDocsForSession(sid);
    }
    failSessionTransfers(session);
    renderWorkspace();
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

  function loadSidebarSplit() {
    try {
      const n = Number(window.localStorage.getItem(SIDEBAR_SPLIT_KEY));
      if (n >= 0.18 && n <= 0.82) sidebarSplitRatio = n;
    } catch (_) {}
  }

  function saveSidebarSplit() {
    try {
      window.localStorage.setItem(SIDEBAR_SPLIT_KEY, String(sidebarSplitRatio));
    } catch (_) {}
  }

  function applySidebarSplit() {
    const pane = document.getElementById("session-pane");
    const chat = document.getElementById("session-section-chat");
    const split = document.getElementById("session-split");
    if (!pane || !chat || pane.clientHeight < SIDEBAR_SPLIT_MIN * 2) return;
    const splitH = split ? split.offsetHeight : 8;
    const max = pane.clientHeight - SIDEBAR_SPLIT_MIN - splitH;
    const top = Math.round(
      Math.min(max, Math.max(SIDEBAR_SPLIT_MIN, sidebarSplitRatio * pane.clientHeight))
    );
    chat.style.flex = "0 0 " + top + "px";
  }

  function bindSidebarSplit() {
    const pane = document.getElementById("session-pane");
    const split = document.getElementById("session-split");
    if (!pane || !split) return;
    loadSidebarSplit();
    applySidebarSplit();

    split.addEventListener("pointerdown", function (event) {
      if (event.button != null && event.button !== 0) return;
      event.preventDefault();
      sidebarSplitDragging = true;
      split.classList.add("is-dragging");
      pane.classList.add("is-resizing");
      if (split.setPointerCapture) split.setPointerCapture(event.pointerId);
    });
    split.addEventListener("pointermove", function (event) {
      if (!sidebarSplitDragging) return;
      const rect = pane.getBoundingClientRect();
      const splitH = split.offsetHeight || 8;
      const max = rect.height - SIDEBAR_SPLIT_MIN - splitH;
      const top = Math.min(max, Math.max(SIDEBAR_SPLIT_MIN, event.clientY - rect.top));
      sidebarSplitRatio = top / rect.height;
      applySidebarSplit();
    });
    function endSplitDrag() {
      if (!sidebarSplitDragging) return;
      sidebarSplitDragging = false;
      split.classList.remove("is-dragging");
      pane.classList.remove("is-resizing");
      saveSidebarSplit();
    }
    split.addEventListener("pointerup", endSplitDrag);
    split.addEventListener("pointercancel", endSplitDrag);
    split.addEventListener("dblclick", function () {
      sidebarSplitRatio = 0.5;
      applySidebarSplit();
      saveSidebarSplit();
    });
  }

  function loadNasView() {
    try {
      const v = window.localStorage.getItem(NAS_VIEW_KEY);
      if (v === "grid" || v === "list") nasViewMode = v;
    } catch (_) {}
  }

  function saveNasView() {
    try {
      window.localStorage.setItem(NAS_VIEW_KEY, nasViewMode);
    } catch (_) {}
  }

  function bindNasExplorer() {
    loadNasView();
    applyNasView();
    bindNasSearch();
    document.addEventListener(
      "pointerdown",
      function (event) {
        if (!nasRenameCtl.active || nasRenameCtl.busy) return;
        if (event.target.closest(".nas-rename-input")) return;
        if (event.target.closest("#nas-ctx")) return;
        if (event.target.closest("#modal-root")) return;
        submitNasRename();
      },
      true
    );
    function onToggle(event) {
      const btn = event.target.closest("[data-nas-view]");
      if (!btn) return;
      nasViewMode = btn.getAttribute("data-nas-view") === "grid" ? "grid" : "list";
      saveNasView();
      applyNasView();
    }
    if (nasViewListBtn) nasViewListBtn.addEventListener("click", onToggle);
    if (nasViewGridBtn) nasViewGridBtn.addEventListener("click", onToggle);
    if (nasPath) {
      nasPath.addEventListener("click", function (event) {
        const btn = event.target.closest("[data-nas-path]");
        if (!btn) return;
        const session = selectedDrive();
        if (!session || session.nasLoading) return;
        const next = normalizeNasPath(btn.getAttribute("data-nas-path") || "/");
        if (next === normalizeNasPath(session.nasPath) && !session.nasListError) return;
        requestNasPath(session, next);
      });
    }
    if (nasPathUp) {
      nasPathUp.addEventListener("click", function () {
        const session = selectedDrive();
        if (!session || session.nasLoading || nasPathUp.disabled) return;
        const next = parentNasPath(session.nasPath);
        if (next === normalizeNasPath(session.nasPath || "/")) return;
        requestNasPath(session, next);
      });
    }
    if (nasPathRefresh) {
      nasPathRefresh.addEventListener("click", function () {
        const session = selectedDrive();
        if (!session || session.nasLoading) return;
        requestNasPath(session, session.nasPath || "/");
      });
    }
    if (nasFileBody) {
      nasFileBody.addEventListener("dblclick", function (event) {
        if (event.target.closest(".nas-rename-input")) return;
        const el = event.target.closest("[data-nas-name]");
        if (!el) return;
        const session = selectedDrive();
        if (!session || session.nasLoading) return;
        const name = el.getAttribute("data-nas-name") || "";
        if (nasRenameCtl.active && nasRenameCtl.name === name) return;
        cancelNasDeselect();
        if (el.getAttribute("data-nas-dir") === "1") {
          requestNasPath(session, joinNasPath(session.nasPath, name));
          return;
        }
        previewNasFile(session, name);
      });
    }
    if (nasFiles) {
      nasFiles.addEventListener("contextmenu", function (event) {
        event.preventDefault();
        if (event.target.closest(".nas-rename-input")) return;
        const session = selectedDrive();
        if (!session || !session.connected || session.nasLoading) return;
        const row = event.target.closest("[data-nas-name]");
        nasTaskCtl.ctxName = row ? row.getAttribute("data-nas-name") || "" : "";
        nasTaskCtl.ctxDir = !!(row && row.getAttribute("data-nas-dir") === "1");
        nasTaskCtl.ctxFile = !!(row && !nasTaskCtl.ctxDir);
        if (nasTaskCtl.ctxName && !isNasSelected(nasTaskCtl.ctxName)) {
          setNasSelection([nasTaskCtl.ctxName]);
        }
        showNasCtx(event.clientX, event.clientY);
      });
    }
    if (nasCtx) {
      nasCtx.addEventListener("click", function (event) {
        if (event.target.closest("[data-nas-download]")) {
          const name = nasTaskCtl.ctxName;
          hideNasCtx();
          startNasDownload(name);
          return;
        }
        if (event.target.closest("[data-nas-move]")) {
          hideNasCtx();
          openNasMoveModalFromCtx();
          return;
        }
        if (event.target.closest("[data-nas-rename]")) {
          hideNasCtx();
          startNasRenameFromCtx();
          return;
        }
        if (event.target.closest("[data-nas-zip]")) {
          hideNasCtx();
          startNasZipFromCtx();
          return;
        }
        if (event.target.closest("[data-nas-delete]")) {
          hideNasCtx();
          openNasDeleteConfirm();
          return;
        }
        const btn = event.target.closest("[data-nas-create]");
        if (!btn) return;
        const kind = btn.getAttribute("data-nas-create") === "folder" ? "folder" : "file";
        hideNasCtx();
        openNasCreateModal(kind);
      });
    }
    if (nasUploadBtn) nasUploadBtn.addEventListener("click", startNasUpload);
    if (nasTaskToggle) nasTaskToggle.addEventListener("click", toggleNasTaskPanel);
    if (nasTaskClear) nasTaskClear.addEventListener("click", confirmClearNasTasks);
    if (nasTaskClose) nasTaskClose.addEventListener("click", closeNasTaskPanel);
    if (nasTaskList) {
      nasTaskList.addEventListener("click", function (event) {
        const retry = event.target.closest("[data-nas-task-retry]");
        const folder = event.target.closest("[data-nas-task-folder]");
        const del = event.target.closest("[data-nas-task-delete]");
        const row = event.target.closest("[data-task-id]");
        if (!row) return;
        const id = row.getAttribute("data-task-id") || "";
        if (retry) retryNasTask(id);
        else if (folder) openNasTaskFolder(id);
        else if (del) deleteNasTask(id);
      });
    }
    if (nasCreateOk) nasCreateOk.addEventListener("click", submitNasCreate);
    if (nasCreateInput) {
      nasCreateInput.addEventListener("keydown", function (event) {
        if (event.key === "Enter") {
          event.preventDefault();
          submitNasCreate();
        }
      });
      nasCreateInput.addEventListener("input", function () {
        hideNasCreateError();
      });
    }
    bindNasMovePicker();
    if (nasZipAsync) {
      nasZipAsync.addEventListener("click", function () {
        if (!nasZipCtl.busy) return;
        nasZipCtl.async = true;
        closeModal();
      });
    }
    if (nasZipOk) {
      nasZipOk.addEventListener("click", function () {
        nasZipCtl.busy = false;
        nasZipCtl.async = false;
        closeModal();
      });
    }
    function onNasStreamError() {
      if (!nasPreview || nasPreview.hidden) return;
      const session = selectedDrive();
      if (!session || !isNasStreamKind(session.nasPreviewKind)) return;
      nasStreamCtl.attached = false;
      if (nasStreamCtl.complete && !nasStreamCtl.playing) {
        showNasPreview(session, session.nasPreviewName, "", nasStreamFailText(session.nasPreviewKind), false);
      }
    }
    if (nasPreviewVideo) nasPreviewVideo.addEventListener("error", onNasStreamError);
    if (nasPreviewAudio) nasPreviewAudio.addEventListener("error", onNasStreamError);
    bindNasEditor();
    bindNasOffice();
  }

  function hideNasCtx() {
    if (nasCtx) nasCtx.hidden = true;
  }

  function showNasCtx(x, y) {
    if (!nasCtx) return;
    if (nasCtxDownload) nasCtxDownload.hidden = !nasTaskCtl.ctxFile;
    if (nasCtxMove) nasCtxMove.hidden = !nasTaskCtl.ctxName;
    if (nasCtxRename) nasCtxRename.hidden = !nasTaskCtl.ctxName;
    if (nasCtxZip) nasCtxZip.hidden = !nasTaskCtl.ctxDir;
    if (nasCtxDelete) nasCtxDelete.hidden = !nasTaskCtl.ctxName;
    nasCtx.hidden = false;
    nasCtx.classList.remove("is-left");
    const pad = 8;
    const menuW = nasCtx.offsetWidth || 132;
    const menuH = nasCtx.offsetHeight || 40;
    let left = x;
    let top = y;
    if (left + menuW + 128 > window.innerWidth - pad) {
      left = Math.max(pad, window.innerWidth - menuW - pad);
      nasCtx.classList.add("is-left");
    }
    if (top + menuH > window.innerHeight - pad) {
      top = Math.max(pad, window.innerHeight - menuH - pad);
    }
    nasCtx.style.left = left + "px";
    nasCtx.style.top = top + "px";
  }

  function findDriveByRpc(sessionId) {
    const sid = Number(sessionId) || 0;
    if (!sid) return null;
    return state.drives.find(function (item) {
      return item.rpcSessionId === sid;
    }) || null;
  }

  function findDriveByPeer(peerToken) {
    const peer = String(peerToken || "");
    if (!peer) return null;
    return state.drives.find(function (item) {
      return item.peerToken === peer;
    }) || null;
  }

  function driveHasRunningTask(drive) {
    return ((drive && drive.nasTasks) || []).some(function (item) {
      return item && item.status === "running";
    });
  }

  function nasTaskActiveCount(drive) {
    return ((drive && drive.nasTasks) || []).filter(function (item) {
      return item && (item.status === "running" || item.status === "queued");
    }).length;
  }

  function applyNasTasks(drive, tasks, fromEvent) {
    if (!drive) return;
    const prev = drive.nasTasks || [];
    const next = Array.isArray(tasks) ? tasks : [];
    if (fromEvent) {
      const finished = next.find(function (task) {
        if (!task || task.kind !== "upload" || task.status !== "done") return false;
        return prev.some(function (item) {
          return item && item.id === task.id && item.status === "running";
        });
      });
      if (
        finished &&
        drive.connected &&
        normalizeNasPath(drive.nasPath || "/") === normalizeNasPath(finished.nasPath || "/")
      ) {
        requestNasPath(drive, drive.nasPath);
      }
    }
    drive.nasTasks = next;
    drive.nasTasksLoaded = true;
    if (state.selectedId === drive.id) renderNasTaskPanel(drive);
    pumpOfficeDownload();
    pumpOfficeSync();
  }

  function onNasTask(payload) {
    const drive = findDriveByPeer(payload && payload.peerToken);
    if (!drive) return;
    applyNasTasks(drive, payload.tasks, true);
  }

  function ensureNasTasks(drive) {
    if (!drive || !drive.connected || !drive.rpcSessionId || drive.nasTasksLoaded) return;
    bindNasTaskSession(drive);
  }

  function bindNasTaskSession(drive) {
    if (!drive || !drive.rpcSessionId) return;
    tauriInvoke("nas_task_bind", {
      ownerToken: ownerToken(),
      peerToken: drive.peerToken,
      sessionId: drive.rpcSessionId,
    })
      .then(function (tasks) {
        applyNasTasks(drive, tasks, false);
      })
      .catch(function () {
        tauriInvoke("nas_task_list", {
          ownerToken: ownerToken(),
          peerToken: drive.peerToken,
        })
          .then(function (tasks) {
            applyNasTasks(drive, tasks, false);
          })
          .catch(function () {});
      });
  }

  function toggleNasTaskPanel() {
    const drive = selectedDrive();
    if (!drive) return;
    drive.nasTaskPanelOpen = !drive.nasTaskPanelOpen;
    ensureNasTasks(drive);
    renderNasTaskPanel(drive);
  }

  function closeNasTaskPanel() {
    const drive = selectedDrive();
    if (!drive) return;
    drive.nasTaskPanelOpen = false;
    renderNasTaskPanel(drive);
  }

  function renderNasTaskPanel(drive) {
    if (!drive) return;
    const tasks = Array.isArray(drive.nasTasks) ? drive.nasTasks : [];
    const active = nasTaskActiveCount(drive);
    if (nasTaskToggle) nasTaskToggle.classList.toggle("is-active", !!drive.nasTaskPanelOpen);
    if (nasTaskBadge) {
      nasTaskBadge.hidden = active <= 0;
      nasTaskBadge.textContent = String(active);
    }
    if (nasTasksEl) nasTasksEl.hidden = !drive.nasTaskPanelOpen;
    if (nasTaskClear) {
      const clearable = tasks.some(function (item) {
        return item && item.status !== "queued" && item.status !== "running";
      });
      nasTaskClear.disabled = !clearable;
    }
    if (!nasTaskList) return;
    if (!tasks.length) {
      nasTaskList.innerHTML = '<div class="nas-task-empty">暂无任务</div>';
      return;
    }
    nasTaskList.innerHTML = tasks
      .slice()
      .reverse()
      .map(renderNasTaskItem)
      .join("");
  }

  function nasTaskStatusText(status) {
    if (status === "queued") return "排队中";
    if (status === "running") return "进行中";
    if (status === "done") return "已完成";
    if (status === "interrupted") return "已中断";
    return "失败";
  }

  function renderNasTaskItem(task) {
    const down = task.kind === "download";
    const status = String(task.status || "");
    const pct =
      task.size > 0 ? Math.min(100, Math.round((Number(task.transferred) || 0) * 100 / task.size)) : status === "done" ? 100 : 0;
    const statusCls = status === "running" || status === "queued" ? " is-run" : status === "done" ? "" : " is-fail";
    const retry =
      status === "failed" || status === "interrupted"
        ? '<button type="button" data-nas-task-retry>重试</button>'
        : "";
    const folder = '<button type="button" data-nas-task-folder>文件夹</button>';
    const running = status === "running";
    const del =
      '<button type="button" class="nas-task-del" data-nas-task-delete' +
      (running ? " disabled title=\"进行中的任务不能删除\"" : ' title="删除此任务"') +
      ' aria-label="删除此任务">×</button>';
    const elapsed = formatDuration(task.elapsedMs || 0);
    const speed = formatSpeed(task.speedBps || 0);
    const stats =
      status === "queued"
        ? ""
        : " · 耗时 " + elapsed + " · 平均 " + speed;
    const err = status !== "done" && task.error ? " · " + String(task.error) : "";
    return (
      '<div class="nas-task' +
      (down ? " is-down" : "") +
      '" data-task-id="' +
      escapeHtml(task.id) +
      '"><div class="nas-task-top"><span class="nas-task-kind' +
      (down ? " is-down" : "") +
      '">' +
      (down ? "下载" : "上传") +
      '</span><span class="nas-task-name" title="' +
      escapeHtml(task.name || "") +
      '">' +
      escapeHtml(task.name || "") +
      '</span><span class="nas-task-status' +
      statusCls +
      '">' +
      nasTaskStatusText(status) +
      "</span>" +
      del +
      "</div><div class=\"nas-task-meta\">" +
      formatSize(task.transferred || 0) +
      " / " +
      formatSize(task.size || 0) +
      stats +
      err +
      '</div><div class="nas-task-bar"><span style="width:' +
      pct +
      '%"></span></div><div class="nas-task-actions">' +
      retry +
      folder +
      "</div></div>"
    );
  }

  function pickLocalFile() {
    try {
      if (window.__TAURI__ && window.__TAURI__.dialog && window.__TAURI__.dialog.open) {
        return window.__TAURI__.dialog.open({ multiple: false, directory: false }).then(function (path) {
          if (Array.isArray(path)) path = path[0];
          return path || "";
        });
      }
    } catch (_) {}
    return new Promise(function (resolve) {
      if (!nasUploadInput) {
        resolve("");
        return;
      }
      nasUploadInput.value = "";
      nasUploadInput.onchange = function () {
        const file = nasUploadInput.files && nasUploadInput.files[0];
        resolve((file && file.path) || "");
      };
      nasUploadInput.click();
    });
  }

  function pickLocalDir() {
    try {
      if (window.__TAURI__ && window.__TAURI__.dialog && window.__TAURI__.dialog.open) {
        return window.__TAURI__.dialog.open({ directory: true, multiple: false }).then(function (path) {
          if (Array.isArray(path)) path = path[0];
          return path || "";
        });
      }
    } catch (_) {}
    return Promise.resolve("");
  }

  function startNasUpload() {
    const drive = selectedDrive();
    if (!drive || !drive.connected || !drive.rpcSessionId) return;
    pickLocalFile().then(function (path) {
      if (!path) return;
      return tauriInvoke("nas_task_upload", {
        ownerToken: ownerToken(),
        peerToken: drive.peerToken,
        sessionId: drive.rpcSessionId,
        localPath: path,
        nasPath: drive.nasPath || "/",
      }).then(function (tasks) {
        drive.nasTaskPanelOpen = true;
        applyNasTasks(drive, tasks, false);
      });
    }).catch(function () {});
  }

  function startNasDownload(name) {
    const drive = selectedDrive();
    if (!drive || !drive.connected || !drive.rpcSessionId || !name) return;
    const file = findNasEntry(drive, name);
    if (!file || file.isDir) return;
    pickLocalDir().then(function (dir) {
      if (!dir) return;
      return tauriInvoke("nas_task_download", {
        ownerToken: ownerToken(),
        peerToken: drive.peerToken,
        sessionId: drive.rpcSessionId,
        fileName: name,
        nasPath: drive.nasPath || "/",
        size: file.size == null || !isFinite(file.size) ? null : Number(file.size),
        destDir: dir,
      }).then(function (tasks) {
        drive.nasTaskPanelOpen = true;
        applyNasTasks(drive, tasks, false);
      });
    }).catch(function () {});
  }

  function pointCandidates(position, event) {
    const out = [];
    if (event && Number.isFinite(event.clientX) && Number.isFinite(event.clientY)) {
      out.push({ x: event.clientX, y: event.clientY });
    }
    if (!position) return out;
    const x = Number(position.x);
    const y = Number(position.y);
    if (!Number.isFinite(x) || !Number.isFinite(y)) return out;
    const dpr = window.devicePixelRatio || 1;
    const title = document.querySelector(".titlebar");
    const th = title ? title.getBoundingClientRect().height : 0;
    out.push({ x: x, y: y });
    out.push({ x: x / dpr, y: y / dpr });
    out.push({ x: x, y: y - th });
    out.push({ x: x / dpr, y: y / dpr - th });
    return out;
  }

  function hitNasDrop(position, event) {
    const drive = selectedDrive();
    if (!drive) return null;
    const points = pointCandidates(position, event);
    for (let i = 0; i < points.length; i += 1) {
      const el = document.elementFromPoint(points[i].x, points[i].y);
      if (!el) continue;
      const folder = el.closest("[data-nas-name]");
      if (
        folder &&
        folder.getAttribute("data-nas-dir") === "1" &&
        folder.closest("#nas-files") &&
        !folder.classList.contains("is-dragging")
      ) {
        const name = folder.getAttribute("data-nas-name") || "";
        return { dir: joinNasPath(drive.nasPath, name), name: name, el: folder };
      }
      const crumb = el.closest("[data-nas-path]");
      if (crumb) {
        const dir = normalizeNasPath(crumb.getAttribute("data-nas-path") || "/");
        const label = crumb.textContent || dir;
        return { dir: dir, name: dir === "/" ? "/" : label, el: crumb };
      }
      if (el.closest("#nas-files")) {
        return { dir: normalizeNasPath(drive.nasPath || "/"), name: "", el: null };
      }
    }
    return null;
  }

  function clearNasDropTarget() {
    document.querySelectorAll(".is-drop-target").forEach(function (el) {
      el.classList.remove("is-drop-target");
    });
  }

  function updateNasDropTarget(position, isMove) {
    clearNasDropTarget();
    const hit = hitNasDrop(position);
    nasDragCtl.lastHit = hit && hit.name ? hit : null;
    const bundle = nasDragCtl.internal;
    const items = bundle && Array.isArray(bundle.items) ? bundle.items : [];
    const valid =
      !!(
        hit &&
        hit.name &&
        hit.el &&
        items.length &&
        items.every(function (item) {
          return canMoveNasTo(item.name, !!item.isDir, bundle.fromPath, hit.dir);
        })
      );
    if (valid) {
      document.querySelectorAll("[data-nas-name][data-nas-dir='1']").forEach(function (el) {
        if (el.getAttribute("data-nas-name") === hit.name) el.classList.add("is-drop-target");
      });
      if (hit.el) hit.el.classList.add("is-drop-target");
    }
    updateNasDragGhost(position, valid ? hit : null);
    if (nasDropMask && !nasDropMask.hidden && nasDropHint && !isMove) {
      nasDropHint.textContent =
        hit && hit.name ? "松开即可上传到 " + hit.name : "松开即可上传到当前目录";
    }
  }

  function cancelNasDeselect() {
    if (nasSelCtl.timer) {
      window.clearTimeout(nasSelCtl.timer);
      nasSelCtl.timer = 0;
    }
    nasSelCtl.pendingDeselect = "";
  }

  function isNasSelected(name) {
    return !!name && nasSelCtl.names.indexOf(name) >= 0;
  }

  function applyNasSelection() {
    if (!nasFileBody) return;
    nasFileBody.querySelectorAll("[data-nas-name]").forEach(function (el) {
      el.classList.toggle("is-selected", isNasSelected(el.getAttribute("data-nas-name") || ""));
    });
  }

  function pruneNasSelection(session) {
    const exist = {};
    ((session && session.nasFiles) || []).forEach(function (item) {
      if (item && item.name) exist[item.name] = true;
    });
    nasSelCtl.names = nasSelCtl.names.filter(function (name) {
      return exist[name];
    });
  }

  function setNasSelection(names) {
    const seen = {};
    nasSelCtl.names = [];
    (names || []).forEach(function (name) {
      if (!name || seen[name]) return;
      seen[name] = true;
      nasSelCtl.names.push(name);
    });
    applyNasSelection();
  }

  function addNasSelection(name) {
    if (!name || isNasSelected(name)) return;
    nasSelCtl.names.push(name);
    applyNasSelection();
  }

  function removeNasSelection(name) {
    nasSelCtl.names = nasSelCtl.names.filter(function (item) {
      return item !== name;
    });
    applyNasSelection();
  }

  function clearNasSelection() {
    cancelNasDeselect();
    if (!nasSelCtl.names.length) {
      applyNasSelection();
      return;
    }
    nasSelCtl.names = [];
    applyNasSelection();
  }

  function scheduleNasDeselect(name) {
    cancelNasDeselect();
    nasSelCtl.pendingDeselect = name;
    nasSelCtl.timer = window.setTimeout(function () {
      if (nasSelCtl.pendingDeselect !== name) return;
      nasSelCtl.pendingDeselect = "";
      nasSelCtl.timer = 0;
      removeNasSelection(name);
    }, 280);
  }

  function selectedNasItems(session) {
    const drive = session || selectedDrive();
    const map = {};
    ((drive && drive.nasFiles) || []).forEach(function (item) {
      if (item && item.name) map[item.name] = item;
    });
    return nasSelCtl.names
      .map(function (name) {
        const file = map[name];
        return { name: name, isDir: !!(file && file.isDir) };
      })
      .filter(function (item) {
        return item.name;
      });
  }

  function visibleNasItemEls() {
    if (!nasFileBody || !nasFiles) return [];
    const grid = nasFiles.getAttribute("data-view") === "grid";
    return Array.prototype.slice.call(
      nasFileBody.querySelectorAll(grid ? ".nas-tile[data-nas-name]" : ".nas-row[data-nas-name]")
    );
  }

  function rectsIntersect(a, b) {
    return a.left < b.right && a.right > b.left && a.top < b.bottom && a.bottom > b.top;
  }

  function namesInMarquee(box) {
    if (!nasFileBody || !box) return [];
    const pane = nasFileBody.getBoundingClientRect();
    const clip = {
      left: Math.max(box.left, pane.left),
      top: Math.max(box.top, pane.top),
      right: Math.min(box.right, pane.right),
      bottom: Math.min(box.bottom, pane.bottom),
    };
    if (clip.left >= clip.right || clip.top >= clip.bottom) return [];
    return visibleNasItemEls()
      .filter(function (el) {
        return rectsIntersect(clip, el.getBoundingClientRect());
      })
      .map(function (el) {
        return el.getAttribute("data-nas-name") || "";
      })
      .filter(Boolean);
  }

  function hideNasMarquee() {
    if (!nasMarquee) return;
    nasMarquee.hidden = true;
    nasMarquee.style.cssText = "";
    document.body.classList.remove("is-nas-selecting");
  }

  function showNasMarquee(x1, y1, x2, y2) {
    if (!nasMarquee) return;
    const left = Math.min(x1, x2);
    const top = Math.min(y1, y2);
    const width = Math.abs(x2 - x1);
    const height = Math.abs(y2 - y1);
    nasMarquee.hidden = false;
    nasMarquee.style.left = left + "px";
    nasMarquee.style.top = top + "px";
    nasMarquee.style.width = width + "px";
    nasMarquee.style.height = height + "px";
    document.body.classList.add("is-nas-selecting");
    setNasSelection(
      namesInMarquee({
        left: left,
        top: top,
        right: left + width,
        bottom: top + height,
      })
    );
  }

  function nasMoveFailReason(code) {
    const key = String(code || "");
    if (key === "exists") return "目标位置已存在同名文件或文件夹";
    if (key === "missing") return "源文件不存在，可能已被删除或移动";
    if (key === "invalid") return "不能移动到该位置";
    if (key.indexOf("不能") >= 0 || key.indexOf("已在") >= 0) return key;
    return "移动失败，请稍后重试";
  }

  function setConfirmFailList(fails, reasonFn) {
    if (!confirmFailList) return;
    if (!fails || !fails.length) {
      confirmFailList.hidden = true;
      confirmFailList.innerHTML = "";
      return;
    }
    const format = typeof reasonFn === "function" ? reasonFn : nasMoveFailReason;
    confirmFailList.hidden = false;
    confirmFailList.innerHTML = fails
      .map(function (item) {
        const name = item && item.name ? String(item.name) : "(未命名)";
        const reason = format(item && (item.error || item.reason));
        return "<li><strong>" + escapeHtml(name) + "</strong>：" + escapeHtml(reason) + "</li>";
      })
      .join("");
  }

  function openNasMoveFailPrompt(fails) {
    const list = (fails || []).filter(function (item) {
      return item && (item.name || item.error);
    });
    if (!list.length) return;
    state.confirmKind = "info";
    if (list.length === 1) {
      confirmTitle.textContent = "无法移动";
      confirmDesc.textContent =
        "「" + (list[0].name || "") + "」" + nasMoveFailReason(list[0].error);
      setConfirmFailList([]);
    } else {
      confirmTitle.textContent = "部分项目未能移动";
      confirmDesc.textContent = "以下项目没有移动成功：";
      setConfirmFailList(list);
    }
    confirmOkBtn.textContent = "知道了";
    confirmOkBtn.className = "btn-primary";
    if (confirmCancelBtn) confirmCancelBtn.hidden = true;
    openModal("confirm");
  }

  function formatNasMoveFailLines(fails) {
    return (fails || [])
      .map(function (item) {
        return "「" + (item.name || "") + "」：" + nasMoveFailReason(item.error);
      })
      .join("\n");
  }

  function nasMoveBlockReason(name, isDir, fromPath, targetDir) {
    const from = normalizeNasPath(fromPath);
    const to = normalizeNasPath(targetDir);
    const src = joinNasPath(from, name);
    if (to === from) return "已在该目录，无需移动";
    if (to === src) return "不能将文件夹移动到自身";
    if (isDir && (to + "/").indexOf(src + "/") === 0) return "不能将文件夹移动到自身或子目录中";
    return "";
  }

  function canMoveNasTo(name, isDir, fromPath, targetDir) {
    return !nasMoveBlockReason(name, isDir, fromPath, targetDir);
  }

  function partitionNasMove(items, fromPath, targetDir) {
    const valid = [];
    const blocked = [];
    (items || []).forEach(function (item) {
      if (!item || !item.name) return;
      const reason = nasMoveBlockReason(item.name, !!item.isDir, fromPath, targetDir);
      if (reason) blocked.push({ name: item.name, error: reason });
      else valid.push(item);
    });
    return { valid: valid, blocked: blocked };
  }

  function reportNasMoveFail(message, fails) {
    nasMoveCtl.busy = false;
    if (fails && fails.length) {
      if (nasMoveCtl.source === "picker" && nasMovePicker.open) {
        showNasMoveError(formatNasMoveFailLines(fails));
        if (nasMoveOk) nasMoveOk.disabled = false;
        return;
      }
      openNasMoveFailPrompt(fails);
      return;
    }
    if (nasMoveCtl.source === "picker") {
      if (nasMovePicker.open) {
        showNasMoveError(message);
        if (nasMoveOk) nasMoveOk.disabled = false;
      }
      return;
    }
    openInfoPrompt("无法移动", message);
  }

  function moveNasEntries(items, fromPath, targetDir, source, extraFails) {
    const drive = selectedDrive();
    if (!drive || !drive.connected || !drive.rpcSessionId) return;
    const list = (items || []).filter(function (item) {
      return item && item.name;
    });
    const blocked = extraFails || [];
    if (!list.length) {
      if (blocked.length) reportNasMoveFail("", blocked);
      return;
    }
    const from = normalizeNasPath(fromPath);
    const to = normalizeNasPath(targetDir);
    nasMoveCtl.source = source === "picker" ? "picker" : "drag";
    nasMoveCtl.blocked = blocked;
    nasMoveCtl.busy = true;
    nasMoveCtl.seq = (nasMoveCtl.seq || 0) + 1;
    const seq = nasMoveCtl.seq;
    if (nasMoveCtl.source === "picker" && nasMoveOk) nasMoveOk.disabled = true;
    tauriInvoke("nas_move_entry", {
      sessionId: drive.rpcSessionId,
      names: list.map(function (item) {
        return item.name;
      }),
      path: from,
      targetPath: to,
    }).catch(function () {
      if (nasMoveCtl.seq !== seq) return;
      reportNasMoveFail(
        "移动失败，请稍后重试。",
        list.map(function (item) {
          return { name: item.name, error: "failed" };
        }).concat(blocked)
      );
    });
    window.setTimeout(function () {
      if (nasMoveCtl.seq !== seq || !nasMoveCtl.busy) return;
      reportNasMoveFail(
        "移动超时，请稍后重试。",
        list.map(function (item) {
          return { name: item.name, error: "failed" };
        }).concat(blocked)
      );
    }, 12000);
  }

  function finishNasInternalDrag() {
    document.querySelectorAll(".is-dragging").forEach(function (el) {
      el.classList.remove("is-dragging");
    });
    nasDragCtl.press = null;
    nasDragCtl.dragging = false;
    nasDragCtl.internal = null;
    nasDragCtl.lastHit = null;
    hideNasDragGhost();
    hideNasMarquee();
    document.body.classList.remove("is-nas-dragging");
    hideDropMask();
  }

  function showNasDragGhost(bundle, x, y) {
    if (!nasDragGhost || !bundle) return;
    const items = Array.isArray(bundle.items) ? bundle.items : [];
    const first = items[0] || {};
    const srcIcon =
      (first.el && first.el.querySelector(".nas-icon")) ||
      (bundle.el && bundle.el.querySelector(".nas-icon"));
    if (nasDragGhostIcon) {
      nasDragGhostIcon.className =
        "nas-icon " + (srcIcon ? srcIcon.className.replace("nas-icon", "").trim() : "is-file");
    }
    if (nasDragGhostName) {
      nasDragGhostName.textContent =
        items.length > 1 ? (first.name || "") + " 等 " + items.length + " 项" : first.name || bundle.name || "";
    }
    nasDragGhost.hidden = false;
    updateNasDragGhost({ x: x, y: y }, null);
  }

  function updateNasDragGhost(position, hit) {
    if (!nasDragGhost || nasDragGhost.hidden) return;
    const x = position && Number.isFinite(position.x) ? position.x : 0;
    const y = position && Number.isFinite(position.y) ? position.y : 0;
    nasDragGhost.style.left = x + 16 + "px";
    nasDragGhost.style.top = y + 16 + "px";
    const valid = !!(hit && hit.name);
    nasDragGhost.classList.toggle("is-ok", valid);
    nasDragGhost.classList.toggle("is-bad", !valid);
    if (nasDragGhostTip) {
      const bundle = nasDragCtl.internal;
      const count = bundle && bundle.items ? bundle.items.length : 1;
      nasDragGhostTip.textContent = valid
        ? count > 1
          ? "移动 " + count + " 项到「" + hit.name + "」"
          : "移动到「" + hit.name + "」"
        : "拖到文件夹后松开即可移动";
    }
  }

  function hideNasDragGhost() {
    if (!nasDragGhost) return;
    nasDragGhost.hidden = true;
    nasDragGhost.classList.remove("is-ok", "is-bad");
  }

  function commitNasInternalMove(bundle, position, event) {
    if (!bundle || nasDragCtl.handled) return false;
    const items = Array.isArray(bundle.items) ? bundle.items : [];
    if (!items.length) return false;
    const drive = selectedDrive();
    if (!drive) return false;
    const fromPath = bundle.fromPath || drive.nasPath || "/";
    const hit = hitNasDrop(position, event);
    if (!hit || !hit.name) return false;
    const parts = partitionNasMove(items, fromPath, hit.dir);
    if (!parts.valid.length) return false;
    nasDragCtl.handled = true;
    moveNasEntries(parts.valid, fromPath, hit.dir, "drag", parts.blocked);
    return true;
  }

  function startNasItemDrag(press, x, y) {
    cancelNasDeselect();
    let items;
    if (press.wasSelected) {
      items = selectedNasItems();
    } else {
      setNasSelection([press.name]);
      items = [{ name: press.name, isDir: !!press.isDir, el: press.el }];
    }
    if (!items.length && press.name) {
      items = [{ name: press.name, isDir: !!press.isDir, el: press.el }];
    }
    nasDragCtl.internal = {
      items: items,
      fromPath: press.fromPath,
      el: press.el,
    };
    const names = {};
    items.forEach(function (item) {
      if (item && item.name) names[item.name] = true;
    });
    if (nasFileBody) {
      nasFileBody.querySelectorAll("[data-nas-name]").forEach(function (el) {
        if (names[el.getAttribute("data-nas-name") || ""]) el.classList.add("is-dragging");
      });
    }
    document.body.classList.add("is-nas-dragging");
    showNasDragGhost(nasDragCtl.internal, x, y);
  }

  function bindNasInternalDrag() {
    if (!nasFileBody) return;
    nasFileBody.addEventListener("dragstart", function (event) {
      event.preventDefault();
    });
    nasFileBody.addEventListener("pointerdown", function (event) {
      if (event.target.closest(".nas-rename-input")) return;
      if (event.button !== 0) return;
      const drive = selectedDrive();
      if (!drive || !drive.connected || drive.nasLoading) return;
      const el = event.target.closest("[data-nas-name]");
      nasDragCtl.dragging = false;
      nasDragCtl.handled = false;
      nasDragCtl.lastHit = null;
      nasDragCtl.internal = null;
      if (el && nasFileBody.contains(el)) {
        const name = el.getAttribute("data-nas-name") || "";
        nasDragCtl.press = {
          kind: "item",
          pointerId: event.pointerId,
          name: name,
          isDir: el.getAttribute("data-nas-dir") === "1",
          wasSelected: isNasSelected(name),
          fromPath: drive.nasPath || "/",
          x: event.clientX,
          y: event.clientY,
          el: el,
        };
        return;
      }
      nasDragCtl.press = {
        kind: "marquee",
        pointerId: event.pointerId,
        x: event.clientX,
        y: event.clientY,
      };
    });
    window.addEventListener("pointermove", function (event) {
      const press = nasDragCtl.press;
      if (!press || event.pointerId !== press.pointerId) return;
      if (!event.buttons) return;
      const dx = event.clientX - press.x;
      const dy = event.clientY - press.y;
      if (press.kind === "marquee") {
        if (!nasDragCtl.dragging && dx * dx + dy * dy < 64) return;
        nasDragCtl.dragging = true;
        showNasMarquee(press.x, press.y, event.clientX, event.clientY);
        return;
      }
      if (!nasDragCtl.dragging) {
        if (dx * dx + dy * dy < 64) return;
        nasDragCtl.dragging = true;
        startNasItemDrag(press, event.clientX, event.clientY);
      }
      updateNasDropTarget({ x: event.clientX, y: event.clientY }, true);
    });
    function endPointerDrag(event) {
      const press = nasDragCtl.press;
      if (!press) return;
      if (event && event.pointerId != null && event.pointerId !== press.pointerId) return;
      const dragging = nasDragCtl.dragging;
      const bundle = nasDragCtl.internal;
      const kind = press.kind;
      nasDragCtl.press = null;
      nasDragCtl.dragging = false;
      if (kind === "marquee") {
        if (!dragging) clearNasSelection();
        hideNasMarquee();
        return;
      }
      if (dragging && bundle) {
        commitNasInternalMove(bundle, { x: event.clientX, y: event.clientY }, event);
        finishNasInternalDrag();
        return;
      }
      finishNasInternalDrag();
      if (!press.name) return;
      if (press.wasSelected) scheduleNasDeselect(press.name);
      else addNasSelection(press.name);
    }
    window.addEventListener("pointerup", endPointerDrag);
    window.addEventListener("pointercancel", function (event) {
      const press = nasDragCtl.press;
      if (!press) return;
      if (event && event.pointerId != null && event.pointerId !== press.pointerId) return;
      nasDragCtl.press = null;
      nasDragCtl.dragging = false;
      hideNasMarquee();
      finishNasInternalDrag();
    });
  }

  function retryNasTask(taskId) {
    const drive = selectedDrive();
    if (!drive || !drive.connected || !drive.rpcSessionId || !taskId) return;
    tauriInvoke("nas_task_retry", {
      ownerToken: ownerToken(),
      peerToken: drive.peerToken,
      sessionId: drive.rpcSessionId,
      taskId: taskId,
    })
      .then(function (tasks) {
        applyNasTasks(drive, tasks, false);
      })
      .catch(function () {});
  }

  function deleteNasTask(taskId) {
    const drive = selectedDrive();
    if (!drive || !taskId) return;
    const task = (drive.nasTasks || []).find(function (item) {
      return item && item.id === taskId;
    });
    if (!task || task.status === "running") return;
    tauriInvoke("nas_task_delete", {
      ownerToken: ownerToken(),
      peerToken: drive.peerToken,
      taskId: taskId,
    })
      .then(function (tasks) {
        applyNasTasks(drive, tasks, false);
      })
      .catch(function () {});
  }

  function confirmClearNasTasks() {
    const drive = selectedDrive();
    if (!drive) return;
    const clearable = (drive.nasTasks || []).some(function (item) {
      return item && item.status !== "queued" && item.status !== "running";
    });
    if (!clearable) return;
    openConfirm("clear-nas-tasks", drive.id);
  }

  function clearNasTasks() {
    const drive = selectedDrive();
    if (!drive) return;
    tauriInvoke("nas_task_clear", {
      ownerToken: ownerToken(),
      peerToken: drive.peerToken,
    })
      .then(function (tasks) {
        applyNasTasks(drive, tasks, false);
      })
      .catch(function () {});
  }

  function openNasTaskFolder(taskId) {
    const drive = selectedDrive();
    if (!drive || !taskId) return;
    const task = (drive.nasTasks || []).find(function (item) {
      return item && item.id === taskId;
    });
    if (!task) return;
    if (task.kind === "upload") {
      requestNasPath(drive, task.nasPath || "/");
      return;
    }
    const path = task.localPath || task.destDir || "";
    if (!path) return;
    tauriInvoke("reveal_in_dir", { path: path }).catch(function () {});
  }

  function hideNasCreateError() {
    if (!nasCreateError) return;
    nasCreateError.hidden = true;
    nasCreateError.textContent = "";
  }

  function showNasCreateError(text) {
    if (!nasCreateError) return;
    nasCreateError.hidden = false;
    nasCreateError.textContent = text || "创建失败";
  }

  function hideNasMoveError() {
    if (!nasMoveError) return;
    nasMoveError.hidden = true;
    nasMoveError.textContent = "";
  }

  function showNasMoveError(text) {
    if (!nasMoveError) return;
    nasMoveError.hidden = false;
    nasMoveError.textContent = text || "无法移动到该位置";
  }

  function resetNasMovePicker() {
    nasMovePicker.open = false;
    nasMovePicker.loading = false;
    nasMovePicker.listError = "";
    nasMovePicker.items = [];
    nasMovePicker.seq = (nasMovePicker.seq || 0) + 1;
    if (nasMoveOk) nasMoveOk.disabled = false;
    hideNasMoveError();
  }

  function foldersFromNasFiles(list) {
    return (Array.isArray(list) ? list : [])
      .filter(function (item) {
        return item && item.isDir && item.name && String(item.name).charAt(0) !== ".";
      })
      .map(function (item) {
        return { name: String(item.name) };
      });
  }

  function mapNasListEntries(entries) {
    return Array.isArray(entries)
      ? entries
          .filter(function (item) {
            const name = item && item.name ? String(item.name) : "";
            return name && name.charAt(0) !== ".";
          })
          .map(function (item) {
            return {
              name: String(item.name),
              isDir: !!item.isDir,
              size: item.size == null ? null : Number(item.size),
              mtime: item.lastDate ? Number(item.lastDate) : 0,
            };
          })
      : [];
  }

  function renderNasMovePicker() {
    if (!nasMoveList) return;
    if (nasMovePath && document.activeElement !== nasMovePath) {
      nasMovePath.value = nasMovePicker.path || "/";
    }
    if (nasMoveUp) nasMoveUp.disabled = normalizeNasPath(nasMovePicker.path || "/") === "/";
    if (nasMovePicker.loading) {
      nasMoveList.innerHTML = '<div class="nas-move-empty">正在加载目录…</div>';
      return;
    }
    if (nasMovePicker.listError) {
      nasMoveList.innerHTML =
        '<div class="nas-move-empty">' + escapeHtml(nasMovePicker.listError) + "</div>";
      return;
    }
    const folders = nasMovePicker.folders || [];
    if (!folders.length) {
      nasMoveList.innerHTML =
        '<div class="nas-move-empty">当前目录没有文件夹，可以直接确定以移动到这里。</div>';
      return;
    }
    nasMoveList.innerHTML = folders
      .map(function (item) {
        const name = item && item.name ? String(item.name) : "";
        return (
          '<button type="button" class="nas-move-item" data-nas-move-dir="' +
          escapeHtml(name) +
          '"><span class="nas-icon is-folder" aria-hidden="true"></span><span>' +
          escapeHtml(name) +
          "</span></button>"
        );
      })
      .join("");
  }

  function requestNasPickerPath(path) {
    const drive = selectedDrive();
    if (!drive || !drive.rpcSessionId || !nasMovePicker.open) return;
    nasMovePicker.path = normalizeNasPath(path);
    nasMovePicker.loading = true;
    nasMovePicker.listError = "";
    nasMovePicker.seq = (nasMovePicker.seq || 0) + 1;
    const seq = nasMovePicker.seq;
    hideNasMoveError();
    if (nasMovePath) nasMovePath.value = nasMovePicker.path;
    renderNasMovePicker();
    tauriInvoke("nas_list_path", {
      sessionId: drive.rpcSessionId,
      path: nasMovePicker.path,
    }).catch(function () {
      if (nasMovePicker.seq !== seq) return;
      nasMovePicker.loading = false;
      nasMovePicker.listError = "无法加载目录";
      renderNasMovePicker();
    });
    window.setTimeout(function () {
      if (nasMovePicker.seq !== seq || !nasMovePicker.loading) return;
      nasMovePicker.loading = false;
      nasMovePicker.listError = "目录列表请求超时";
      renderNasMovePicker();
    }, 12000);
  }

  function openNasMoveModalFromCtx() {
    const drive = selectedDrive();
    if (!drive || !nasTaskCtl.ctxName) return;
    let items;
    if (isNasSelected(nasTaskCtl.ctxName)) {
      items = selectedNasItems(drive);
    } else {
      items = [{ name: nasTaskCtl.ctxName, isDir: !!nasTaskCtl.ctxDir }];
    }
    openNasMoveModal(items);
  }

  function openNasMoveModal(items) {
    cancelNasRename();
    const drive = selectedDrive();
    const list = (items || []).filter(function (item) {
      return item && item.name;
    });
    if (!drive || !drive.connected || !list.length) return;
    nasMovePicker.open = true;
    nasMovePicker.items = list;
    nasMovePicker.name = list[0].name;
    nasMovePicker.isDir = !!list[0].isDir;
    nasMovePicker.fromPath = normalizeNasPath(drive.nasPath || "/");
    nasMovePicker.path = nasMovePicker.fromPath;
    nasMovePicker.seq = (nasMovePicker.seq || 0) + 1;
    nasMovePicker.loading = false;
    nasMovePicker.listError = "";
    nasMovePicker.folders = foldersFromNasFiles(drive.nasFiles);
    if (nasMoveHint) {
      nasMoveHint.textContent =
        list.length > 1
          ? "将已选的 " +
            list.length +
            " 个项目移动到下面打开的目录。可编辑路径后回车或点「打开」进入，双击文件夹进入子目录。点「确定」后移动到当前打开的目录。"
          : "将「" +
            list[0].name +
            "」移动到下面打开的目录。可编辑路径后回车或点「打开」进入，双击文件夹进入子目录。点「确定」后移动到当前打开的目录。";
    }
    hideNasMoveError();
    if (nasMoveOk) nasMoveOk.disabled = false;
    if (nasMovePath) nasMovePath.value = nasMovePicker.path;
    openModal("nas-move");
    if (drive.nasLoading || drive.nasListError) {
      requestNasPickerPath(nasMovePicker.path);
    } else {
      renderNasMovePicker();
    }
    window.setTimeout(function () {
      if (nasMovePath) {
        nasMovePath.focus();
        nasMovePath.select();
      }
    }, 0);
  }

  function submitNasMovePicker() {
    if (nasMoveCtl.busy) return;
    hideNasMoveError();
    const items =
      Array.isArray(nasMovePicker.items) && nasMovePicker.items.length
        ? nasMovePicker.items
        : nasMovePicker.name
          ? [{ name: nasMovePicker.name, isDir: nasMovePicker.isDir }]
          : [];
    if (!items.length) {
      showNasMoveError("未选择要移动的项目");
      return;
    }
    const to = normalizeNasPath(nasMovePath ? nasMovePath.value : nasMovePicker.path);
    nasMovePicker.path = to;
    if (nasMovePath) nasMovePath.value = to;
    const parts = partitionNasMove(items, nasMovePicker.fromPath, to);
    if (!parts.valid.length) {
      showNasMoveError(formatNasMoveFailLines(parts.blocked));
      return;
    }
    moveNasEntries(parts.valid, nasMovePicker.fromPath, to, "picker", parts.blocked);
  }

  function bindNasMovePicker() {
    if (nasMoveOk) nasMoveOk.addEventListener("click", submitNasMovePicker);
    if (nasMoveGo) {
      nasMoveGo.addEventListener("click", function () {
        if (!nasMovePicker.open) return;
        requestNasPickerPath(nasMovePath ? nasMovePath.value : nasMovePicker.path);
      });
    }
    if (nasMoveUp) {
      nasMoveUp.addEventListener("click", function () {
        if (!nasMovePicker.open || nasMoveUp.disabled) return;
        requestNasPickerPath(parentNasPath(nasMovePicker.path));
      });
    }
    if (nasMovePath) {
      nasMovePath.addEventListener("keydown", function (event) {
        if (event.key === "Enter") {
          event.preventDefault();
          requestNasPickerPath(nasMovePath.value);
        }
      });
      nasMovePath.addEventListener("input", hideNasMoveError);
    }
    if (nasMoveList) {
      nasMoveList.addEventListener("dblclick", function (event) {
        const btn = event.target.closest("[data-nas-move-dir]");
        if (!btn || !nasMovePicker.open) return;
        const name = btn.getAttribute("data-nas-move-dir") || "";
        if (!name) return;
        requestNasPickerPath(joinNasPath(nasMovePicker.path, name));
      });
    }
  }

  function nasCreateHasExt(name) {
    const i = name.lastIndexOf(".");
    return i > 0 && i < name.length - 1;
  }

  function validateNasCreateName(kind, raw) {
    const name = String(raw || "").trim();
    if (!name) return kind === "folder" ? "请填写文件夹名" : "请填写文件名";
    if (/[\\/:*?"<>|\u0000]/.test(name)) return "名称不能包含特殊字符";
    if (name === "." || name === ".." || name.charAt(0) === ".") return "名称不能以点开头";
    if (kind === "file") {
      if (!nasCreateHasExt(name)) return "文件必须填写扩展名，例如 readme.txt";
    } else if (name.indexOf(".") >= 0) {
      return "文件夹不能包含扩展名，请不要使用点号";
    }
    return "";
  }

  function openNasCreateModal(kind) {
    cancelNasRename();
    nasCreateCtl.kind = kind === "folder" ? "folder" : "file";
    nasCreateCtl.busy = false;
    nasCreateCtl.seq = (nasCreateCtl.seq || 0) + 1;
    if (nasCreateOk) nasCreateOk.disabled = false;
    hideNasCreateError();
    if (nasCreateCtl.kind === "folder") {
      if (nasCreateTitle) nasCreateTitle.textContent = "新建文件夹";
      if (nasCreateLabel) nasCreateLabel.textContent = "文件夹名";
      if (nasCreateHint) nasCreateHint.textContent = "请填写文件夹名，不能包含扩展名（不要使用点号）。";
      if (nasCreateInput) nasCreateInput.placeholder = "例如 文档";
    } else {
      if (nasCreateTitle) nasCreateTitle.textContent = "新建文件";
      if (nasCreateLabel) nasCreateLabel.textContent = "文件名";
      if (nasCreateHint) nasCreateHint.textContent = "请填写文件名，必须包含扩展名，例如 readme.txt";
      if (nasCreateInput) nasCreateInput.placeholder = "例如 readme.txt";
    }
    if (nasCreateInput) nasCreateInput.value = "";
    openModal("nas-create");
    window.setTimeout(function () {
      if (nasCreateInput) nasCreateInput.focus();
    }, 0);
  }

  function submitNasCreate() {
    if (nasCreateCtl.busy) return;
    const session = selectedDrive();
    if (!session || !session.rpcSessionId) {
      showNasCreateError("请先连接网盘");
      return;
    }
    const name = nasCreateInput ? nasCreateInput.value.trim() : "";
    const err = validateNasCreateName(nasCreateCtl.kind, name);
    if (err) {
      showNasCreateError(err);
      return;
    }
    nasCreateCtl.busy = true;
    nasCreateCtl.seq = (nasCreateCtl.seq || 0) + 1;
    const seq = nasCreateCtl.seq;
    if (nasCreateOk) nasCreateOk.disabled = true;
    hideNasCreateError();
    tauriInvoke("nas_create_entry", {
      sessionId: session.rpcSessionId,
      name: name,
      path: session.nasPath || "/",
    }).catch(function () {
      if (nasCreateCtl.seq !== seq) return;
      nasCreateCtl.busy = false;
      if (nasCreateOk) nasCreateOk.disabled = false;
      showNasCreateError("创建失败");
    });
    window.setTimeout(function () {
      if (nasCreateCtl.seq !== seq || !nasCreateCtl.busy) return;
      nasCreateCtl.busy = false;
      if (nasCreateOk) nasCreateOk.disabled = false;
      showNasCreateError("创建超时");
    }, 12000);
  }

  function onNasCreate(payload) {
    const sid = Number(payload.sessionId) || 0;
    const drive = state.drives.find(function (item) {
      return item.rpcSessionId === sid;
    });
    if (!drive) return;
    if (nasCreateCtl.busy) {
      nasCreateCtl.busy = false;
      nasCreateCtl.seq = (nasCreateCtl.seq || 0) + 1;
      if (nasCreateOk) nasCreateOk.disabled = false;
    }
    if (!payload.ok) {
      if (modalNasCreate && !modalNasCreate.hidden) {
        const code = String(payload.error || "");
        if (code === "exists") showNasCreateError("已存在同名文件或文件夹");
        else if (code === "invalid") showNasCreateError("名称不合法");
        else showNasCreateError("创建失败");
      }
      return;
    }
    closeModal();
    const dir = normalizeNasPath(payload.path || "/");
    if (normalizeNasPath(drive.nasPath || "/") === dir) {
      requestNasPath(drive, drive.nasPath);
    }
  }

  function onNasMove(payload) {
    const sid = Number(payload.sessionId) || 0;
    const drive = state.drives.find(function (item) {
      return item.rpcSessionId === sid;
    });
    if (!drive) return;
    if (nasMoveCtl.busy) {
      nasMoveCtl.busy = false;
      nasMoveCtl.seq = (nasMoveCtl.seq || 0) + 1;
      if (nasMoveOk) nasMoveOk.disabled = false;
    }
    const results = Array.isArray(payload.results) && payload.results.length
      ? payload.results.map(function (item) {
          return {
            name: item && item.name ? String(item.name) : "",
            ok: !!(item && item.ok),
            error: item && item.error ? String(item.error) : "",
          };
        })
      : [
          {
            name: payload.name ? String(payload.name) : "",
            ok: !!payload.ok,
            error: payload.error ? String(payload.error) : "",
          },
        ];
    const blocked = nasMoveCtl.blocked || [];
    nasMoveCtl.blocked = [];
    const serverFails = results.filter(function (item) {
      return !item.ok;
    });
    const fails = serverFails.concat(blocked);
    const anyOk = results.some(function (item) {
      return item.ok;
    });
    if (fails.length) {
      if (nasMovePicker.open && !anyOk) {
        showNasMoveError(formatNasMoveFailLines(fails));
      } else {
        if (nasMovePicker.open) closeModal();
        openNasMoveFailPrompt(fails);
      }
    } else if (nasMovePicker.open) {
      closeModal();
    }
    if (anyOk) {
      const moved = {};
      results.forEach(function (item) {
        if (item.ok && item.name) moved[item.name] = true;
      });
      nasSelCtl.names = nasSelCtl.names.filter(function (name) {
        return !moved[name];
      });
    }
    const current = normalizeNasPath(drive.nasPath || "/");
    const from = normalizeNasPath(payload.path || "/");
    const to = normalizeNasPath(payload.targetPath || "/");
    if (anyOk && (current === from || current === to)) {
      requestNasPath(drive, drive.nasPath);
    } else {
      applyNasSelection();
    }
  }

  function ctxNasDeleteItems() {
    const drive = selectedDrive();
    if (!drive || !nasTaskCtl.ctxName) return [];
    if (isNasSelected(nasTaskCtl.ctxName)) return selectedNasItems(drive);
    return [{ name: nasTaskCtl.ctxName, isDir: !!nasTaskCtl.ctxDir }];
  }

  function formatNasDeleteDesc(items) {
    const list = items || [];
    if (list.length === 1) {
      const kind = list[0].isDir ? "文件夹" : "文件";
      const extra = list[0].isDir ? "文件夹会连同其中的内容一起删除。" : "";
      return "确定删除" + kind + "「" + (list[0].name || "") + "」？" + extra + "删除后无法恢复。";
    }
    const hasDir = list.some(function (item) {
      return item && item.isDir;
    });
    return (
      "确定删除已选的 " +
      list.length +
      " 个项目？" +
      (hasDir ? "其中的文件夹会连同内容一起删除。" : "") +
      "删除后无法恢复。"
    );
  }

  function nasDeleteFailReason(key) {
    const text = String(key || "");
    if (text === "missing") return "文件或文件夹不存在";
    if (text === "invalid") return "不能删除该项";
    if (text === "failed") return "删除失败，请稍后重试";
    if (text.indexOf("不能") >= 0 || text.indexOf("不存在") >= 0) return text;
    return "删除失败，请稍后重试";
  }

  function openNasDeleteFailPrompt(fails) {
    const list = (fails || []).filter(function (item) {
      return item && (item.name || item.error);
    });
    if (!list.length) return;
    state.confirmKind = "info";
    if (list.length === 1) {
      confirmTitle.textContent = "无法删除";
      confirmDesc.textContent =
        "「" + (list[0].name || "") + "」" + nasDeleteFailReason(list[0].error);
      setConfirmFailList([]);
    } else {
      confirmTitle.textContent = "部分项目未能删除";
      confirmDesc.textContent = "以下项目没有删除成功：";
      setConfirmFailList(list, nasDeleteFailReason);
    }
    confirmOkBtn.textContent = "知道了";
    confirmOkBtn.className = "btn-primary";
    if (confirmCancelBtn) confirmCancelBtn.hidden = true;
    openModal("confirm");
  }

  function openNasDeleteConfirm() {
    cancelNasRename();
    const drive = selectedDrive();
    const items = ctxNasDeleteItems().filter(function (item) {
      return item && item.name;
    });
    if (!drive || !drive.connected || !drive.rpcSessionId || !items.length) return;
    nasDeleteCtl.items = items;
    nasDeleteCtl.path = normalizeNasPath(drive.nasPath || "/");
    state.confirmKind = "delete-nas";
    confirmTitle.textContent =
      items.length > 1 ? "删除项目" : items[0].isDir ? "删除文件夹" : "删除文件";
    confirmDesc.textContent = formatNasDeleteDesc(items);
    setConfirmFailList([]);
    confirmOkBtn.textContent = "删除";
    confirmOkBtn.className = "btn-danger";
    if (confirmCancelBtn) confirmCancelBtn.hidden = false;
    openModal("confirm");
  }

  function confirmNasDelete() {
    const drive = selectedDrive();
    const items = (nasDeleteCtl.items || []).filter(function (item) {
      return item && item.name;
    });
    const from = nasDeleteCtl.path || (drive && drive.nasPath) || "/";
    state.confirmKind = "";
    closeModal();
    if (!drive || !drive.connected || !drive.rpcSessionId || !items.length || nasDeleteCtl.busy) {
      nasDeleteCtl.items = [];
      return;
    }
    nasDeleteCtl.busy = true;
    nasDeleteCtl.seq = (nasDeleteCtl.seq || 0) + 1;
    const seq = nasDeleteCtl.seq;
    tauriInvoke("nas_delete_entry", {
      sessionId: drive.rpcSessionId,
      names: items.map(function (item) {
        return item.name;
      }),
      path: normalizeNasPath(from),
    }).catch(function () {
      if (nasDeleteCtl.seq !== seq) return;
      nasDeleteCtl.busy = false;
      openNasDeleteFailPrompt(
        items.map(function (item) {
          return { name: item.name, error: "failed" };
        })
      );
    });
    window.setTimeout(function () {
      if (nasDeleteCtl.seq !== seq || !nasDeleteCtl.busy) return;
      nasDeleteCtl.busy = false;
      openNasDeleteFailPrompt(
        items.map(function (item) {
          return { name: item.name, error: "failed" };
        })
      );
    }, 60000);
  }

  function onNasDelete(payload) {
    const sid = Number(payload.sessionId) || 0;
    const drive = state.drives.find(function (item) {
      return item.rpcSessionId === sid;
    });
    if (!drive) return;
    if (nasDeleteCtl.busy) {
      nasDeleteCtl.busy = false;
      nasDeleteCtl.seq = (nasDeleteCtl.seq || 0) + 1;
    }
    nasDeleteCtl.items = [];
    const results = Array.isArray(payload.results) && payload.results.length
      ? payload.results.map(function (item) {
          return {
            name: item && item.name ? String(item.name) : "",
            ok: !!(item && item.ok),
            error: item && item.error ? String(item.error) : "",
          };
        })
      : [
          {
            name: payload.name ? String(payload.name) : "",
            ok: !!payload.ok,
            error: payload.error ? String(payload.error) : "",
          },
        ];
    const fails = results.filter(function (item) {
      return !item.ok;
    });
    const anyOk = results.some(function (item) {
      return item.ok;
    });
    if (fails.length) openNasDeleteFailPrompt(fails);
    if (anyOk) {
      const removed = {};
      results.forEach(function (item) {
        if (item.ok && item.name) removed[item.name] = true;
      });
      nasSelCtl.names = nasSelCtl.names.filter(function (name) {
        return !removed[name];
      });
    }
    const current = normalizeNasPath(drive.nasPath || "/");
    const from = normalizeNasPath(payload.path || "/");
    if (anyOk && current === from) {
      requestNasPath(drive, drive.nasPath);
    } else {
      applyNasSelection();
    }
  }

  function visibleNasItemEl(name) {
    const els = visibleNasItemEls();
    for (let i = 0; i < els.length; i += 1) {
      if ((els[i].getAttribute("data-nas-name") || "") === name) return els[i];
    }
    return null;
  }

  function nasRenameTextEl(itemEl) {
    if (!itemEl) return null;
    return itemEl.querySelector(".nas-name-text") || itemEl.querySelector(".nas-tile-name");
  }

  function clearNasRenameDom() {
    document.querySelectorAll(".nas-rename-input").forEach(function (el) {
      el.remove();
    });
    document.querySelectorAll(".is-renaming").forEach(function (el) {
      el.classList.remove("is-renaming");
    });
    document.querySelectorAll(".nas-name-text, .nas-tile-name").forEach(function (el) {
      el.hidden = false;
    });
  }

  function cancelNasRename() {
    nasRenameCtl.active = false;
    nasRenameCtl.busy = false;
    nasRenameCtl.seq = (nasRenameCtl.seq || 0) + 1;
    nasRenameCtl.name = "";
    nasRenameCtl.draft = "";
    clearNasRenameDom();
  }

  function selectNasRenameStem(input, name, isDir) {
    if (!input) return;
    input.focus();
    const text = String(name || "");
    if (isDir) {
      input.select();
      return;
    }
    const i = text.lastIndexOf(".");
    if (i > 0) input.setSelectionRange(0, i);
    else input.select();
  }

  function validateNasRenameName(raw) {
    const name = String(raw || "").trim();
    if (!name) return "请填写名称";
    if (/[\\/:*?"<>|\u0000]/.test(name)) return "名称不能包含特殊字符";
    if (name === "." || name === ".." || name.charAt(0) === ".") return "名称不能以点开头";
    return "";
  }

  function startNasRenameFromCtx() {
    if (!nasTaskCtl.ctxName) return;
    startNasRename(nasTaskCtl.ctxName, !!nasTaskCtl.ctxDir);
  }

  function startNasRename(name, isDir) {
    const drive = selectedDrive();
    if (!drive || !drive.connected || !name) return;
    nasRenameCtl.active = true;
    nasRenameCtl.busy = false;
    nasRenameCtl.name = name;
    nasRenameCtl.isDir = !!isDir;
    nasRenameCtl.path = normalizeNasPath(drive.nasPath || "/");
    nasRenameCtl.draft = name;
    nasRenameCtl.selStart = 0;
    nasRenameCtl.selEnd = String(name || "").length;
    mountNasRenameEditor(true);
  }

  function mountNasRenameEditor(selectStem) {
    if (!nasRenameCtl.active || !nasRenameCtl.name) return;
    const drive = selectedDrive();
    if (!drive || normalizeNasPath(drive.nasPath || "/") !== nasRenameCtl.path) {
      cancelNasRename();
      return;
    }
    if (!findNasEntry(drive, nasRenameCtl.name)) {
      cancelNasRename();
      return;
    }
    const itemEl = visibleNasItemEl(nasRenameCtl.name);
    const textEl = nasRenameTextEl(itemEl);
    if (!itemEl || !textEl) return;
    clearNasRenameDom();
    textEl.hidden = true;
    itemEl.classList.add("is-renaming");
    const input = document.createElement("input");
    input.type = "text";
    input.className = "nas-rename-input";
    input.spellcheck = false;
    input.maxLength = 200;
    input.value = nasRenameCtl.draft || nasRenameCtl.name;
    input.disabled = !!nasRenameCtl.busy;
    textEl.parentNode.insertBefore(input, textEl.nextSibling);
    input.addEventListener("pointerdown", function (event) {
      event.stopPropagation();
    });
    input.addEventListener("click", function (event) {
      event.stopPropagation();
    });
    input.addEventListener("input", function () {
      nasRenameCtl.draft = input.value;
      input.classList.remove("is-error");
    });
    input.addEventListener("keydown", function (event) {
      if (event.key === "Enter") {
        event.preventDefault();
        event.stopPropagation();
        submitNasRename();
        return;
      }
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        cancelNasRename();
      }
    });
    input.addEventListener("keyup", function () {
      nasRenameCtl.selStart = input.selectionStart;
      nasRenameCtl.selEnd = input.selectionEnd;
    });
    window.setTimeout(function () {
      if (!nasRenameCtl.active) return;
      const live = itemEl.querySelector(".nas-rename-input");
      if (!live || live.disabled) return;
      if (selectStem) selectNasRenameStem(live, nasRenameCtl.name, nasRenameCtl.isDir);
      else {
        live.focus();
        const start = Number.isFinite(nasRenameCtl.selStart) ? nasRenameCtl.selStart : live.value.length;
        const end = Number.isFinite(nasRenameCtl.selEnd) ? nasRenameCtl.selEnd : live.value.length;
        try {
          live.setSelectionRange(start, end);
        } catch (_) {}
      }
    }, 0);
  }

  function restoreNasRenameEditor() {
    if (!nasRenameCtl.active) return;
    mountNasRenameEditor(false);
  }

  function submitNasRename() {
    if (!nasRenameCtl.active || nasRenameCtl.busy) return;
    const drive = selectedDrive();
    const next = String(nasRenameCtl.draft || "").trim();
    const prev = nasRenameCtl.name;
    if (!drive || !drive.connected || !drive.rpcSessionId) {
      cancelNasRename();
      return;
    }
    if (!next || next === prev) {
      cancelNasRename();
      return;
    }
    const err = validateNasRenameName(next);
    const input = document.querySelector(".nas-rename-input");
    if (err) {
      if (input) {
        input.classList.add("is-error");
        input.focus();
        input.select();
      }
      return;
    }
    nasRenameCtl.busy = true;
    nasRenameCtl.seq = (nasRenameCtl.seq || 0) + 1;
    const seq = nasRenameCtl.seq;
    if (input) input.disabled = true;
    tauriInvoke("nas_rename_entry", {
      sessionId: drive.rpcSessionId,
      name: prev,
      newName: next,
      path: nasRenameCtl.path || drive.nasPath || "/",
    }).catch(function () {
      if (nasRenameCtl.seq !== seq) return;
      nasRenameCtl.busy = false;
      mountNasRenameEditor(false);
      const live = document.querySelector(".nas-rename-input");
      if (live) live.classList.add("is-error");
    });
    window.setTimeout(function () {
      if (nasRenameCtl.seq !== seq || !nasRenameCtl.busy) return;
      nasRenameCtl.busy = false;
      mountNasRenameEditor(false);
      const live = document.querySelector(".nas-rename-input");
      if (live) live.classList.add("is-error");
    }, 12000);
  }

  function onNasRename(payload) {
    const sid = Number(payload.sessionId) || 0;
    const drive = state.drives.find(function (item) {
      return item.rpcSessionId === sid;
    });
    if (!drive) return;
    const from = payload.name ? String(payload.name) : "";
    const to = payload.newName ? String(payload.newName) : "";
    const path = normalizeNasPath(payload.path || "/");
    const renaming =
      nasRenameCtl.active &&
      nasRenameCtl.name === from &&
      nasRenameCtl.path === path;
    if (renaming) {
      nasRenameCtl.busy = false;
      nasRenameCtl.seq = (nasRenameCtl.seq || 0) + 1;
    }
    if (!payload.ok) {
      if (renaming) {
        mountNasRenameEditor(false);
        const live = document.querySelector(".nas-rename-input");
        if (live) {
          live.classList.add("is-error");
          live.focus();
          live.select();
        }
      }
      return;
    }
    if (renaming) cancelNasRename();
    nasSelCtl.names = nasSelCtl.names.map(function (name) {
      return name === from ? to : name;
    });
    const current = normalizeNasPath(drive.nasPath || "/");
    if (current === path) requestNasPath(drive, drive.nasPath);
    else applyNasSelection();
  }

  function bindNasSearch() {
    if (!nasSearchInput) return;
    nasSearchInput.addEventListener("input", function () {
      scheduleNasSearch();
    });
    nasSearchInput.addEventListener("focus", function () {
      if (nasSearchCtl.keyword && (nasSearchCtl.results.length || nasSearchCtl.loading)) {
        showNasSearchDrop();
      }
    });
    nasSearchInput.addEventListener("keydown", function (event) {
      if (event.key === "ArrowDown") {
        event.preventDefault();
        moveNasSearchHighlight(1);
        return;
      }
      if (event.key === "ArrowUp") {
        event.preventDefault();
        moveNasSearchHighlight(-1);
        return;
      }
      if (event.key === "Enter") {
        event.preventDefault();
        const items = nasSearchCtl.results || [];
        if (!items.length) return;
        const idx = nasSearchCtl.highlight >= 0 ? nasSearchCtl.highlight : 0;
        openNasSearchHit(items[idx]);
        return;
      }
      if (event.key === "Escape") {
        event.preventDefault();
        hideNasSearchDrop();
        nasSearchInput.blur();
      }
    });
    if (nasSearchDrop) {
      nasSearchDrop.addEventListener("mousedown", function (event) {
        event.preventDefault();
      });
      nasSearchDrop.addEventListener("click", function (event) {
        const btn = event.target.closest("[data-nas-search-idx]");
        if (!btn) return;
        const idx = Number(btn.getAttribute("data-nas-search-idx"));
        const hit = nasSearchCtl.results[idx];
        if (hit) openNasSearchHit(hit);
      });
    }
    document.addEventListener(
      "pointerdown",
      function (event) {
        if (!nasSearchDrop || nasSearchDrop.hidden) return;
        if (event.target.closest(".nas-search")) return;
        hideNasSearchDrop();
      },
      true
    );
  }

  function scheduleNasSearch() {
    const query = nasSearchInput ? String(nasSearchInput.value || "").trim() : "";
    if (nasSearchCtl.timer) {
      window.clearTimeout(nasSearchCtl.timer);
      nasSearchCtl.timer = 0;
    }
    if (!query) {
      nasSearchCtl.keyword = "";
      nasSearchCtl.results = [];
      nasSearchCtl.loading = false;
      nasSearchCtl.seq = (nasSearchCtl.seq || 0) + 1;
      hideNasSearchDrop();
      return;
    }
    nasSearchCtl.timer = window.setTimeout(function () {
      runNasSearch(query);
    }, 250);
  }

  function runNasSearch(query) {
    const drive = selectedDrive();
    if (!drive || !drive.connected || !drive.rpcSessionId) return;
    nasSearchCtl.keyword = query;
    nasSearchCtl.loading = true;
    nasSearchCtl.highlight = -1;
    nasSearchCtl.seq = (nasSearchCtl.seq || 0) + 1;
    const seq = nasSearchCtl.seq;
    renderNasSearchDrop();
    showNasSearchDrop();
    tauriInvoke("nas_search", {
      sessionId: drive.rpcSessionId,
      keyword: query,
    }).catch(function () {
      if (nasSearchCtl.seq !== seq) return;
      nasSearchCtl.loading = false;
      nasSearchCtl.results = [];
      nasSearchCtl.truncated = false;
      renderNasSearchDrop("搜索失败");
    });
    window.setTimeout(function () {
      if (nasSearchCtl.seq !== seq || !nasSearchCtl.loading) return;
      nasSearchCtl.loading = false;
      nasSearchCtl.results = [];
      renderNasSearchDrop("搜索超时");
    }, 32000);
  }

  function onNasSearch(payload) {
    const sid = Number(payload.sessionId) || 0;
    const drive = state.drives.find(function (item) {
      return item.rpcSessionId === sid;
    });
    if (!drive) return;
    const keyword = String(payload.keyword || "").trim();
    const current = nasSearchInput ? String(nasSearchInput.value || "").trim() : "";
    if (keyword !== current) return;
    nasSearchCtl.loading = false;
    nasSearchCtl.keyword = keyword;
    nasSearchCtl.truncated = !!payload.truncated;
    nasSearchCtl.results = Array.isArray(payload.results)
      ? payload.results
          .map(function (item) {
            return {
              name: item && item.name ? String(item.name) : "",
              filePath: normalizeNasPath((item && item.filePath) || "/"),
              isDir: !!(item && item.isDir),
            };
          })
          .filter(function (item) {
            return item.name;
          })
      : [];
    nasSearchCtl.highlight = nasSearchCtl.results.length ? 0 : -1;
    if (!payload.ok) {
      nasSearchCtl.results = [];
      renderNasSearchDrop("搜索超时");
      showNasSearchDrop();
      return;
    }
    renderNasSearchDrop();
    showNasSearchDrop();
  }

  function highlightNasSearchName(name, query) {
    const src = String(name || "");
    const q = String(query || "");
    if (!q) return escapeHtml(src);
    const i = src.toLowerCase().indexOf(q.toLowerCase());
    if (i < 0) return escapeHtml(src);
    return (
      escapeHtml(src.slice(0, i)) +
      "<mark>" +
      escapeHtml(src.slice(i, i + q.length)) +
      "</mark>" +
      escapeHtml(src.slice(i + q.length))
    );
  }

  function renderNasSearchDrop(errorText) {
    if (!nasSearchDrop) return;
    if (errorText) {
      nasSearchDrop.innerHTML = '<div class="nas-search-empty">' + escapeHtml(errorText) + "</div>";
      return;
    }
    if (nasSearchCtl.loading) {
      nasSearchDrop.innerHTML = '<div class="nas-search-empty">正在搜索…</div>';
      return;
    }
    const items = nasSearchCtl.results || [];
    if (!items.length) {
      nasSearchDrop.innerHTML = '<div class="nas-search-empty">没有匹配的文件或文件夹</div>';
      return;
    }
    const q = nasSearchCtl.keyword;
    let html = items
      .map(function (item, idx) {
        const kind = nasKindMeta(classifyNasName(item.name, item.isDir));
        const loc = item.isDir ? joinNasPath(item.filePath, item.name) : item.filePath;
        return (
          '<button type="button" class="nas-search-item' +
          (idx === nasSearchCtl.highlight ? " is-on" : "") +
          '" data-nas-search-idx="' +
          idx +
          '"><span class="nas-icon ' +
          kind.cls +
          '"></span><span class="nas-search-meta"><strong>' +
          highlightNasSearchName(item.name, q) +
          "</strong><em>" +
          escapeHtml(loc || "/") +
          "</em></span></button>"
        );
      })
      .join("");
    if (nasSearchCtl.truncated) {
      html += '<div class="nas-search-more">仅显示部分结果，请再输入更完整的名称</div>';
    }
    nasSearchDrop.innerHTML = html;
  }

  function showNasSearchDrop() {
    if (nasSearchDrop) nasSearchDrop.hidden = false;
  }

  function hideNasSearchDrop() {
    if (nasSearchDrop) nasSearchDrop.hidden = true;
    nasSearchCtl.highlight = -1;
  }

  function moveNasSearchHighlight(delta) {
    const items = nasSearchCtl.results || [];
    if (!items.length) return;
    showNasSearchDrop();
    let next = nasSearchCtl.highlight + delta;
    if (next < 0) next = items.length - 1;
    if (next >= items.length) next = 0;
    nasSearchCtl.highlight = next;
    renderNasSearchDrop();
    if (!nasSearchDrop) return;
    const el = nasSearchDrop.querySelector('[data-nas-search-idx="' + next + '"]');
    if (el && el.scrollIntoView) el.scrollIntoView({ block: "nearest" });
  }

  function openNasSearchHit(hit) {
    const drive = selectedDrive();
    if (!drive || !hit || !hit.name) return;
    hideNasSearchDrop();
    if (hit.isDir) {
      nasSearchCtl.pendingSelect = null;
      requestNasPath(drive, joinNasPath(hit.filePath, hit.name));
      return;
    }
    const parent = normalizeNasPath(hit.filePath);
    nasSearchCtl.pendingSelect = { path: parent, name: hit.name };
    if (normalizeNasPath(drive.nasPath || "/") === parent && !drive.nasLoading) {
      setNasSelection([hit.name]);
      scrollNasSelectionIntoView();
      nasSearchCtl.pendingSelect = null;
      return;
    }
    requestNasPath(drive, parent);
  }

  function applyPendingNasSearchSelect(session) {
    const pending = nasSearchCtl.pendingSelect;
    if (!pending || !pending.name || !session) return;
    if (normalizeNasPath(session.nasPath || "/") !== normalizeNasPath(pending.path)) return;
    if (session.nasLoading) return;
    nasSearchCtl.pendingSelect = null;
    if (!findNasEntry(session, pending.name)) return;
    setNasSelection([pending.name]);
    scrollNasSelectionIntoView();
  }

  function scrollNasSelectionIntoView() {
    const el = visibleNasItemEl(nasSelCtl.names[0] || "");
    if (el && el.scrollIntoView) el.scrollIntoView({ block: "nearest" });
  }

  function nasZipFailReason(code) {
    const key = String(code || "");
    if (key === "exists") return "已存在同名压缩文件，无法继续压缩";
    if (key === "missing") return "文件夹不存在";
    if (key === "invalid") return "不能压缩该项";
    if (key === "failed") return "压缩失败，请稍后重试";
    if (key.indexOf("已存在") >= 0 || key.indexOf("不能") >= 0) return key;
    return "压缩失败，请稍后重试";
  }

  function setNasZipProgress(pct) {
    const n = Math.max(0, Math.min(100, Math.round(Number(pct) || 0)));
    nasZipCtl.progress = n;
    if (nasZipBar) nasZipBar.style.width = n + "%";
    if (nasZipPct) nasZipPct.textContent = n + "%";
  }

  function showNasZipBusyUI(name) {
    if (nasZipTitle) nasZipTitle.textContent = "正在压缩";
    if (nasZipDesc) nasZipDesc.textContent = "正在压缩「" + name + "」，请稍候…";
    if (nasZipProgress) nasZipProgress.hidden = false;
    if (nasZipSpinner) nasZipSpinner.hidden = false;
    if (nasZipError) {
      nasZipError.hidden = true;
      nasZipError.textContent = "";
    }
    if (nasZipAsync) nasZipAsync.hidden = false;
    if (nasZipOk) nasZipOk.hidden = true;
    setNasZipProgress(nasZipCtl.progress || 0);
  }

  function startNasZipFromCtx() {
    const drive = selectedDrive();
    const name = nasTaskCtl.ctxName;
    if (!drive || !drive.connected || !drive.rpcSessionId || !nasTaskCtl.ctxDir || !name) return;
    if (nasZipCtl.busy) {
      if (nasZipCtl.name === name && nasZipCtl.path === normalizeNasPath(drive.nasPath || "/")) {
        showNasZipBusyUI(name);
        openModal("nas-zip");
        return;
      }
      openInfoPrompt("正在压缩", "当前已有压缩任务进行中，请等待完成后再试。");
      return;
    }
    cancelNasRename();
    nasZipCtl.busy = true;
    nasZipCtl.async = false;
    nasZipCtl.name = name;
    nasZipCtl.path = normalizeNasPath(drive.nasPath || "/");
    nasZipCtl.zipName = name + ".zip";
    nasZipCtl.progress = 0;
    nasZipCtl.selectName = "";
    nasZipCtl.seq = (nasZipCtl.seq || 0) + 1;
    showNasZipBusyUI(name);
    openModal("nas-zip");
    tauriInvoke("nas_zip_entry", {
      sessionId: drive.rpcSessionId,
      name: name,
      path: nasZipCtl.path,
    }).catch(function () {
      finishNasZip(false, "failed");
    });
  }

  function finishNasZip(ok, error) {
    const name = nasZipCtl.name;
    const zipName = nasZipCtl.zipName || name + ".zip";
    const path = nasZipCtl.path;
    const wasAsync = nasZipCtl.async;
    const drive = selectedDrive();
    nasZipCtl.busy = false;
    if (ok) {
      setNasZipProgress(100);
      if (nasZipSpinner) nasZipSpinner.hidden = true;
      if (nasZipTitle) nasZipTitle.textContent = "压缩完成";
      if (nasZipDesc) nasZipDesc.textContent = "已生成「" + zipName + "」。";
      if (nasZipError) nasZipError.hidden = true;
      if (drive && normalizeNasPath(drive.nasPath || "/") === path) {
        nasZipCtl.selectName = zipName;
        requestNasPath(drive, drive.nasPath);
      }
      if (wasAsync || !modalNasZip || modalNasZip.hidden) {
        openInfoPrompt("压缩完成", "已生成「" + zipName + "」。");
        return;
      }
    } else {
      const reason = nasZipFailReason(error);
      if (nasZipSpinner) nasZipSpinner.hidden = true;
      if (nasZipTitle) nasZipTitle.textContent = "无法压缩";
      if (nasZipDesc) nasZipDesc.textContent = "「" + name + "」没有压缩成功。";
      if (nasZipError) {
        nasZipError.hidden = false;
        nasZipError.textContent = reason;
      }
      if (wasAsync || !modalNasZip || modalNasZip.hidden) {
        openInfoPrompt("无法压缩", "「" + name + "」" + reason);
        return;
      }
    }
    if (nasZipAsync) nasZipAsync.hidden = true;
    if (nasZipOk) nasZipOk.hidden = false;
  }

  function onNasZip(payload) {
    const sid = Number(payload.sessionId) || 0;
    const drive = state.drives.find(function (item) {
      return item.rpcSessionId === sid;
    });
    if (!drive) return;
    const name = payload.name ? String(payload.name) : "";
    const path = normalizeNasPath(payload.path || "/");
    if (!nasZipCtl.busy || nasZipCtl.name !== name || nasZipCtl.path !== path) return;
    if (payload.zipName) nasZipCtl.zipName = String(payload.zipName);
    const done = !!payload.done;
    if (!done) {
      setNasZipProgress(payload.progress);
      return;
    }
    finishNasZip(!!payload.ok, payload.error);
  }

  function applyPendingNasZipSelect(session) {
    const name = nasZipCtl.selectName;
    if (!name || !session) return;
    if (session.nasLoading) return;
    nasZipCtl.selectName = "";
    if (!findNasEntry(session, name)) return;
    setNasSelection([name]);
    scrollNasSelectionIntoView();
  }

  function selectedDrive() {
    const session = findItem(state.selectedId);
    return isDrive(session) ? session : null;
  }

  function normalizeNasPath(raw) {
    const parts = String(raw || "/")
      .replace(/\\/g, "/")
      .split("/");
    const out = [];
    for (let i = 0; i < parts.length; i += 1) {
      const part = parts[i];
      if (!part || part === ".") continue;
      if (part === "..") {
        out.pop();
        continue;
      }
      if (part.charAt(0) === ".") continue;
      out.push(part);
    }
    return out.length ? "/" + out.join("/") : "/";
  }

  function parentNasPath(raw) {
    const cur = normalizeNasPath(raw);
    if (cur === "/") return "/";
    const parts = cur.split("/").filter(Boolean);
    parts.pop();
    return parts.length ? "/" + parts.join("/") : "/";
  }

  function joinNasPath(base, name) {
    const clean = String(name || "").replace(/[\\/]/g, "");
    if (!clean || clean.charAt(0) === "." || clean === "." || clean === "..") {
      return normalizeNasPath(base);
    }
    const root = normalizeNasPath(base);
    return root === "/" ? "/" + clean : root + "/" + clean;
  }

  function requestNasPath(session, path) {
    if (!session || !session.rpcSessionId) return;
    const next = normalizeNasPath(path);
    if (normalizeNasPath(session.nasPath || "/") !== next) clearNasSelection();
    session.nasPath = next;
    session.nasLoading = true;
    session.nasListError = "";
    session.nasListSeq = (session.nasListSeq || 0) + 1;
    scheduleNasListTimeout(session, session.nasListSeq);
    tauriInvoke("nas_list_path", {
      sessionId: session.rpcSessionId,
      path: session.nasPath,
    }).catch(function () {});
    if (state.selectedId === session.id) renderNasExplorer(session);
  }

  function scheduleNasListTimeout(session, seq) {
    window.setTimeout(function () {
      if (!session || session.nasListSeq !== seq || !session.nasLoading) return;
      session.nasLoading = false;
      session.nasListError = "文件列表请求超时";
      if (state.selectedId === session.id) renderNasExplorer(session);
    }, 12000);
  }

  function applyNasView() {
    if (nasFiles) nasFiles.setAttribute("data-view", nasViewMode);
    if (nasViewListBtn) nasViewListBtn.classList.toggle("is-active", nasViewMode === "list");
    if (nasViewGridBtn) nasViewGridBtn.classList.toggle("is-active", nasViewMode === "grid");
    restoreNasRenameEditor();
  }

  function startNasWatch(session) {
    if (!isDrive(session) || !session.rpcSessionId) return;
    cancelNasRename();
    hideNasSearchDrop();
    clearNasSelection();
    session.nasWatching = true;
    session.nasPath = "/";
    session.nasFiles = [];
    session.nasLoading = true;
    session.nasListError = "";
    session.nasListSeq = (session.nasListSeq || 0) + 1;
    scheduleNasListTimeout(session, session.nasListSeq);
    tauriInvoke("nas_watch_start", { sessionId: session.rpcSessionId }).catch(function () {});
    if (state.selectedId === session.id) renderNasExplorer(session);
  }

  function stopNasWatch(session) {
    if (!session) return;
    session.nasWatching = false;
    const sid = session.rpcSessionId || 0;
    if (!sid) return;
    tauriInvoke("nas_watch_stop", { sessionId: sid }).catch(function () {});
  }

  function onNasInfo(payload) {
    const sid = Number(payload.sessionId) || 0;
    if (!sid) return;
    const drive = state.drives.find(function (item) {
      return item.rpcSessionId === sid;
    });
    if (!drive) return;
    drive.nasInfo = {
      diskSize: Number(payload.diskSize) || 0,
      banlenSize: Number(payload.banlenSize) || 0,
      fileNum: Number(payload.fileNum) || 0,
      updatedAt: Date.now(),
    };
    if (state.selectedId === drive.id) renderNasExplorer(drive);
  }

  function onNasPath(payload) {
    const sid = Number(payload.sessionId) || 0;
    if (!sid) return;
    const drive = state.drives.find(function (item) {
      return item.rpcSessionId === sid;
    });
    if (!drive) return;
    const eventPath = payload.path ? normalizeNasPath(payload.path) : "";
    const mappedOk = payload.ok !== false;
    const mapped = mappedOk ? mapNasListEntries(payload.entries) : [];
    let usedByPicker = false;
    if (nasMovePicker.open) {
      const match = eventPath ? eventPath === nasMovePicker.path : !!nasMovePicker.loading;
      if (match) {
        usedByPicker = true;
        nasMovePicker.loading = false;
        nasMovePicker.listError = mappedOk ? "" : "目录列表请求超时";
        nasMovePicker.folders = mappedOk
          ? mapped
              .filter(function (item) {
                return item.isDir;
              })
              .map(function (item) {
                return { name: item.name };
              })
          : [];
        renderNasMovePicker();
      }
    }
    const drivePath = normalizeNasPath(drive.nasPath || "/");
    if (eventPath && eventPath !== drivePath) return;
    if (!eventPath && usedByPicker && nasMovePicker.path !== drivePath) return;
    drive.nasLoading = false;
    if (!mappedOk) {
      drive.nasListError = "文件列表请求超时";
    } else {
      drive.nasListError = "";
      drive.nasFiles = mapped;
    }
    if (state.selectedId === drive.id) renderNasExplorer(drive);
  }

  function findNasEntry(session, name) {
    return ((session && session.nasFiles) || []).find(function (item) {
      return item && item.name === name;
    }) || null;
  }

  function previewNasFile(session, name) {
    if (!session || !session.rpcSessionId || !name) return;
    const file = findNasEntry(session, name);
    if (!file || file.isDir) return;
    const kind = classifyNasName(file.name, false);
    if (!prepareNasOpen(kind)) return;
    if (kind === "image") previewNasImage(session, file);
    else if (kind === "video" || kind === "audio") previewNasStream(session, file, kind);
    else if (isNasTextKind(kind)) previewNasText(session, file, kind);
    else if (isNasOfficeKind(kind)) previewNasOffice(session, file, kind);
  }

  function previewNasImage(session, file) {
    const name = file.name;
    if (file.size != null && isFinite(file.size) && file.size >= NAS_PREVIEW_MAX) {
      session.nasPreviewSeq = (session.nasPreviewSeq || 0) + 1;
      session.nasPreviewName = "";
      session.nasPreviewKind = "image";
      showNasPreview(session, name, "", "图片过大请下载到本地查看", false);
      return;
    }
    beginNasPreview(session, name, "image");
    const seq = session.nasPreviewSeq;
    showNasPreview(session, name, "", "正在加载预览…", true);
    requestNasPreviewFile(session, file, seq, function (result) {
      if (!result || !result.cached || !result.localPath) return;
      openNasPreviewImage(session, name, result.localPath);
    });
    window.setTimeout(function () {
      if (!session || session.nasPreviewSeq !== seq) return;
      if (!nasPreview || nasPreview.hidden) return;
      if (nasPreviewImage && !nasPreviewImage.hidden && nasPreviewImage.getAttribute("src")) return;
      showNasPreview(session, name, "", "预览超时", false);
    }, 90000);
  }

  function isNasStreamKind(kind) {
    return kind === "video" || kind === "audio";
  }

  function isNasTextKind(kind) {
    return kind === "text" || kind === "code";
  }

  function isNasOfficeKind(kind) {
    return kind === "doc" || kind === "pdf";
  }

  function previewNasText(session, file, kind) {
    const name = file.name;
    if (file.size != null && isFinite(file.size) && file.size >= NAS_TEXT_MAX) {
      session.nasPreviewSeq = (session.nasPreviewSeq || 0) + 1;
      session.nasPreviewName = "";
      session.nasPreviewKind = kind;
      showNasPreview(session, name, "", "文件过大无法在线编辑", false);
      return;
    }
    beginNasPreview(session, name, kind);
    const seq = session.nasPreviewSeq;
    showNasPreview(session, name, "", "正在加载文件…", true);
    requestNasPreviewFile(session, file, seq, function (result) {
      if (!result || !result.localPath) {
        showNasPreview(session, name, "", "无法打开，请稍后重试", false);
        return;
      }
      if (result.cached) {
        openNasTextFromPath(session, result.localPath);
      }
    });
    window.setTimeout(function () {
      if (!session || session.nasPreviewSeq !== seq) return;
      if (!nasPreview || nasPreview.hidden) return;
      if (nasEditorEl && !nasEditorEl.hidden) return;
      showNasPreview(session, name, "", "打开超时", false);
    }, 90000);
  }

  function previewNasOffice(session, file, kind) {
    const name = file.name;
    if (String(name || "").indexOf("~$") === 0) return;
    const officeKind = kind || "doc";
    if (file.size != null && isFinite(file.size) && file.size >= NAS_OFFICE_MAX) {
      session.nasPreviewSeq = (session.nasPreviewSeq || 0) + 1;
      session.nasPreviewName = "";
      session.nasPreviewKind = officeKind;
      showNasPreview(session, name, "", "文件过大无法在线编辑", false);
      return;
    }
    if (!prepareNasOpen(officeKind)) return;
    const path = normalizeNasPath(session.nasPath || "/");
    const sid = session.rpcSessionId || 0;
    let doc = findOfficeDoc(sid, path, name);
    if (doc) {
      reopenOfficeDoc(doc);
      return;
    }
    doc = createOfficeDoc({
      sessionId: sid,
      name: name,
      path: path,
      pending: true,
    });
    focusOfficeDoc(doc, true);
    showNasOfficePanelFor(doc, "正在下载文件…", true);
    pumpOfficeDownload();
  }

  function prepareNasOpen(kind) {
    if (nasEditorEl && !nasEditorEl.hidden) {
      if (nasEditorDirty()) {
        requestNasEditorExit();
        return false;
      }
      forceCloseNasEditor();
    }
    if (nasOfficeEl && !nasOfficeEl.hidden && !isNasOfficeKind(kind)) {
      hideNasOfficeOverlay();
    }
    return true;
  }

  function nasStreamEl(kind) {
    return kind === "audio" ? nasPreviewAudio : nasPreviewVideo;
  }

  function nasStreamFailText(kind) {
    return kind === "audio" ? "无法预览该音频" : "无法预览该视频";
  }

  function previewNasStream(session, file, kind) {
    const name = file.name;
    const max = kind === "audio" ? NAS_AUDIO_MAX : NAS_VIDEO_MAX;
    const tooBig =
      kind === "audio" ? "音频过大请下载到本地查看" : "视频过大请下载到本地查看";
    if (file.size != null && isFinite(file.size) && file.size >= max) {
      session.nasPreviewSeq = (session.nasPreviewSeq || 0) + 1;
      session.nasPreviewName = "";
      session.nasPreviewKind = kind;
      showNasPreview(session, name, "", tooBig, false);
      return;
    }
    beginNasPreview(session, name, kind);
    const seq = session.nasPreviewSeq;
    showNasPreview(session, name, "", "正在加载预览…", true);
    requestNasPreviewFile(session, file, seq, function (result) {
      if (!result || !result.localPath) {
        showNasPreview(session, name, "", "无法预览，请稍后重试", false);
        return;
      }
      if (result.cached) {
        openNasPreviewStream(session, result.localPath, true);
        return;
      }
      startNasStreamPump(session, seq, result.localPath);
    });
  }

  function beginNasPreview(session, name, kind) {
    stopNasStreamPump();
    session.nasPreviewSeq = (session.nasPreviewSeq || 0) + 1;
    session.nasPreviewName = name;
    session.nasPreviewKind = kind;
    session.nasPreviewPath = normalizeNasPath(session.nasPath || "/");
  }

  function requestNasPreviewFile(session, file, seq, onResult) {
    const size = file.size == null || !isFinite(file.size) ? null : Number(file.size);
    tauriInvoke("nas_get_file", {
      sessionId: session.rpcSessionId,
      fileName: file.name,
      filePath: session.nasPreviewPath,
      size: size,
    })
      .then(function (result) {
        if (!session || session.nasPreviewSeq !== seq) return;
        if (result && result.busy) {
          showNasPreview(session, file.name, "", "有文件正在传输，请稍候", false);
          return;
        }
        onResult(result || null);
      })
      .catch(function () {
        if (!session || session.nasPreviewSeq !== seq) return;
        onResult(null);
      });
  }

  function onNasFile(payload) {
    const sid = Number(payload.sessionId) || 0;
    if (!sid) return;
    const drive = state.drives.find(function (item) {
      return item.rpcSessionId === sid;
    });
    if (!drive) return;
    const name = String(payload.fileName || "");
    const dir = normalizeNasPath(payload.filePath || "/");
    const officeDoc = findOfficeDoc(sid, dir, name);
    if (officeDoc && (officeDoc.pending || !officeDoc.opened)) {
      officeDoc.downloadStarted = false;
      if (!payload.ok || !payload.localPath) {
        officeDoc.pending = false;
        setOfficeDocHint(officeDoc, "无法打开，请稍后重试");
        showNasOfficePanelFor(officeDoc, "无法打开，请稍后重试", false);
        renderOfficeDock();
        pumpOfficeDownload();
        return;
      }
      openNasOfficeFromPath(officeDoc, payload.localPath);
      pumpOfficeDownload();
      return;
    }
    if (!drive.nasPreviewName) return;
    if (name && name !== drive.nasPreviewName) return;
    if (drive.nasPreviewPath && dir !== drive.nasPreviewPath) return;
    if (isNasStreamKind(drive.nasPreviewKind)) {
      nasStreamCtl.complete = true;
      if (payload.ok && payload.localPath) {
        openNasPreviewStream(drive, payload.localPath, true);
      } else if (!nasStreamCtl.playing) {
        showNasPreview(drive, name || drive.nasPreviewName, "", "无法预览，请稍后重试", false);
      }
      return;
    }
    if (!payload.ok || !payload.localPath) {
      showNasPreview(drive, name || drive.nasPreviewName, "", "无法打开，请稍后重试", false);
      return;
    }
    if (isNasTextKind(drive.nasPreviewKind)) {
      openNasTextFromPath(drive, payload.localPath);
      return;
    }
    if (isNasOfficeKind(drive.nasPreviewKind)) {
      const fallback = findOfficeDoc(sid, dir, name) || focusedOfficeDoc();
      if (fallback) openNasOfficeFromPath(fallback, payload.localPath);
      return;
    }
    openNasPreviewImage(drive, name || drive.nasPreviewName, payload.localPath);
  }

  function nasPreviewSrc(localPath) {
    const src = convertFileSrc(localPath);
    if (!src) return "";
    return src + (src.indexOf("?") >= 0 ? "&" : "?") + "t=" + Date.now();
  }

  function openNasPreviewImage(session, name, localPath) {
    const src = nasPreviewSrc(localPath);
    if (!src) {
      showNasPreview(session, name, "", "无法预览，请稍后重试", false);
      return;
    }
    showNasPreview(session, name, src, "", false, "image");
  }

  function openNasPreviewStream(session, localPath, complete) {
    const kind = (session && session.nasPreviewKind) || "video";
    const el = nasStreamEl(kind);
    if (!el) {
      showNasPreview(session, session && session.nasPreviewName, "", "无法预览，请稍后重试", false);
      return;
    }
    const src = nasPreviewSrc(localPath);
    if (!src) {
      showNasPreview(session, session && session.nasPreviewName, "", "无法预览，请稍后重试", false);
      return;
    }
    nasStreamCtl.path = localPath;
    nasStreamCtl.complete = !!complete;
    const resumeAt = el.currentTime || 0;
    if (!complete && nasStreamCtl.attached) return;
    nasStreamCtl.attached = true;
    el.hidden = true;
    el.src = src;
    el.load();
    function onReady() {
      el.removeEventListener("canplay", onReady);
      nasStreamCtl.playing = true;
      nasStreamCtl.attached = true;
      el.hidden = false;
      if (resumeAt > 0.4) {
        try {
          el.currentTime = resumeAt;
        } catch (_) {}
      }
      el.play().catch(function () {});
      setNasPreviewLoading(false, "");
    }
    el.addEventListener("canplay", onReady, { once: true });
    if (complete) {
      window.setTimeout(function () {
        if (nasStreamCtl.playing) return;
        if (!nasPreview || nasPreview.hidden) return;
        if (!session || !isNasStreamKind(session.nasPreviewKind)) return;
        showNasPreview(session, session.nasPreviewName, "", nasStreamFailText(session.nasPreviewKind), false);
      }, 8000);
    }
  }

  function startNasStreamPump(session, seq, localPath) {
    stopNasStreamPump();
    nasStreamCtl.seq = seq;
    nasStreamCtl.path = localPath;
    nasStreamCtl.lastSize = 0;
    nasStreamCtl.playing = false;
    nasStreamCtl.complete = false;
    nasStreamCtl.attached = false;
    function tick() {
      if (!session || session.nasPreviewSeq !== seq || !nasPreview || nasPreview.hidden) return;
      if (nasStreamCtl.playing && nasStreamCtl.complete) return;
      tauriInvoke("file_stat", { path: localPath })
        .then(function (stat) {
          if (!session || session.nasPreviewSeq !== seq) return;
          const size = Number(stat && stat.size) || 0;
          const grew = size > nasStreamCtl.lastSize;
          nasStreamCtl.lastSize = size;
          const enough = size >= NAS_VIDEO_MIN_BYTES || (nasStreamCtl.complete && size > 0);
          if (enough && grew && !nasStreamCtl.playing) {
            openNasPreviewStream(session, localPath, nasStreamCtl.complete);
          }
        })
        .catch(function () {});
      nasStreamCtl.timer = window.setTimeout(tick, 800);
    }
    tick();
    nasStreamCtl.hintTimer = window.setTimeout(function () {
      if (!session || session.nasPreviewSeq !== seq) return;
      if (nasStreamCtl.playing) return;
      if (!nasPreview || nasPreview.hidden) return;
      setNasPreviewLoading(true, "等待下载完成后再打开");
    }, NAS_VIDEO_HINT_MS);
  }

  function stopNasStreamPump() {
    if (nasStreamCtl.timer) {
      window.clearTimeout(nasStreamCtl.timer);
      nasStreamCtl.timer = 0;
    }
    if (nasStreamCtl.hintTimer) {
      window.clearTimeout(nasStreamCtl.hintTimer);
      nasStreamCtl.hintTimer = 0;
    }
    nasStreamCtl.playing = false;
    nasStreamCtl.complete = false;
    nasStreamCtl.attached = false;
    nasStreamCtl.path = "";
  }

  function setNasPreviewLoading(loading, status) {
    if (nasPreviewSpinner) nasPreviewSpinner.hidden = !loading;
    if (nasPreviewStatus) {
      if (status) {
        nasPreviewStatus.hidden = false;
        nasPreviewStatus.textContent = status;
      } else {
        nasPreviewStatus.hidden = true;
        nasPreviewStatus.textContent = "";
      }
    }
    if (nasPreviewLoading) nasPreviewLoading.hidden = !(loading || status);
  }

  function showNasPreview(session, name, src, status, loading, mediaKind) {
    if (!nasPreview) return;
    closeMediaOverlay();
    const kind = mediaKind || (session && session.nasPreviewKind) || "image";
    if (nasPreviewImage) {
      if (kind === "image" && src) {
        nasPreviewImage.hidden = false;
        nasPreviewImage.alt = name || "";
        nasPreviewImage.src = src;
      } else {
        nasPreviewImage.hidden = true;
        nasPreviewImage.removeAttribute("src");
        nasPreviewImage.alt = "";
      }
    }
    if (nasPreviewVideo) {
      if (kind !== "video") {
        nasPreviewVideo.pause();
        nasPreviewVideo.removeAttribute("src");
        nasPreviewVideo.load();
        nasPreviewVideo.hidden = true;
      } else if (src) {
        nasPreviewVideo.hidden = false;
        nasPreviewVideo.src = src;
      }
    }
    if (nasPreviewAudio) {
      if (kind !== "audio") {
        nasPreviewAudio.pause();
        nasPreviewAudio.removeAttribute("src");
        nasPreviewAudio.load();
        nasPreviewAudio.hidden = true;
      } else if (src) {
        nasPreviewAudio.hidden = false;
        nasPreviewAudio.src = src;
      }
    }
    setNasPreviewLoading(!!loading, status || "");
    nasPreview.hidden = false;
  }

  function closeNasPreview() {
    if (!nasPreview) return;
    nasPreview.hidden = true;
    stopNasStreamPump();
    if (nasPreviewImage) {
      nasPreviewImage.removeAttribute("src");
      nasPreviewImage.alt = "";
      nasPreviewImage.hidden = true;
    }
    if (nasPreviewVideo) {
      nasPreviewVideo.pause();
      nasPreviewVideo.removeAttribute("src");
      nasPreviewVideo.load();
      nasPreviewVideo.hidden = true;
    }
    if (nasPreviewAudio) {
      nasPreviewAudio.pause();
      nasPreviewAudio.removeAttribute("src");
      nasPreviewAudio.load();
      nasPreviewAudio.hidden = true;
    }
    setNasPreviewLoading(false, "");
    const session = selectedDrive();
    if (session) {
      session.nasPreviewSeq = (session.nasPreviewSeq || 0) + 1;
      session.nasPreviewName = "";
      session.nasPreviewKind = "";
      session.nasPreviewPath = "";
    }
  }

  function bindNasEditor() {
    if (nasEditorSave) {
      nasEditorSave.addEventListener("click", function () {
        saveNasEditor(false);
      });
    }
    if (nasEditorExit) {
      nasEditorExit.addEventListener("click", requestNasEditorExit);
    }
    if (nasEditorAskCancel) {
      nasEditorAskCancel.addEventListener("click", hideNasEditorAsk);
    }
    if (nasEditorAskDiscard) {
      nasEditorAskDiscard.addEventListener("click", function () {
        hideNasEditorAsk();
        forceCloseNasEditor();
      });
    }
    if (nasEditorAskSave) {
      nasEditorAskSave.addEventListener("click", function () {
        hideNasEditorAsk();
        saveNasEditor(true);
      });
    }
  }

  function bindNasOffice() {
    if (nasOfficeSync) {
      nasOfficeSync.addEventListener("click", function () {
        const doc = focusedOfficeDoc();
        if (doc) queueOfficeSync(doc, false, true);
      });
    }
    if (nasOfficeExit) {
      nasOfficeExit.addEventListener("click", requestNasOfficeExit);
    }
    if (nasOfficeAskCancel) {
      nasOfficeAskCancel.addEventListener("click", hideNasOfficeAsk);
    }
    if (nasOfficeAskDiscard) {
      nasOfficeAskDiscard.addEventListener("click", function () {
        const doc = findOfficeDocById(nasOfficeAskForId) || focusedOfficeDoc();
        hideNasOfficeAsk();
        if (doc) removeOfficeDoc(doc);
      });
    }
    if (nasOfficeAskSync) {
      nasOfficeAskSync.addEventListener("click", function () {
        const doc = findOfficeDocById(nasOfficeAskForId) || focusedOfficeDoc();
        hideNasOfficeAsk();
        if (doc) queueOfficeSync(doc, true, true);
      });
    }
    if (nasOfficeDockList) {
      nasOfficeDockList.addEventListener("click", function (event) {
        const closeBtn = event.target.closest(".nas-office-chip-close");
        const chip = event.target.closest(".nas-office-chip");
        const id = Number((closeBtn || chip) && (closeBtn || chip).getAttribute("data-id")) || 0;
        const doc = findOfficeDocById(id);
        if (!doc) return;
        if (closeBtn) {
          event.preventDefault();
          event.stopPropagation();
          requestRemoveOfficeDoc(doc);
          return;
        }
        reopenOfficeDoc(doc);
      });
    }
  }

  function findOfficeDocById(id) {
    const nid = Number(id) || 0;
    if (!nid) return null;
    return nasOfficeDocs.find(function (item) {
      return item.id === nid;
    }) || null;
  }

  function findOfficeDoc(sessionId, path, name) {
    const sid = Number(sessionId) || 0;
    const dir = normalizeNasPath(path || "/");
    const fileName = String(name || "");
    if (!sid || !fileName) return null;
    return nasOfficeDocs.find(function (item) {
      return item.sessionId === sid && item.path === dir && item.name === fileName;
    }) || null;
  }

  function focusedOfficeDoc() {
    return findOfficeDocById(nasOfficeFocusId);
  }

  function officeDocDirty(doc) {
    if (!doc || !doc.opened) return false;
    return Number(doc.lastSize) !== Number(doc.origSize) || Number(doc.lastMtime) !== Number(doc.origMtime);
  }

  function officeSessionBusy(sessionId, exceptId) {
    const sid = Number(sessionId) || 0;
    if (!sid) return true;
    if (driveHasRunningTask(findDriveByRpc(sid))) return true;
    return nasOfficeDocs.some(function (item) {
      if (item.id === exceptId) return false;
      if (item.sessionId !== sid) return false;
      return !!(item.saving || (item.pending && item.downloadStarted));
    });
  }

  function createOfficeDoc(init) {
    const doc = {
      id: nasOfficeNextId++,
      sessionId: Number(init && init.sessionId) || 0,
      name: (init && init.name) || "",
      path: normalizeNasPath((init && init.path) || "/"),
      localPath: "",
      origSize: 0,
      origMtime: 0,
      lastSize: 0,
      lastMtime: 0,
      syncSize: 0,
      syncMtime: 0,
      changedAt: 0,
      saving: false,
      queued: false,
      pending: !!(init && init.pending),
      downloadStarted: false,
      closeAfterSave: false,
      opened: false,
      noApp: false,
      saveTimer: 0,
      openTimer: 0,
      hint: "",
    };
    nasOfficeDocs.push(doc);
    ensureOfficeWatch();
    renderOfficeDock();
    return doc;
  }

  function clearOfficeDocTimer(doc, key) {
    if (!doc || !doc[key]) return;
    window.clearTimeout(doc[key]);
    doc[key] = 0;
  }

  function hideNasOfficeAsk() {
    nasOfficeAskForId = 0;
    if (nasOfficeAsk) nasOfficeAsk.hidden = true;
  }

  function showNasOfficeAsk(doc) {
    nasOfficeAskForId = doc && doc.id ? doc.id : 0;
    if (nasOfficeAsk) nasOfficeAsk.hidden = false;
  }

  function setNasOfficeHint(text) {
    if (nasOfficeHint) nasOfficeHint.textContent = text || "";
  }

  function setNasOfficeBusy(busy) {
    if (nasOfficeSync) nasOfficeSync.disabled = !!busy;
  }

  function setNasOfficeLoading(loading, status) {
    if (nasOfficeLoading) nasOfficeLoading.hidden = !loading;
    if (nasOfficeGuide) nasOfficeGuide.hidden = !!loading;
    if (nasOfficeStatus && status) nasOfficeStatus.textContent = status;
    if (!loading && status) setNasOfficeHint(status);
  }

  function setOfficeDocHint(doc, text) {
    if (!doc) return;
    doc.hint = text || "";
    if (focusedOfficeDoc() === doc) setNasOfficeHint(doc.hint);
  }

  function officeDocStatusText(doc) {
    if (!doc) return "";
    if (doc.pending && !doc.localPath) return "下载中";
    if (doc.saving || doc.queued) return "同步中";
    if (doc.noApp) return "未安装办公软件";
    if (officeDocDirty(doc)) return "有未同步";
    if (doc.opened) return "已同步";
    if (doc.hint) return doc.hint;
    return "打开中";
  }

  function renderOfficeDock() {
    const hasDocs = nasOfficeDocs.length > 0;
    document.body.classList.toggle("has-office-dock", hasDocs);
    if (nasOfficeDock) nasOfficeDock.hidden = !hasDocs;
    if (!nasOfficeDockList) return;
    if (!hasDocs) {
      nasOfficeDockList.innerHTML = "";
      return;
    }
    nasOfficeDockList.innerHTML = nasOfficeDocs
      .map(function (doc) {
        const dirty = officeDocDirty(doc);
        const syncing = !!(doc.saving || doc.queued);
        const cls =
          "nas-office-chip" +
          (doc.id === nasOfficeFocusId && nasOfficeEl && !nasOfficeEl.hidden ? " is-active" : "") +
          (syncing ? " is-syncing" : dirty ? " is-dirty" : "");
        return (
          '<div class="' +
          cls +
          '" data-id="' +
          doc.id +
          '"><div class="nas-office-chip-main"><span class="nas-office-chip-name">' +
          escapeHtml(doc.name || "文稿") +
          '</span><span class="nas-office-chip-status">' +
          escapeHtml(officeDocStatusText(doc)) +
          '</span></div><button type="button" class="nas-office-chip-close" data-id="' +
          doc.id +
          '" aria-label="结束编辑">×</button></div>'
        );
      })
      .join("");
  }

  function refreshOfficeOverlay(doc) {
    if (!doc || focusedOfficeDoc() !== doc || !nasOfficeEl || nasOfficeEl.hidden) {
      renderOfficeDock();
      return;
    }
    if (nasOfficeTitle) nasOfficeTitle.textContent = doc.name || "文稿";
    if (nasOfficePath) {
      const dir = doc.path === "/" ? "" : doc.path;
      nasOfficePath.textContent = dir + "/" + (doc.name || "");
    }
    setNasOfficeBusy(!!doc.saving);
    if (doc.pending && !doc.localPath) {
      setNasOfficeLoading(true, doc.hint || "正在下载文件…");
    } else if (doc.saving) {
      setNasOfficeLoading(true, "正在同步到网盘…");
    } else {
      setNasOfficeLoading(false, "");
      setNasOfficeHint(doc.hint || (officeDocDirty(doc) ? "已保存到本地，正在等待自动同步" : "已同步"));
      if (nasOfficeGuide) {
        nasOfficeGuide.hidden = false;
        nasOfficeGuide.textContent = doc.noApp
          ? "未检测到 WPS、Office 或 LibreOffice，请先安装办公软件后再打开。"
          : "已使用系统软件打开。请在办公软件中保存，保存后会自动同步到网盘。";
      }
    }
    renderOfficeDock();
  }

  function hideNasOfficeOverlay() {
    hideNasOfficeAsk();
    if (nasOfficeEl) nasOfficeEl.hidden = true;
    setNasOfficeHint("");
    setNasOfficeBusy(false);
    renderOfficeDock();
  }

  function focusOfficeDoc(doc, showOverlay) {
    if (!doc) return;
    nasOfficeFocusId = doc.id;
    if (showOverlay) showNasOfficePanelFor(doc, doc.hint || "", !!(doc.pending && !doc.localPath));
    else renderOfficeDock();
  }

  function showNasOfficePanelFor(doc, status, loading) {
    if (!nasOfficeEl || !doc) return;
    if (nasPreview) nasPreview.hidden = true;
    stopNasStreamPump();
    nasOfficeFocusId = doc.id;
    if (nasOfficeTitle) nasOfficeTitle.textContent = doc.name || "文稿";
    if (nasOfficePath) {
      const dir = doc.path === "/" ? "" : doc.path;
      nasOfficePath.textContent = dir + "/" + (doc.name || "");
    }
    hideNasOfficeAsk();
    setNasOfficeBusy(!!doc.saving);
    setNasOfficeLoading(!!loading, status || doc.hint || "正在加载文件…");
    if (!loading) setOfficeDocHint(doc, status || doc.hint);
    nasOfficeEl.hidden = false;
    renderOfficeDock();
  }

  function removeOfficeDoc(doc) {
    if (!doc) return;
    clearOfficeDocTimer(doc, "saveTimer");
    clearOfficeDocTimer(doc, "openTimer");
    nasOfficeDocs = nasOfficeDocs.filter(function (item) {
      return item.id !== doc.id;
    });
    if (nasOfficeFocusId === doc.id) {
      nasOfficeFocusId = nasOfficeDocs.length ? nasOfficeDocs[nasOfficeDocs.length - 1].id : 0;
      hideNasOfficeOverlay();
    }
    if (!nasOfficeDocs.length) stopOfficeWatch();
    renderOfficeDock();
  }

  function closeOfficeDocsForSession(sessionId) {
    const sid = Number(sessionId) || 0;
    nasOfficeDocs.filter(function (item) {
      return item.sessionId === sid;
    }).forEach(removeOfficeDoc);
  }

  function closeAllOfficeDocs() {
    nasOfficeDocs.slice().forEach(removeOfficeDoc);
    hideNasOfficeOverlay();
    stopOfficeWatch();
  }

  function forceCloseNasOffice() {
    closeAllOfficeDocs();
  }

  function requestNasOfficeExit() {
    hideNasOfficeOverlay();
  }

  function requestRemoveOfficeDoc(doc) {
    if (!doc) return;
    if (doc.saving) {
      focusOfficeDoc(doc, true);
      setOfficeDocHint(doc, "正在同步，请稍候再关闭");
      return;
    }
    if (doc.opened && officeDocDirty(doc)) {
      focusOfficeDoc(doc, true);
      showNasOfficeAsk(doc);
      return;
    }
    removeOfficeDoc(doc);
  }

  function applyOfficeDocStat(doc, stat, asOriginal) {
    if (!doc) return;
    const size = Number(stat && stat.size) || 0;
    const mtime = Number(stat && stat.modifiedMs) || 0;
    if (size !== Number(doc.lastSize) || mtime !== Number(doc.lastMtime)) {
      doc.changedAt = Date.now();
    }
    doc.lastSize = size;
    doc.lastMtime = mtime;
    if (asOriginal) {
      doc.origSize = size;
      doc.origMtime = mtime;
      doc.changedAt = Date.now();
    }
  }

  function ensureOfficeWatch() {
    if (nasOfficeWatchTimer) return;
    function tick() {
      if (!nasOfficeDocs.length) {
        nasOfficeWatchTimer = 0;
        return;
      }
      nasOfficeDocs.forEach(pollOfficeDoc);
      pumpOfficeDownload();
      pumpOfficeSync();
      nasOfficeWatchTimer = window.setTimeout(tick, 1500);
    }
    tick();
  }

  function stopOfficeWatch() {
    if (!nasOfficeWatchTimer) return;
    window.clearTimeout(nasOfficeWatchTimer);
    nasOfficeWatchTimer = 0;
  }

  function pollOfficeDoc(doc) {
    if (!doc || !doc.localPath || doc.noApp) return;
    tauriInvoke("file_stat", { path: doc.localPath })
      .then(function (stat) {
        if (!findOfficeDocById(doc.id)) return;
        applyOfficeDocStat(doc, stat, false);
        if (doc.saving || doc.noApp) return;
        if (officeDocDirty(doc)) {
          const stable = doc.changedAt && Date.now() - doc.changedAt >= NAS_OFFICE_STABLE_MS;
          setOfficeDocHint(doc, stable ? "已保存到本地，正在自动同步" : "已保存到本地，等待写入完成");
          if (stable) queueOfficeSync(doc, false, false);
        } else if (!doc.queued) {
          setOfficeDocHint(doc, "已同步");
        }
        refreshOfficeOverlay(doc);
      })
      .catch(function () {});
  }

  function pumpOfficeDownload() {
    nasOfficeDocs.forEach(function (doc) {
      if (!doc.pending || doc.localPath || doc.downloadStarted) return;
      if (officeSessionBusy(doc.sessionId, doc.id)) return;
      startOfficeDownload(doc);
    });
  }

  function startOfficeDownload(doc) {
    if (!doc || doc.downloadStarted || doc.localPath) return;
    const drive = findDriveByRpc(doc.sessionId);
    if (!drive) {
      setOfficeDocHint(doc, "网盘未连接");
      refreshOfficeOverlay(doc);
      return;
    }
    doc.downloadStarted = true;
    clearOfficeDocTimer(doc, "openTimer");
    doc.openTimer = window.setTimeout(function () {
      if (!findOfficeDocById(doc.id) || doc.opened || doc.localPath) return;
      doc.downloadStarted = false;
      setOfficeDocHint(doc, "打开超时");
      showNasOfficePanelFor(doc, "打开超时", false);
      renderOfficeDock();
    }, 90000);
    tauriInvoke("nas_get_file", {
      sessionId: doc.sessionId,
      fileName: doc.name,
      filePath: doc.path,
      size: null,
    })
      .then(function (result) {
        if (!findOfficeDocById(doc.id)) return;
        if (result && result.busy) {
          doc.downloadStarted = false;
          setOfficeDocHint(doc, "有文件正在传输，请稍候");
          refreshOfficeOverlay(doc);
          return;
        }
        if (result && result.cached && result.localPath) {
          openNasOfficeFromPath(doc, result.localPath);
        }
      })
      .catch(function () {
        if (!findOfficeDocById(doc.id)) return;
        doc.downloadStarted = false;
        doc.pending = false;
        setOfficeDocHint(doc, "无法打开，请稍后重试");
        refreshOfficeOverlay(doc);
      });
  }

  function reopenOfficeDoc(doc) {
    if (!doc) return;
    focusOfficeDoc(doc, true);
    if (doc.opened && doc.localPath) {
      tauriInvoke("office_open_file", { path: doc.localPath }).catch(function () {});
      refreshOfficeOverlay(doc);
      return;
    }
    if (doc.localPath) {
      openNasOfficeFromPath(doc, doc.localPath);
      return;
    }
    showNasOfficePanelFor(doc, doc.hint || "正在下载文件…", !!doc.pending);
  }

  function openNasOfficeFromPath(doc, localPath) {
    if (!doc || !localPath) {
      setNasOfficeLoading(false, "无法打开，请稍后重试");
      return;
    }
    clearOfficeDocTimer(doc, "openTimer");
    doc.localPath = localPath;
    doc.pending = false;
    doc.downloadStarted = false;
    if (focusedOfficeDoc() === doc) {
      showNasOfficePanelFor(doc, "正在打开文件…", true);
    }
    tauriInvoke("file_stat", { path: localPath })
      .then(function (stat) {
        applyOfficeDocStat(doc, stat, true);
        return tauriInvoke("office_open_file", { path: localPath });
      })
      .then(function () {
        if (!findOfficeDocById(doc.id)) return;
        doc.opened = true;
        doc.noApp = false;
        setOfficeDocHint(doc, "已同步");
        ensureOfficeWatch();
        refreshOfficeOverlay(doc);
        pumpOfficeDownload();
      })
      .catch(function (err) {
        const msg = String((err && err.message) || err || "");
        doc.opened = false;
        if (msg.indexOf("no-office-app") >= 0) {
          doc.noApp = true;
          setOfficeDocHint(doc, "请安装办公软件");
          refreshOfficeOverlay(doc);
          return;
        }
        setOfficeDocHint(doc, "无法打开，请稍后重试");
        refreshOfficeOverlay(doc);
      });
  }

  function queueOfficeSync(doc, thenClose, immediate) {
    if (!doc) return;
    hideNasOfficeAsk();
    if (doc.noApp) {
      setOfficeDocHint(doc, "请安装办公软件");
      if (thenClose) removeOfficeDoc(doc);
      return;
    }
    if (!doc.opened) {
      setOfficeDocHint(doc, "文件还在加载");
      refreshOfficeOverlay(doc);
      return;
    }
    if (!officeDocDirty(doc)) {
      setOfficeDocHint(doc, "没有修改");
      if (thenClose) removeOfficeDoc(doc);
      else refreshOfficeOverlay(doc);
      return;
    }
    doc.closeAfterSave = !!thenClose || doc.closeAfterSave;
    doc.queued = true;
    if (immediate) doc.changedAt = Date.now() - NAS_OFFICE_STABLE_MS;
    refreshOfficeOverlay(doc);
    pumpOfficeSync();
  }

  function pumpOfficeSync() {
    const doc = nasOfficeDocs.find(function (item) {
      if (!item.queued || item.saving || !item.opened || item.noApp) return false;
      if (!officeDocDirty(item) && !item.closeAfterSave) {
        item.queued = false;
        return false;
      }
      return !officeSessionBusy(item.sessionId, item.id);
    });
    if (!doc) return;
    startOfficeSync(doc);
  }

  function startOfficeSync(doc) {
    if (!doc || doc.saving) return;
    if (!doc.sessionId || !doc.localPath || !doc.name) {
      doc.queued = false;
      setOfficeDocHint(doc, "同步失败");
      refreshOfficeOverlay(doc);
      return;
    }
    doc.saving = true;
    doc.queued = false;
    doc.syncSize = doc.lastSize;
    doc.syncMtime = doc.lastMtime;
    clearOfficeDocTimer(doc, "saveTimer");
    doc.saveTimer = window.setTimeout(function () {
      if (!findOfficeDocById(doc.id) || !doc.saving) return;
      doc.saving = false;
      doc.queued = true;
      setOfficeDocHint(doc, "同步超时，将重试");
      refreshOfficeOverlay(doc);
      pumpOfficeSync();
    }, 120000);
    setOfficeDocHint(doc, "正在同步到网盘…");
    refreshOfficeOverlay(doc);
    tauriInvoke("office_snapshot_file", { path: doc.localPath })
      .then(function (snapPath) {
        if (!snapPath) throw new Error("snapshot-empty");
        return tauriInvoke("nas_put_file", {
          sessionId: doc.sessionId,
          fileName: doc.name,
          filePath: doc.path,
          localPath: snapPath,
        });
      })
      .catch(function () {
        if (!findOfficeDocById(doc.id)) return;
        clearOfficeDocTimer(doc, "saveTimer");
        doc.saving = false;
        doc.queued = true;
        setOfficeDocHint(doc, "文件正在写入，稍后自动同步");
        refreshOfficeOverlay(doc);
        window.setTimeout(pumpOfficeSync, 4000);
      });
  }

  function onNasOfficePut(payload) {
    const sid = Number(payload.sessionId) || 0;
    const name = String(payload.fileName || "");
    const dir = normalizeNasPath(payload.filePath || "/");
    const doc = nasOfficeDocs.find(function (item) {
      return item.saving && item.sessionId === sid && item.name === name && item.path === dir;
    });
    if (!doc) return false;
    clearOfficeDocTimer(doc, "saveTimer");
    doc.saving = false;
    if (!payload.ok) {
      doc.queued = true;
      setOfficeDocHint(doc, "网盘忙，稍后自动同步");
      refreshOfficeOverlay(doc);
      window.setTimeout(pumpOfficeSync, 2500);
      return true;
    }
    doc.origSize = doc.syncSize;
    doc.origMtime = doc.syncMtime;
    const drive = findDriveByRpc(doc.sessionId);
    if (drive && normalizeNasPath(drive.nasPath || "/") === dir) {
      requestNasPath(drive, drive.nasPath);
    }
    const finish = function () {
      if (!findOfficeDocById(doc.id)) return;
      if (officeDocDirty(doc)) {
        queueOfficeSync(doc, doc.closeAfterSave, false);
        return;
      }
      setOfficeDocHint(doc, "已同步");
      if (doc.closeAfterSave) {
        doc.closeAfterSave = false;
        removeOfficeDoc(doc);
        return;
      }
      refreshOfficeOverlay(doc);
      pumpOfficeSync();
      pumpOfficeDownload();
    };
    if (!doc.localPath) {
      finish();
      return true;
    }
    tauriInvoke("file_stat", { path: doc.localPath })
      .then(function (stat) {
        applyOfficeDocStat(doc, stat, false);
        finish();
      })
      .catch(function () {
        finish();
      });
    return true;
  }

  function flushOfficeDocsThen(done) {
    nasOfficeDocs.forEach(function (doc) {
      if (doc.opened && !doc.noApp && officeDocDirty(doc)) doc.queued = true;
    });
    pumpOfficeSync();
    let n = 0;
    function tick() {
      n += 1;
      const busy = nasOfficeDocs.some(function (doc) {
        return doc.saving;
      });
      const queued = nasOfficeDocs.some(function (doc) {
        return doc.queued && officeDocDirty(doc) && doc.opened;
      });
      if ((!busy && !queued) || n >= 16) {
        if (typeof done === "function") done();
        return;
      }
      if (!busy) pumpOfficeSync();
      window.setTimeout(tick, 500);
    }
    tick();
  }

  function nasEditorDirty() {
    return !!(nasEditorBody && nasEditorBody.value !== nasEditorCtl.original);
  }

  function setNasEditorHint(text) {
    if (!nasEditorHint) return;
    nasEditorHint.textContent = text || "";
  }

  function setNasEditorBusy(busy) {
    if (nasEditorSave) nasEditorSave.disabled = !!busy;
    if (nasEditorExit) nasEditorExit.disabled = !!busy;
    if (nasEditorBody) nasEditorBody.readOnly = !!busy;
  }

  function hideNasEditorAsk() {
    if (nasEditorAsk) nasEditorAsk.hidden = true;
  }

  function showNasEditorAsk() {
    if (nasEditorAsk) nasEditorAsk.hidden = false;
  }

  function clearNasEditorSaveTimer() {
    if (nasEditorCtl.saveTimer) {
      window.clearTimeout(nasEditorCtl.saveTimer);
      nasEditorCtl.saveTimer = 0;
    }
  }

  function resetNasEditorCtl() {
    clearNasEditorSaveTimer();
    nasEditorCtl.sessionId = 0;
    nasEditorCtl.name = "";
    nasEditorCtl.path = "";
    nasEditorCtl.localPath = "";
    nasEditorCtl.original = "";
    nasEditorCtl.saveText = "";
    nasEditorCtl.saving = false;
    nasEditorCtl.closeAfterSave = false;
  }

  function forceCloseNasEditor() {
    hideNasEditorAsk();
    if (nasEditorEl) nasEditorEl.hidden = true;
    if (nasEditorBody) nasEditorBody.value = "";
    setNasEditorHint("");
    setNasEditorBusy(false);
    resetNasEditorCtl();
  }

  function requestNasEditorExit() {
    if (!nasEditorEl || nasEditorEl.hidden) return;
    if (nasEditorCtl.saving) return;
    hideNasEditorAsk();
    if (!nasEditorDirty()) {
      forceCloseNasEditor();
      return;
    }
    showNasEditorAsk();
  }

  function openNasTextFromPath(session, localPath) {
    if (!session || !localPath) {
      showNasPreview(session, session && session.nasPreviewName, "", "无法打开，请稍后重试", false);
      return;
    }
    tauriInvoke("nas_read_text", { path: localPath })
      .then(function (text) {
        openNasEditor(session, localPath, text == null ? "" : String(text));
      })
      .catch(function (err) {
        const msg = String((err && err.message) || err || "");
        if (msg.indexOf("too-large") >= 0) {
          showNasPreview(session, session.nasPreviewName, "", "文件过大无法在线编辑", false);
          return;
        }
        if (msg.indexOf("not-text") >= 0) {
          showNasPreview(session, session.nasPreviewName, "", "该文件不是可编辑的文本", false);
          return;
        }
        showNasPreview(session, session.nasPreviewName, "", "无法打开，请稍后重试", false);
      });
  }

  function openNasEditor(session, localPath, text) {
    if (!nasEditorEl || !nasEditorBody || !session) return;
    const name = session.nasPreviewName || "";
    const path = normalizeNasPath(session.nasPreviewPath || session.nasPath || "/");
    closeNasPreview();
    nasEditorCtl.sessionId = session.rpcSessionId || 0;
    nasEditorCtl.name = name;
    nasEditorCtl.path = path;
    nasEditorCtl.localPath = localPath;
    nasEditorCtl.saving = false;
    nasEditorCtl.closeAfterSave = false;
    nasEditorCtl.saveText = "";
    if (nasEditorTitle) nasEditorTitle.textContent = nasEditorCtl.name || "文本";
    if (nasEditorPath) {
      const dir = nasEditorCtl.path === "/" ? "" : nasEditorCtl.path;
      nasEditorPath.textContent = dir + "/" + (nasEditorCtl.name || "");
    }
    nasEditorBody.value = text;
    nasEditorCtl.original = nasEditorBody.value;
    hideNasEditorAsk();
    setNasEditorHint("");
    setNasEditorBusy(false);
    nasEditorEl.hidden = false;
    nasEditorBody.focus();
  }

  function saveNasEditor(thenClose) {
    if (!nasEditorEl || nasEditorEl.hidden || nasEditorCtl.saving) return;
    hideNasEditorAsk();
    if (!nasEditorDirty()) {
      setNasEditorHint("没有修改");
      if (thenClose) forceCloseNasEditor();
      return;
    }
    if (!nasEditorCtl.sessionId || !nasEditorCtl.localPath || !nasEditorCtl.name) {
      setNasEditorHint("保存失败");
      return;
    }
    if (driveHasRunningTask(findDriveByRpc(nasEditorCtl.sessionId))) {
      setNasEditorHint("有传输任务进行中，请稍候再保存");
      return;
    }
    const text = nasEditorBody ? nasEditorBody.value : "";
    nasEditorCtl.saving = true;
    nasEditorCtl.closeAfterSave = !!thenClose;
    nasEditorCtl.saveText = text;
    setNasEditorBusy(true);
    setNasEditorHint("正在保存…");
    clearNasEditorSaveTimer();
    nasEditorCtl.saveTimer = window.setTimeout(function () {
      if (!nasEditorCtl.saving) return;
      nasEditorCtl.saving = false;
      nasEditorCtl.closeAfterSave = false;
      setNasEditorBusy(false);
      setNasEditorHint("保存超时");
    }, 120000);
    tauriInvoke("nas_write_text", { path: nasEditorCtl.localPath, text: text })
      .then(function () {
        return tauriInvoke("nas_put_file", {
          sessionId: nasEditorCtl.sessionId,
          fileName: nasEditorCtl.name,
          filePath: nasEditorCtl.path,
          localPath: nasEditorCtl.localPath,
        });
      })
      .catch(function () {
        clearNasEditorSaveTimer();
        nasEditorCtl.saving = false;
        nasEditorCtl.closeAfterSave = false;
        setNasEditorBusy(false);
        setNasEditorHint("保存失败");
      });
  }

  function onNasPut(payload) {
    if (onNasOfficePut(payload)) return;
    const sid = Number(payload.sessionId) || 0;
    const name = String(payload.fileName || "");
    const dir = normalizeNasPath(payload.filePath || "/");
    if (!sid || sid !== nasEditorCtl.sessionId) return;
    if (name && nasEditorCtl.name && name !== nasEditorCtl.name) return;
    if (dir !== nasEditorCtl.path) return;
    clearNasEditorSaveTimer();
    nasEditorCtl.saving = false;
    setNasEditorBusy(false);
    if (!payload.ok) {
      nasEditorCtl.closeAfterSave = false;
      setNasEditorHint("保存失败");
      return;
    }
    nasEditorCtl.original = nasEditorCtl.saveText;
    const stillDirty = nasEditorDirty();
    setNasEditorHint(stillDirty ? "已保存，还有未提交的修改" : "已保存");
    const drive = state.drives.find(function (item) {
      return item.rpcSessionId === sid;
    });
    if (drive && normalizeNasPath(drive.nasPath || "/") === dir) {
      requestNasPath(drive, drive.nasPath);
    }
    if (nasEditorCtl.closeAfterSave) {
      nasEditorCtl.closeAfterSave = false;
      if (stillDirty) showNasEditorAsk();
      else forceCloseNasEditor();
    }
  }

  function nasKindMeta(kind) {
    if (kind === "folder") return { label: "文件夹", cls: "is-folder" };
    if (kind === "image") return { label: "图像", cls: "is-image" };
    if (kind === "video") return { label: "影片", cls: "is-video" };
    if (kind === "audio") return { label: "音频", cls: "is-audio" };
    if (kind === "pdf") return { label: "PDF 文稿", cls: "is-pdf" };
    if (kind === "archive") return { label: "压缩包", cls: "is-archive" };
    if (kind === "doc") return { label: "文稿", cls: "is-doc" };
    if (kind === "text") return { label: "文本", cls: "is-text" };
    if (kind === "code") return { label: "代码", cls: "is-code" };
    return { label: "文稿", cls: "is-file" };
  }

  function renderNasFiles(session) {
    if (!nasFileBody) return;
    if (session && session.nasLoading) {
      nasFileBody.innerHTML =
        '<div class="nas-empty">' +
        '<div class="nas-empty-icon" aria-hidden="true"></div>' +
        "<h3>正在加载</h3>" +
        "<p>正在获取当前目录的文件列表…</p>" +
        "</div>";
      return;
    }
    if (session && session.nasListError) {
      nasFileBody.innerHTML =
        '<div class="nas-empty">' +
        '<div class="nas-empty-icon" aria-hidden="true"></div>' +
        "<h3>无法加载</h3>" +
        "<p>" +
        escapeHtml(session.nasListError) +
        "</p>" +
        "</div>";
      return;
    }
    const list = session && Array.isArray(session.nasFiles) ? session.nasFiles : [];
    if (!list.length) {
      nasFileBody.innerHTML =
        '<div class="nas-empty">' +
        '<div class="nas-empty-icon" aria-hidden="true"></div>' +
        "<h3>此文件夹为空</h3>" +
        "<p>当前目录没有可见的文件或文件夹。</p>" +
        "</div>";
      pruneNasSelection(session);
      applyPendingNasSearchSelect(session);
      return;
    }
    nasFileBody.innerHTML = list
      .map(function (file) {
        const isDir = !!file.isDir;
        const kind = nasKindMeta(file.kind || classifyNasName(file.name, isDir));
        const name = file.name || "";
        const size = isDir || file.size == null || !isFinite(file.size) ? "—" : formatSize(file.size);
        const mtime = file.mtime ? formatDateTime(file.mtime) : "—";
        const attrs =
          ' data-nas-name="' +
          escapeHtml(name) +
          '" data-nas-dir="' +
          (isDir ? "1" : "0") +
          '"';
          const rowClass = isDir
            ? " is-dir"
            : kind.cls === "is-image"
              ? " is-image"
              : kind.cls === "is-video"
                ? " is-video"
                : kind.cls === "is-audio"
                  ? " is-audio"
                  : kind.cls === "is-text"
                    ? " is-text"
                    : kind.cls === "is-code"
                      ? " is-code"
                      : kind.cls === "is-doc"
                        ? " is-doc"
                        : kind.cls === "is-pdf"
                          ? " is-pdf"
                          : "";
        return (
          '<div class="nas-row' +
          rowClass +
          '"' +
          attrs +
          ">" +
          '<div class="nas-row-name"><span class="nas-icon ' +
          kind.cls +
          '"></span><span class="nas-name-text" title="' +
          escapeHtml(name) +
          '">' +
          escapeHtml(name) +
          "</span></div>" +
          "<span>" +
          escapeHtml(kind.label) +
          "</span><span>" +
          escapeHtml(size) +
          "</span><span>" +
          escapeHtml(mtime) +
          "</span></div>" +
          '<div class="nas-tile' +
          rowClass +
          '"' +
          attrs +
          '><span class="nas-icon ' +
          kind.cls +
          '"></span><span class="nas-tile-name nas-name-text" title="' +
          escapeHtml(name) +
          '">' +
          escapeHtml(name) +
          "</span></div>"
        );
      })
      .join("");
    pruneNasSelection(session);
    applyNasSelection();
    restoreNasRenameEditor();
    applyPendingNasSearchSelect(session);
    applyPendingNasZipSelect(session);
  }

  function renderNasCrumbs(path) {
    if (!nasPath) return;
    const parts = normalizeNasPath(path).split("/").filter(Boolean);
    let html =
      '<button type="button" class="nas-crumb-item' +
      (parts.length ? "" : " is-current") +
      '" data-nas-path="/" title="/">/</button>';
    let acc = "";
    for (let i = 0; i < parts.length; i += 1) {
      acc += "/" + parts[i];
      const last = i === parts.length - 1;
      html +=
        '<span class="nas-crumb-sep" aria-hidden="true">/</span>' +
        '<button type="button" class="nas-crumb-item' +
        (last ? " is-current" : "") +
        '" data-nas-path="' +
        escapeHtml(acc) +
        '" title="' +
        escapeHtml(acc) +
        '">' +
        escapeHtml(parts[i]) +
        "</button>";
    }
    nasPath.innerHTML = html;
    if (nasPathUp) nasPathUp.disabled = !parts.length;
  }

  function classifyNasName(name, isDir) {
    if (isDir) return "folder";
    const ext = String(name || "")
      .split(".")
      .pop()
      .toLowerCase();
    if (IMAGE_EXT.indexOf(ext) >= 0) return "image";
    if (VIDEO_EXT.indexOf(ext) >= 0) return "video";
    if (AUDIO_EXT.indexOf(ext) >= 0) return "audio";
    if (ext === "pdf") return "pdf";
    if (["zip", "rar", "7z", "tar", "gz", "bz2", "xz"].indexOf(ext) >= 0) return "archive";
    if (OFFICE_EXT.indexOf(ext) >= 0) return "doc";
    if (TEXT_EXT.indexOf(ext) >= 0) return "text";
    if (CODE_EXT.indexOf(ext) >= 0) return "code";
    return "file";
  }

  function renderNasExplorer(session) {
    if (!session) return;
    if (nasTitle) nasTitle.textContent = session.remark || "网盘连接";
    renderNasCrumbs(session.nasPath || "/");
    applyNasView();
    const info = session.nasInfo;
    if (!info) {
      if (nasDiskTotal) nasDiskTotal.textContent = "—";
      if (nasDiskFree) nasDiskFree.textContent = "—";
      if (nasDiskUsed) nasDiskUsed.textContent = "—";
      if (nasFileNum) nasFileNum.textContent = "—";
      if (nasMeterBar) nasMeterBar.style.width = "0%";
      if (nasMeterBar && nasMeterBar.parentElement) {
        nasMeterBar.parentElement.classList.remove("is-warn", "is-full");
      }
      if (nasStatsHint) nasStatsHint.textContent = "正在获取网盘信息…";
    } else {
      const total = info.diskSize || 0;
      const free = info.banlenSize || 0;
      const used = total > free ? total - free : 0;
      const pct = total > 0 ? Math.min(100, Math.round((used / total) * 100)) : 0;
      if (nasDiskTotal) nasDiskTotal.textContent = formatSize(total);
      if (nasDiskFree) nasDiskFree.textContent = formatSize(free);
      if (nasDiskUsed) nasDiskUsed.textContent = formatSize(used) + "  (" + pct + "%)";
      if (nasFileNum) nasFileNum.textContent = String(info.fileNum || 0);
      if (nasMeterBar) nasMeterBar.style.width = pct + "%";
      if (nasMeterBar && nasMeterBar.parentElement) {
        nasMeterBar.parentElement.classList.toggle("is-warn", pct >= 80 && pct < 95);
        nasMeterBar.parentElement.classList.toggle("is-full", pct >= 95);
      }
      if (nasStatsHint) {
        nasStatsHint.textContent = "更新于 " + formatClock(info.updatedAt);
      }
    }
    renderNasFiles(session);
    renderNasTaskPanel(session);
    ensureNasTasks(session);
  }

  function bindWorkspaceEvents() {
    document.getElementById("btn-logout").addEventListener("click", logout);
    document.getElementById("btn-new-session").addEventListener("click", function () {
      openModal("new");
    });
    document.getElementById("btn-new-drive").addEventListener("click", function () {
      openModal("new-drive");
    });
    bindSidebarSplit();
    bindNasExplorer();
    document.getElementById("btn-create-session").addEventListener("click", createSession);
    document.getElementById("btn-create-drive").addEventListener("click", createDrive);
    document.getElementById("btn-save-remark").addEventListener("click", saveRemark);
    confirmOkBtn.addEventListener("click", onConfirmOk);
    connectBtn.addEventListener("click", connectSelected);
    document.getElementById("btn-voice").addEventListener("click", startVoiceCall);
    document.getElementById("btn-desktop").addEventListener("click", function () {
      const btn = document.getElementById("btn-desktop");
      if (btn && btn.textContent === "结束控制") {
        tauriInvoke("desktop_hangup").catch(showDesktopError);
        return;
      }
      startDesktopControl();
    });
    document.getElementById("desktop-stop").addEventListener("click", function () {
      tauriInvoke("desktop_hangup").catch(showDesktopError);
    });
    document.getElementById("desktop-cancel").addEventListener("click", function () {
      tauriInvoke("desktop_hangup").catch(showDesktopError);
    });
    document.getElementById("desktop-reject").addEventListener("click", function () {
      tauriInvoke("desktop_reject").catch(showDesktopError);
    });
    document.getElementById("desktop-accept").addEventListener("click", function () {
      tauriInvoke("desktop_accept").catch(showDesktopError);
    });
    document.getElementById("desktop-full").addEventListener("click", function () {
      setDesktopFullscreen(!state.desktopFullscreen);
    });
    bindDesktopPointer();
    document.addEventListener("keydown", function (event) {
      if (event.key === "Escape" && state.desktopFullscreen) {
        event.preventDefault();
        setDesktopFullscreen(false);
      }
    });
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
    if (driveListEl) driveListEl.addEventListener("click", onSessionListClick);
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
      applySidebarSplit();
      layoutDesktopCanvas();
    });
    document.addEventListener("keydown", function (event) {
      if (event.key === "Escape") {
        if (nasSearchDrop && !nasSearchDrop.hidden) {
          event.preventDefault();
          hideNasSearchDrop();
          return;
        }
        if (nasRenameCtl.active) {
          event.preventDefault();
          cancelNasRename();
          return;
        }
        if (nasEditorEl && !nasEditorEl.hidden) {
          event.preventDefault();
          if (nasEditorAsk && !nasEditorAsk.hidden) hideNasEditorAsk();
          else requestNasEditorExit();
          return;
        }
        if (nasOfficeEl && !nasOfficeEl.hidden) {
          event.preventDefault();
          if (nasOfficeAsk && !nasOfficeAsk.hidden) hideNasOfficeAsk();
          else requestNasOfficeExit();
          return;
        }
        hideBubbleMenu();
        hideShotMenu();
        closeMediaOverlay();
        closeNasPreview();
      }
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "f") {
        const drive = selectedDrive();
        if (drive && nasSearchInput) {
          event.preventDefault();
          nasSearchInput.focus();
          nasSearchInput.select();
          return;
        }
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
    if (nasPreview) {
      nasPreview.addEventListener("click", function (event) {
        if (event.target.closest("[data-close-nas-preview]")) {
          event.preventDefault();
          closeNasPreview();
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
      if (nasCtx && !nasCtx.hidden && !event.target.closest("#nas-ctx")) {
        hideNasCtx();
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
    if (newDriveToken) {
      newDriveToken.addEventListener("keydown", function (event) {
        if (event.key === "Enter") createDrive();
      });
      newDriveToken.addEventListener("input", function () {
        if (newDriveError) newDriveError.hidden = true;
      });
    }
    if (newDrivePass) {
      newDrivePass.addEventListener("input", function () {
        if (newDriveError) newDriveError.hidden = true;
      });
    }
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
    const session = findItem(id);
    if (action === "remark") {
      state.remarkSessionId = id;
      remarkInput.value = session ? session.remark : "";
      const remarkTitle = document.getElementById("remark-title");
      if (remarkTitle) remarkTitle.textContent = isDrive(session) ? "设置网盘备注" : "设置会话备注";
      remarkInput.placeholder = isDrive(session) ? "例如：家里 NAS" : "例如：小明";
      openModal("remark");
    } else if (action === "close") {
      closeSession(id);
    } else if (action === "clear") {
      if (isDrive(session)) return;
      openConfirm("clear", id);
    } else if (action === "delete") {
      openConfirm(isDrive(session) ? "delete-drive" : "delete", id);
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
    renderDriveList();
    renderChat();
  }

  function renderSessionList() {
    renderPeerList(sessionListEl, state.sessions, {
      empty: '暂无聊天会话<br />点击「新建会话」连接对端',
      closeLabel: "关闭会话",
      deleteLabel: "删除会话",
      showClear: true,
      glyph: "chat",
    });
  }

  function renderDriveList() {
    if (!driveListEl) return;
    renderPeerList(driveListEl, state.drives, {
      empty: '暂无连接<br />点击「新建连接」连接家里的 NAS',
      closeLabel: "关闭连接",
      deleteLabel: "删除连接",
      showClear: false,
      glyph: "drive",
    });
  }

  function renderPeerList(el, items, opts) {
    if (!el) return;
    if (!items.length) {
      el.innerHTML = '<div class="session-list-empty">' + opts.empty + "</div>";
      return;
    }
    el.innerHTML = items
      .map(function (session) {
        const title = session.remark || session.peerToken;
        const sub = session.remark ? session.peerToken : "未备注";
        const active = session.id === state.selectedId ? " is-active" : "";
        const on = session.connected ? " is-on" : "";
        const online = session.connected ? " is-online" : "";
        const menuOpen = session.id === state.menuSessionId;
        const closeBtn = session.connected
          ? '<button type="button" data-action="close">' + opts.closeLabel + "</button>"
          : "";
        const clearBtn = opts.showClear
          ? '<button type="button" data-action="clear">清空内容</button>'
          : "";
        const menu = menuOpen
          ? '<div class="session-dropdown">' +
            '<button type="button" data-action="remark">设置备注</button>' +
            closeBtn +
            clearBtn +
            '<button type="button" class="is-danger" data-action="delete">' +
            opts.deleteLabel +
            "</button>" +
            "</div>"
          : "";
        const glyphClass = opts.glyph === "drive" ? "drive-glyph" : "chat-glyph";
        const icon =
          '<span class="peer-item-icon"><span class="' +
          glyphClass +
          '"></span><i class="status-dot' +
          on +
          '"></i></span>';
        return (
          '<div class="session-row' +
          (menuOpen ? " is-menu-open" : "") +
          online +
          '" data-id="' +
          escapeHtml(session.id) +
          '">' +
          '<div class="session-item' +
          active +
          '">' +
          icon +
          '<div class="session-item-body">' +
          '<span class="session-item-title">' +
          escapeHtml(title) +
          "</span>" +
          '<span class="session-item-sub">' +
          escapeHtml(sub) +
          "</span>" +
          "</div>" +
          '<button class="session-gear" type="button" aria-label="设置">⚙</button>' +
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
    if (panelDrive) panelDrive.classList.toggle("is-visible", name === "drive");
    chatMain.classList.toggle("is-visible", name === "chat");
  }

  function renderChat() {
    const session = findItem(state.selectedId);
    if (!session) {
      showPanel("unselected");
      return;
    }

    if (!session.connected) {
      if (
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
        connectTitle.textContent = isDrive(session) ? "请先连接网盘" : "请先建立 P2P 连接";
      }
      if (connectDesc) {
        connectDesc.textContent = isDrive(session)
          ? "当前尚未与 NAS / 网盘设备连通。连接成功后即可查阅和更新家里的文件。"
          : "当前会话尚未与对端连通，连接成功后即可收发消息和文件。";
      }
      const name = session.remark || session.peerToken;
      connectPeer.textContent = (isDrive(session) ? "网盘 Token：" : "对方 Token：") + session.peerToken;
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

    if (isDrive(session)) {
      showPanel("drive");
      if (!session.nasWatching) startNasWatch(session);
      renderNasExplorer(session);
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
    const btnDesktop = document.getElementById("btn-desktop");
    if (btnDesktop) btnDesktop.disabled = false;
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
    setConfirmFailList([]);
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
    renderDriveList();
  }

  function clearConnectTimer() {
    if (state.connectTimer) {
      window.clearTimeout(state.connectTimer);
      state.connectTimer = null;
    }
  }

  function connectSelected() {
    const session = findItem(state.selectedId);
    if (!session || session.connected || session.connecting) return;
    session.peerPass = connectPeerPass.value.trim();
    session.connectError = "";
    session.connecting = true;
    renderWorkspace();

    const localId = session.id;
    const failText = isDrive(session)
      ? "连接失败。请确认网盘 Token 是否在线，以及当前网络是否可达。"
      : "连接失败。请确认对方 Token 是否在线，以及当前网络是否可达。";
    tauriInvoke("webrpc_open_session", {
      peerToken: session.peerToken,
      passphrase: session.peerPass || "",
    })
      .then(function (sessionId) {
        const sid = Number(sessionId) || 0;
        const current = findItem(localId);
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
          current.connectError = failText;
          renderWorkspace();
          return;
        }
        current.connecting = false;
        current.connected = true;
        current.rpcSessionId = sid;
        current.connectError = "";
        persistItemUpdate(current);
        if (isDrive(current)) {
          startNasWatch(current);
          bindNasTaskSession(current);
        }
        renderWorkspace();
      })
      .catch(function (err) {
        const current = findItem(localId);
        if (!current) return;
        current.connecting = false;
        current.connected = false;
        current.rpcSessionId = 0;
        current.connectError =
          invokeErrorText(err).indexOf("handshake-send-failed") >= 0
            ? "会话通信异常，通知消息未能送达，连接已关闭。请检查网络后重试。"
            : failText;
        renderWorkspace();
      });
  }

  function releaseRpcSession(session) {
    if (isDrive(session)) session.nasWatching = false;
    const sid = session && session.rpcSessionId ? session.rpcSessionId : 0;
    if (session) session.rpcSessionId = 0;
    if (!sid) return Promise.resolve();
    return tauriInvoke("webrpc_close_session", { sessionId: sid }).catch(function () {});
  }

  function openModal(name) {
    if (nasZipCtl.busy && !nasZipCtl.async && name !== "nas-zip") return;
    modalRoot.hidden = false;
    modalNew.hidden = name !== "new";
    if (modalNewDrive) modalNewDrive.hidden = name !== "new-drive";
    modalRemark.hidden = name !== "remark";
    modalConfirm.hidden = name !== "confirm";
    if (modalNasCreate) modalNasCreate.hidden = name !== "nas-create";
    if (modalNasMove) modalNasMove.hidden = name !== "nas-move";
    if (modalNasZip) modalNasZip.hidden = name !== "nas-zip";
    if (name !== "nas-move") resetNasMovePicker();
    newSessionError.hidden = true;
    if (newDriveError) newDriveError.hidden = true;
    if (name === "new") {
      newPeerToken.value = "";
      newPeerPass.value = "";
      window.setTimeout(function () {
        newPeerToken.focus();
      }, 0);
    }
    if (name === "new-drive" && newDriveToken) {
      newDriveToken.value = "";
      if (newDrivePass) newDrivePass.value = "";
      window.setTimeout(function () {
        newDriveToken.focus();
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
    setConfirmFailList([]);
    confirmOkBtn.className = "btn-danger";
    if (confirmCancelBtn) confirmCancelBtn.hidden = false;
    if (kind === "clear") {
      state.clearSessionId = sessionId;
      confirmTitle.textContent = "清空内容";
      confirmDesc.textContent =
        "将清空本会话在本地的聊天记录（含文本和文件预览）。不会断开连接，也不会删除会话、备注或 Token。";
      confirmOkBtn.textContent = "清空";
    } else if (kind === "clear-nas-tasks") {
      confirmTitle.textContent = "清除任务记录";
      confirmDesc.textContent =
        "将清除已完成、失败和已中断的任务记录。进行中和排队中的任务不会被清除。";
      confirmOkBtn.textContent = "清除";
    } else if (kind === "delete-drive") {
      state.deleteSessionId = sessionId;
      confirmTitle.textContent = "删除连接";
      confirmDesc.textContent = "删除后将从「网盘连接」列表和本地缓存中移除。不会影响聊天会话。";
      confirmOkBtn.textContent = "删除";
    } else {
      state.deleteSessionId = sessionId;
      confirmTitle.textContent = "删除会话";
      confirmDesc.textContent = "删除后将从列表和本地缓存中移除。";
      confirmOkBtn.textContent = "删除";
    }
    openModal("confirm");
  }

  function closeModal() {
    if (nasZipCtl.busy && !nasZipCtl.async) return;
    modalRoot.hidden = true;
    modalNew.hidden = true;
    if (modalNewDrive) modalNewDrive.hidden = true;
    modalRemark.hidden = true;
    modalConfirm.hidden = true;
    if (modalNasCreate) modalNasCreate.hidden = true;
    if (modalNasMove) modalNasMove.hidden = true;
    if (modalNasZip) modalNasZip.hidden = true;
    resetNasMovePicker();
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
    if (state.confirmKind === "clear-nas-tasks") {
      state.confirmKind = "";
      closeModal();
      clearNasTasks();
      return;
    }
    if (state.confirmKind === "delete-nas") {
      confirmNasDelete();
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
      kind: "chat",
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

  function createDrive() {
    if (!newDriveToken || !newDriveError) return;
    const token = newDriveToken.value.trim();
    const pass = newDrivePass ? newDrivePass.value.trim() : "";
    if (!token) {
      newDriveError.textContent = "请输入网盘 Token";
      newDriveError.hidden = false;
      return;
    }
    if (
      state.drives.some(function (item) {
        return item.peerToken === token;
      })
    ) {
      newDriveError.textContent = "该网盘已存在";
      newDriveError.hidden = false;
      return;
    }

    const drive = hydrateDrive({
      peerToken: token,
      peerPass: pass,
      remark: "",
    });

    persistDriveCreate(drive)
      .then(function () {
        state.drives.unshift(drive);
        selectSession(drive.id);
        state.connectFillId = null;
        closeModal();
        renderWorkspace();
      })
      .catch(function (err) {
        const text = String((err && err.message) || err || "");
        newDriveError.textContent =
          text.indexOf("drive-exists") >= 0 ? "该网盘已存在" : "保存网盘失败";
        newDriveError.hidden = false;
      });
  }

  function saveRemark() {
    const session = findItem(state.remarkSessionId);
    if (session) {
      session.remark = remarkInput.value.trim();
      persistItemUpdate(session);
    }
    closeModal();
    renderWorkspace();
  }

  function closeSession(id) {
    const session = findItem(id);
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
    const item = findItem(id);
    if (!item) {
      state.deleteSessionId = null;
      state.confirmKind = "";
      closeModal();
      return;
    }
    const list = isDrive(item) ? state.drives : state.sessions;
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

  function findDrive(id) {
    return state.drives.find(function (s) {
      return s.id === id;
    });
  }

  function findItem(id) {
    return findSession(id) || findDrive(id) || null;
  }

  function findByRpcSessionId(sessionId) {
    const sid = Number(sessionId) || 0;
    if (!sid) return null;
    return (
      state.sessions.find(function (item) {
        return item.rpcSessionId === sid;
      }) ||
      state.drives.find(function (item) {
        return item.rpcSessionId === sid;
      }) ||
      null
    );
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
