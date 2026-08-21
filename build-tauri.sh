#!/usr/bin/env bash
# 在当前 HTML UI 项目内编译 Tauri 桌面程序（本机平台）。
# 用法：./build-tauri.sh
# 产物：dist-tauri/

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
UI_DIR="${PROJECT_ROOT}/ui"
SRC_TAURI="${PROJECT_ROOT}/src-tauri"
DIST_DIR="${PROJECT_ROOT}/dist-tauri"
ASSETS_DIR="${PROJECT_ROOT}/assets"
ICONS_DIR="${SRC_TAURI}/icons"

cd "${PROJECT_ROOT}"
export PATH="${HOME}/.cargo/bin:${PATH}"

if ! command -v cargo >/dev/null 2>&1; then
  echo "[ERROR] 未找到 cargo，请先安装 Rust。"
  exit 1
fi

sync_ui() {
  if [[ ! -f "${PROJECT_ROOT}/index.html" ]]; then
    echo "[ERROR] 找不到 ${PROJECT_ROOT}/index.html"
    exit 1
  fi
  mkdir -p "${UI_DIR}/css" "${UI_DIR}/js" "${UI_DIR}/assets"
  cp -f "${PROJECT_ROOT}/index.html" "${UI_DIR}/index.html"
  cp -f "${PROJECT_ROOT}/screenshot.html" "${UI_DIR}/screenshot.html"
  cp -f "${PROJECT_ROOT}/css/app.css" "${UI_DIR}/css/app.css"
  cp -f "${PROJECT_ROOT}/css/screenshot.css" "${UI_DIR}/css/screenshot.css"
  cp -f "${PROJECT_ROOT}/js/app.js" "${UI_DIR}/js/app.js"
  cp -f "${PROJECT_ROOT}/js/screenshot.js" "${UI_DIR}/js/screenshot.js"
  if [[ -f "${ASSETS_DIR}/file2file_logo.png" ]]; then
    cp -f "${ASSETS_DIR}/file2file_logo.png" "${UI_DIR}/assets/file2file_logo.png"
  fi
  echo "[INFO] 已同步前端到 ${UI_DIR}"
}

ensure_icons() {
  mkdir -p "${ICONS_DIR}"
  local icns="${ICONS_DIR}/icon.icns"
  local ico="${ICONS_DIR}/icon.ico"
  local png="${ASSETS_DIR}/file2file_logo.png"
  local source_png="${ICONS_DIR}/_source.png"

  if command -v sips >/dev/null 2>&1; then
    if [[ -f "${icns}" ]]; then
      sips -s format png "${icns}" --out "${source_png}" >/dev/null
    elif [[ -f "${png}" ]]; then
      cp -f "${png}" "${source_png}"
    fi
    if [[ -f "${source_png}" ]]; then
      sips -z 32 32 "${source_png}" --out "${ICONS_DIR}/32x32.png" >/dev/null
      sips -z 128 128 "${source_png}" --out "${ICONS_DIR}/128x128.png" >/dev/null
      sips -z 256 256 "${source_png}" --out "${ICONS_DIR}/128x128@2x.png" >/dev/null
      rm -f "${source_png}"
    fi
  fi

  local missing=0
  for f in 32x32.png 128x128.png "128x128@2x.png" icon.icns icon.ico; do
    if [[ ! -f "${ICONS_DIR}/${f}" ]]; then
      echo "[ERROR] 缺少图标: ${ICONS_DIR}/${f}"
      missing=1
    fi
  done
  if [[ "${missing}" -ne 0 ]]; then
    exit 1
  fi
  echo "[INFO] 图标已就绪"
}

ensure_tauri_cli() {
  if cargo tauri --version >/dev/null 2>&1; then
    cargo tauri --version
    return 0
  fi
  echo "[INFO] 未找到 tauri-cli，正在安装（首次较慢）..."
  cargo install tauri-cli --locked --version "^2"
}

sync_ui
ensure_icons
ensure_tauri_cli

echo "[INFO] 开始编译 Tauri 桌面程序（当前平台）..."
TARGET_DIR="${CARGO_TARGET_DIR:-${SRC_TAURI}/target}"
(
  cd "${SRC_TAURI}"
  cargo tauri build --bundles app
)

mkdir -p "${DIST_DIR}"
BUNDLE_MAC="${TARGET_DIR}/release/bundle/macos/File2File.app"
BUNDLE_LINUX_DEB="$(ls -d "${TARGET_DIR}"/release/bundle/deb/*.deb 2>/dev/null | head -n 1 || true)"
BUNDLE_LINUX_APPIMAGE="$(ls -d "${TARGET_DIR}"/release/bundle/appimage/*.AppImage 2>/dev/null | head -n 1 || true)"
BUNDLE_WIN="$(ls -d "${TARGET_DIR}"/release/bundle/nsis/*.exe 2>/dev/null | head -n 1 || true)"
BIN_UNIX="${TARGET_DIR}/release/file2file-tauri"
BIN_WIN="${TARGET_DIR}/release/file2file-tauri.exe"

if [[ -d "${BUNDLE_MAC}" ]]; then
  rm -rf "${DIST_DIR}/File2File.app"
  cp -R "${BUNDLE_MAC}" "${DIST_DIR}/File2File.app"
  echo "[OK] macOS 应用: ${DIST_DIR}/File2File.app"
  echo "     运行: open \"${DIST_DIR}/File2File.app\""
elif [[ -n "${BUNDLE_LINUX_APPIMAGE}" ]]; then
  cp -f "${BUNDLE_LINUX_APPIMAGE}" "${DIST_DIR}/"
  echo "[OK] Linux AppImage: ${DIST_DIR}/$(basename "${BUNDLE_LINUX_APPIMAGE}")"
elif [[ -n "${BUNDLE_LINUX_DEB}" ]]; then
  cp -f "${BUNDLE_LINUX_DEB}" "${DIST_DIR}/"
  echo "[OK] Linux deb: ${DIST_DIR}/$(basename "${BUNDLE_LINUX_DEB}")"
elif [[ -n "${BUNDLE_WIN}" ]]; then
  cp -f "${BUNDLE_WIN}" "${DIST_DIR}/"
  echo "[OK] Windows 安装包: ${DIST_DIR}/$(basename "${BUNDLE_WIN}")"
elif [[ -x "${BIN_UNIX}" ]]; then
  cp -f "${BIN_UNIX}" "${DIST_DIR}/File2File"
  echo "[OK] 可执行文件: ${DIST_DIR}/File2File"
elif [[ -f "${BIN_WIN}" ]]; then
  cp -f "${BIN_WIN}" "${DIST_DIR}/File2File.exe"
  echo "[OK] 可执行文件: ${DIST_DIR}/File2File.exe"
else
  echo "[ERROR] 编译完成但未找到打包产物，请查看 ${TARGET_DIR}/release/bundle"
  exit 1
fi
