(function () {
  const HANDLE = 8;
  const MIN_SIZE = 4;

  const bg = document.getElementById("bg");
  const canvas = document.getElementById("overlay");
  const ctx = canvas.getContext("2d");
  const hint = document.getElementById("hint");
  const sizeTip = document.getElementById("size-tip");
  const magnifier = document.getElementById("magnifier");
  const magCanvas = document.getElementById("mag-canvas");
  const magMeta = document.getElementById("mag-meta");
  const magCtx = magCanvas.getContext("2d");
  const toolbar = document.getElementById("toolbar");
  const styleBar = document.getElementById("style-bar");
  const textEditor = document.getElementById("text-editor");

  const src = document.createElement("canvas");
  const srcCtx = src.getContext("2d");

  const state = {
    ready: false,
    sx: 1,
    sy: 1,
    sel: null,
    drag: null,
    tool: "",
    color: "#f45454",
    width: 2,
    marks: [],
    draft: null,
    finishing: false,
  };

  function invoke(cmd, args) {
    try {
      if (window.__TAURI_INTERNALS__ && typeof window.__TAURI_INTERNALS__.invoke === "function") {
        return window.__TAURI_INTERNALS__.invoke(cmd, args || {});
      }
      if (window.__TAURI__ && window.__TAURI__.core && typeof window.__TAURI__.core.invoke === "function") {
        return window.__TAURI__.core.invoke(cmd, args || {});
      }
    } catch (_) {}
    return Promise.reject(new Error("unavailable"));
  }

  function convertFileSrc(path) {
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

  function loadImage(url) {
    return new Promise(function (resolve, reject) {
      const img = new Image();
      img.onload = function () {
        resolve(img);
      };
      img.onerror = reject;
      img.src = url;
    });
  }

  async function boot() {
    const info = await invoke("screenshot_overlay_info");
    const path = info && info.imagePath;
    if (!path) throw new Error("missing-path");
    let url = "";
    try {
      const bytes = await invoke("screenshot_overlay_png");
      const raw = bytes instanceof Uint8Array ? bytes : Uint8Array.from(bytes || []);
      url = URL.createObjectURL(new Blob([raw], { type: "image/png" }));
    } catch (_) {
      try {
        const srcUrl = convertFileSrc(path);
        const res = await fetch(srcUrl);
        const blob = await res.blob();
        url = URL.createObjectURL(blob);
      } catch (__) {
        url = convertFileSrc(path);
      }
    }
    const image = await loadImage(url);
    bg.src = url;
    src.width = image.naturalWidth;
    src.height = image.naturalHeight;
    srcCtx.drawImage(image, 0, 0);
    resizeCanvas();
    state.ready = true;
    paint();
  }

  function resizeCanvas() {
    const w = window.innerWidth;
    const h = window.innerHeight;
    canvas.width = src.width;
    canvas.height = src.height;
    canvas.style.width = w + "px";
    canvas.style.height = h + "px";
    state.sx = src.width / Math.max(w, 1);
    state.sy = src.height / Math.max(h, 1);
  }

  function toImg(event) {
    const rect = canvas.getBoundingClientRect();
    return {
      x: clamp((event.clientX - rect.left) * state.sx, 0, src.width),
      y: clamp((event.clientY - rect.top) * state.sy, 0, src.height),
    };
  }

  function toCss(x, y) {
    return { x: x / state.sx, y: y / state.sy };
  }

  function clamp(v, min, max) {
    return Math.max(min, Math.min(max, v));
  }

  function normRect(a, b) {
    const x = Math.min(a.x, b.x);
    const y = Math.min(a.y, b.y);
    return {
      x: x,
      y: y,
      w: Math.abs(a.x - b.x),
      h: Math.abs(a.y - b.y),
    };
  }

  function selRect() {
    return state.sel;
  }

  function insideSel(pt) {
    const s = selRect();
    if (!s) return false;
    return pt.x >= s.x && pt.x <= s.x + s.w && pt.y >= s.y && pt.y <= s.y + s.h;
  }

  function handleAt(pt) {
    const s = selRect();
    if (!s) return "";
    const hs = HANDLE * Math.max(state.sx, state.sy);
    const points = {
      nw: { x: s.x, y: s.y },
      n: { x: s.x + s.w / 2, y: s.y },
      ne: { x: s.x + s.w, y: s.y },
      e: { x: s.x + s.w, y: s.y + s.h / 2 },
      se: { x: s.x + s.w, y: s.y + s.h },
      s: { x: s.x + s.w / 2, y: s.y + s.h },
      sw: { x: s.x, y: s.y + s.h },
      w: { x: s.x, y: s.y + s.h / 2 },
    };
    let found = "";
    Object.keys(points).forEach(function (key) {
      const p = points[key];
      if (Math.abs(pt.x - p.x) <= hs && Math.abs(pt.y - p.y) <= hs) found = key;
    });
    return found;
  }

  function setCursor(name) {
    document.body.classList.remove("is-move", "is-nwse", "is-nesw", "is-ew", "is-ns", "is-text", "is-draw");
    if (name) document.body.classList.add(name);
  }

  function cursorFor(pt) {
    if (textEditor && !textEditor.hidden) return "is-text";
    if (!state.sel) return "";
    const h = handleAt(pt);
    if (h === "nw" || h === "se") return "is-nwse";
    if (h === "ne" || h === "sw") return "is-nesw";
    if (h === "e" || h === "w") return "is-ew";
    if (h === "n" || h === "s") return "is-ns";
    if (insideSel(pt)) {
      if (state.tool === "text") return "is-text";
      if (state.tool) return "is-draw";
      return "is-move";
    }
    return "";
  }

  function paint() {
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    ctx.fillStyle = "rgba(0,0,0,0.55)";
    ctx.fillRect(0, 0, canvas.width, canvas.height);
    const s = selRect();
    if (!s || s.w < 1 || s.h < 1) return;

    ctx.save();
    ctx.beginPath();
    ctx.rect(s.x, s.y, s.w, s.h);
    ctx.clip();
    ctx.drawImage(src, 0, 0);
    state.marks.forEach(function (mark) {
      drawMark(ctx, mark);
    });
    if (state.draft) drawMark(ctx, state.draft);
    ctx.restore();

    ctx.strokeStyle = "#4da3ff";
    ctx.lineWidth = Math.max(state.sx, state.sy);
    ctx.strokeRect(s.x + 0.5, s.y + 0.5, Math.max(0, s.w - 1), Math.max(0, s.h - 1));

    const hs = HANDLE * Math.max(state.sx, state.sy);
    const handles = [
      [s.x, s.y],
      [s.x + s.w / 2, s.y],
      [s.x + s.w, s.y],
      [s.x + s.w, s.y + s.h / 2],
      [s.x + s.w, s.y + s.h],
      [s.x + s.w / 2, s.y + s.h],
      [s.x, s.y + s.h],
      [s.x, s.y + s.h / 2],
    ];
    ctx.fillStyle = "#fff";
    ctx.strokeStyle = "#4da3ff";
    handles.forEach(function (p) {
      ctx.fillRect(p[0] - hs / 2, p[1] - hs / 2, hs, hs);
      ctx.strokeRect(p[0] - hs / 2, p[1] - hs / 2, hs, hs);
    });
  }

  function drawMark(c, mark) {
    c.save();
    c.strokeStyle = mark.color;
    c.fillStyle = mark.color;
    c.lineWidth = mark.width * Math.max(state.sx, 1);
    c.lineCap = "round";
    c.lineJoin = "round";
    if (mark.type === "rect") {
      c.strokeRect(mark.x, mark.y, mark.w, mark.h);
    } else if (mark.type === "ellipse") {
      c.beginPath();
      c.ellipse(mark.x + mark.w / 2, mark.y + mark.h / 2, Math.abs(mark.w / 2), Math.abs(mark.h / 2), 0, 0, Math.PI * 2);
      c.stroke();
    } else if (mark.type === "arrow") {
      drawArrow(c, mark.x1, mark.y1, mark.x2, mark.y2, c.lineWidth);
    } else if (mark.type === "pen") {
      const pts = mark.points || [];
      if (pts.length) {
        c.beginPath();
        c.moveTo(pts[0].x, pts[0].y);
        for (let i = 1; i < pts.length; i++) c.lineTo(pts[i].x, pts[i].y);
        c.stroke();
      }
    } else if (mark.type === "mosaic") {
      applyMosaic(c, mark.points || [], mark.width);
    } else if (mark.type === "text") {
      const size = 12 + mark.width * 4;
      c.font = size * Math.max(state.sx, 1) + "px sans-serif";
      c.textBaseline = "top";
      const lines = String(mark.text || "").split("\n");
      lines.forEach(function (line, i) {
        c.fillText(line, mark.x, mark.y + i * size * 1.35 * Math.max(state.sx, 1));
      });
    }
    c.restore();
  }

  function drawArrow(c, x1, y1, x2, y2, width) {
    const angle = Math.atan2(y2 - y1, x2 - x1);
    const head = 10 + width * 2;
    c.beginPath();
    c.moveTo(x1, y1);
    c.lineTo(x2, y2);
    c.stroke();
    c.beginPath();
    c.moveTo(x2, y2);
    c.lineTo(x2 - head * Math.cos(angle - 0.4), y2 - head * Math.sin(angle - 0.4));
    c.lineTo(x2 - head * Math.cos(angle + 0.4), y2 - head * Math.sin(angle + 0.4));
    c.closePath();
    c.fill();
  }

  function applyMosaic(c, points, width) {
    const block = Math.max(6, Math.round(8 * state.sx));
    const radius = Math.max(12, width * 7) * state.sx;
    const seen = {};
    points.forEach(function (pt) {
      const x0 = Math.floor((pt.x - radius) / block) * block;
      const y0 = Math.floor((pt.y - radius) / block) * block;
      const x1 = pt.x + radius;
      const y1 = pt.y + radius;
      for (let y = y0; y < y1; y += block) {
        for (let x = x0; x < x1; x += block) {
          const dx = x + block / 2 - pt.x;
          const dy = y + block / 2 - pt.y;
          if (dx * dx + dy * dy > radius * radius) continue;
          const key = x + "," + y;
          if (seen[key]) continue;
          seen[key] = 1;
          const sx = clamp(Math.floor(x), 0, src.width - 1);
          const sy = clamp(Math.floor(y), 0, src.height - 1);
          const pixel = srcCtx.getImageData(sx, sy, 1, 1).data;
          c.fillStyle = "rgba(" + pixel[0] + "," + pixel[1] + "," + pixel[2] + "," + pixel[3] / 255 + ")";
          c.fillRect(x, y, block, block);
        }
      }
    });
  }

  function layoutUi() {
    const s = selRect();
    if (!s) {
      sizeTip.hidden = true;
      toolbar.hidden = true;
      return;
    }
    const a = toCss(s.x, s.y);
    const b = toCss(s.x + s.w, s.y + s.h);
    const left = Math.min(a.x, b.x);
    const top = Math.min(a.y, b.y);
    const right = Math.max(a.x, b.x);
    const bottom = Math.max(a.y, b.y);
    const w = Math.round(s.w);
    const h = Math.round(s.h);
    sizeTip.hidden = false;
    sizeTip.textContent = w + " × " + h;
    let tipX = left;
    let tipY = top - 24;
    if (tipY < 8) tipY = top + 8;
    sizeTip.style.left = Math.max(8, tipX) + "px";
    sizeTip.style.top = tipY + "px";

    if (state.drag && state.drag.kind === "create") {
      toolbar.hidden = true;
      return;
    }
    toolbar.hidden = false;
    styleBar.hidden = !state.tool;
    requestAnimationFrame(function () {
      const tw = toolbar.offsetWidth || 320;
      const th = toolbar.offsetHeight || 40;
      let x = right - tw;
      let y = bottom + 8;
      if (x < 8) x = 8;
      if (x + tw > window.innerWidth - 8) x = window.innerWidth - tw - 8;
      if (y + th > window.innerHeight - 8) y = top - th - 8;
      if (y < 8) y = 8;
      toolbar.style.left = x + "px";
      toolbar.style.top = y + "px";
    });
  }

  function updateMagnifier(event) {
    if (state.sel && !(state.drag && state.drag.kind === "create")) {
      magnifier.hidden = true;
      return;
    }
    const pt = toImg(event);
    magnifier.hidden = false;
    const zoom = 8;
    const mw = magCanvas.width;
    const mh = magCanvas.height;
    magCtx.imageSmoothingEnabled = false;
    magCtx.clearRect(0, 0, mw, mh);
    magCtx.drawImage(src, pt.x - mw / zoom / 2, pt.y - mh / zoom / 2, mw / zoom, mh / zoom, 0, 0, mw, mh);
    magCtx.strokeStyle = "#f45454";
    magCtx.beginPath();
    magCtx.moveTo(mw / 2, 0);
    magCtx.lineTo(mw / 2, mh);
    magCtx.moveTo(0, mh / 2);
    magCtx.lineTo(mw, mh / 2);
    magCtx.stroke();
    magMeta.textContent = Math.round(pt.x) + ", " + Math.round(pt.y);
    let left = event.clientX + 18;
    let top = event.clientY + 18;
    if (left + 120 > window.innerWidth) left = event.clientX - 128;
    if (top + 110 > window.innerHeight) top = event.clientY - 118;
    magnifier.style.left = left + "px";
    magnifier.style.top = top + "px";
  }

  function commitText() {
    if (textEditor.hidden) return;
    const text = textEditor.value.replace(/\s+$/, "");
    const x = Number(textEditor.dataset.x || 0);
    const y = Number(textEditor.dataset.y || 0);
    textEditor.hidden = true;
    textEditor.value = "";
    if (text) {
      state.marks.push({
        type: "text",
        x: x,
        y: y,
        text: text,
        color: state.color,
        width: state.width,
      });
    }
    paint();
  }

  function openText(pt) {
    commitText();
    const css = toCss(pt.x, pt.y);
    textEditor.hidden = false;
    textEditor.dataset.x = String(pt.x);
    textEditor.dataset.y = String(pt.y);
    textEditor.style.left = css.x + "px";
    textEditor.style.top = css.y + "px";
    textEditor.style.color = state.color;
    textEditor.style.fontSize = 12 + state.width * 4 + "px";
    textEditor.value = "";
    textEditor.focus();
  }

  function onDown(event) {
    if (!state.ready || event.button !== 0) return;
    if (event.target.closest("#toolbar") || event.target === textEditor) return;
    commitText();
    const pt = toImg(event);
    const handle = handleAt(pt);
    if (handle) {
      state.drag = { kind: "resize", handle: handle, start: pt, sel: Object.assign({}, state.sel) };
      return;
    }
    if (state.sel && insideSel(pt) && state.tool === "text") {
      openText(pt);
      return;
    }
    if (state.sel && insideSel(pt) && state.tool) {
      state.draft = makeDraft(pt);
      state.drag = { kind: "draw", start: pt };
      paint();
      return;
    }
    if (state.sel && insideSel(pt) && !state.tool) {
      state.drag = { kind: "move", start: pt, sel: Object.assign({}, state.sel) };
      return;
    }
    if (!state.sel) {
      hint.classList.add("is-hide");
      state.sel = { x: pt.x, y: pt.y, w: 0, h: 0 };
      state.drag = { kind: "create", start: pt };
      paint();
      layoutUi();
    }
  }

  function makeDraft(pt) {
    const base = { type: state.tool, color: state.color, width: state.width };
    if (state.tool === "rect" || state.tool === "ellipse") {
      return Object.assign(base, { x: pt.x, y: pt.y, w: 0, h: 0 });
    }
    if (state.tool === "arrow") {
      return Object.assign(base, { x1: pt.x, y1: pt.y, x2: pt.x, y2: pt.y });
    }
    return Object.assign(base, { points: [{ x: pt.x, y: pt.y }] });
  }

  function resizeSel(origin, handle, pt) {
    let x1 = origin.x;
    let y1 = origin.y;
    let x2 = origin.x + origin.w;
    let y2 = origin.y + origin.h;
    if (handle.indexOf("n") >= 0) y1 = pt.y;
    if (handle.indexOf("s") >= 0) y2 = pt.y;
    if (handle.indexOf("w") >= 0) x1 = pt.x;
    if (handle.indexOf("e") >= 0) x2 = pt.x;
    const next = normRect({ x: x1, y: y1 }, { x: x2, y: y2 });
    next.x = clamp(next.x, 0, src.width);
    next.y = clamp(next.y, 0, src.height);
    next.w = clamp(next.w, MIN_SIZE, src.width - next.x);
    next.h = clamp(next.h, MIN_SIZE, src.height - next.y);
    return next;
  }

  function onMove(event) {
    if (!state.ready) return;
    const pt = toImg(event);
    setCursor(cursorFor(pt));
    updateMagnifier(event);
    if (!state.drag) return;
    if (state.drag.kind === "create") {
      state.sel = normRect(state.drag.start, pt);
      paint();
      layoutUi();
      return;
    }
    if (state.drag.kind === "move") {
      const dx = pt.x - state.drag.start.x;
      const dy = pt.y - state.drag.start.y;
      const origin = state.drag.sel;
      state.sel = {
        x: clamp(origin.x + dx, 0, Math.max(0, src.width - origin.w)),
        y: clamp(origin.y + dy, 0, Math.max(0, src.height - origin.h)),
        w: origin.w,
        h: origin.h,
      };
      paint();
      layoutUi();
      return;
    }
    if (state.drag.kind === "resize") {
      state.sel = resizeSel(state.drag.sel, state.drag.handle, pt);
      paint();
      layoutUi();
      return;
    }
    if (state.drag.kind === "draw" && state.draft) {
      if (state.draft.type === "rect" || state.draft.type === "ellipse") {
        const r = normRect(state.drag.start, pt);
        state.draft.x = r.x;
        state.draft.y = r.y;
        state.draft.w = r.w;
        state.draft.h = r.h;
      } else if (state.draft.type === "arrow") {
        state.draft.x2 = pt.x;
        state.draft.y2 = pt.y;
      } else {
        state.draft.points.push({ x: pt.x, y: pt.y });
      }
      paint();
    }
  }

  function onUp() {
    if (!state.drag) return;
    if (state.drag.kind === "create" && state.sel && (state.sel.w < MIN_SIZE || state.sel.h < MIN_SIZE)) {
      state.sel = null;
      hint.classList.remove("is-hide");
    }
    if (state.drag.kind === "draw" && state.draft) {
      state.marks.push(state.draft);
      state.draft = null;
    }
    state.drag = null;
    paint();
    layoutUi();
    magnifier.hidden = !!state.sel;
  }

  function cropBox() {
    const s = selRect();
    if (!s || s.w < 1 || s.h < 1) return null;
    const x = Math.max(0, Math.floor(s.x));
    const y = Math.max(0, Math.floor(s.y));
    const w = Math.max(1, Math.min(src.width - x, Math.ceil(s.w)));
    const h = Math.max(1, Math.min(src.height - y, Math.ceil(s.h)));
    return { x: x, y: y, w: w, h: h };
  }

  function exportOverlay(box) {
    const out = document.createElement("canvas");
    out.width = box.w;
    out.height = box.h;
    const octx = out.getContext("2d");
    octx.translate(-box.x, -box.y);
    state.marks.forEach(function (mark) {
      drawMark(octx, mark);
    });
    return out;
  }

  function sendFinish(box, overlayPng) {
    invoke("screenshot_finish", {
      x: box.x,
      y: box.y,
      width: box.w,
      height: box.h,
      overlayPng: overlayPng || [],
    }).catch(function () {
      state.finishing = false;
    });
  }

  function finish() {
    if (state.finishing) return;
    commitText();
    const box = cropBox();
    if (!box) return;
    state.finishing = true;
    if (!state.marks.length) {
      sendFinish(box, []);
      return;
    }
    const out = exportOverlay(box);
    out.toBlob(function (blob) {
      if (!blob) {
        sendFinish(box, []);
        return;
      }
      blob.arrayBuffer().then(function (buf) {
        sendFinish(box, Array.from(new Uint8Array(buf)));
      }).catch(function () {
        state.finishing = false;
      });
    }, "image/png");
  }

  function cancel() {
    invoke("screenshot_cancel").catch(function () {});
  }

  function undo() {
    commitText();
    state.marks.pop();
    paint();
  }

  toolbar.addEventListener("mousedown", function (event) {
    event.stopPropagation();
  });
  toolbar.addEventListener("click", function (event) {
    const btn = event.target.closest("button");
    if (!btn) return;
    if (btn.dataset.tool) {
      commitText();
      state.tool = state.tool === btn.dataset.tool ? "" : btn.dataset.tool;
      toolbar.querySelectorAll("[data-tool]").forEach(function (el) {
        el.classList.toggle("is-on", el.dataset.tool === state.tool);
      });
      layoutUi();
      return;
    }
    if (btn.dataset.act === "undo") undo();
    if (btn.dataset.act === "cancel") cancel();
    if (btn.dataset.act === "ok") finish();
  });
  styleBar.addEventListener("click", function (event) {
    const btn = event.target.closest("button");
    if (!btn) return;
    if (btn.dataset.width) {
      state.width = Number(btn.dataset.width) || 2;
      styleBar.querySelectorAll(".swatch-size").forEach(function (el) {
        el.classList.toggle("is-on", el === btn);
      });
    }
    if (btn.dataset.color) {
      state.color = btn.dataset.color;
      styleBar.querySelectorAll(".swatch-color").forEach(function (el) {
        el.classList.toggle("is-on", el === btn);
      });
    }
  });

  window.addEventListener("mousedown", onDown);
  window.addEventListener("mousemove", onMove);
  window.addEventListener("mouseup", onUp);
  window.addEventListener("dblclick", function (event) {
    if (!state.sel) return;
    if (insideSel(toImg(event))) finish();
  });
  window.addEventListener("contextmenu", function (event) {
    event.preventDefault();
    cancel();
  });
  window.addEventListener("keydown", function (event) {
    if (event.key === "Escape") {
      if (!textEditor.hidden) {
        textEditor.hidden = true;
        textEditor.value = "";
        return;
      }
      cancel();
      return;
    }
    if (event.key === "Enter" && !event.shiftKey && textEditor.hidden) {
      event.preventDefault();
      finish();
    }
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "z") {
      event.preventDefault();
      undo();
    }
  });
  window.addEventListener("resize", function () {
    resizeCanvas();
    paint();
    layoutUi();
  });

  boot().catch(function () {
    cancel();
  });
})();
