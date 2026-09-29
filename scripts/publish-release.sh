#!/usr/bin/env bash
# 将 dist-tauri 里已编译好的各平台包发布到 GitHub Releases。
#
# ========== 配置（改版本号主要改这里）==========
# 也可临时覆盖：VERSION=0.2.0 ./scripts/publish-release.sh
: "${VERSION:=0.1.0}"
# 是否发布为草稿（1=草稿，0=正式发布）
: "${DRAFT:=0}"
# 仓库（留空则用当前 git remote origin），例如 xiaoming-software/File2File-Desktop
: "${REPO:=}"
# ============================================
#
# 用法：
#   ./scripts/publish-release.sh              # 按上面 VERSION 发布
#   VERSION=0.2.0 ./scripts/publish-release.sh  # 临时覆盖版本号
#   ./scripts/publish-release.sh --dry-run     # 只打包，不上传
#   ./scripts/publish-release.sh --draft       # 先发草稿
#
# 前置：
#   1. 已执行 ./build-all.sh，产物在 dist-tauri/
#   2. 已安装 gh：brew install gh && gh auth login
#   3. VERSION 建议与 src-tauri/Cargo.toml、tauri.conf.json 一致

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST_DIR="${PROJECT_ROOT}/dist-tauri"
ASSETS_DIR="${PROJECT_ROOT}/release-assets"
DRY_RUN="0"

log() { echo "[INFO] $*"; }
err() { echo "[ERROR] $*" >&2; }
warn() { echo "[WARN] $*" >&2; }

usage() {
  cat <<EOF
用法: $0 [--dry-run] [--draft] [--help]

在脚本顶部修改 VERSION，或临时覆盖：
  VERSION=0.1.1 ./scripts/publish-release.sh
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run) DRY_RUN="1"; shift ;;
    --draft) DRAFT="1"; shift ;;
    -h|--help) usage; exit 0 ;;
    *) err "未知参数: $1"; usage; exit 1 ;;
  esac
done

# 规范化版本：配置写 0.1.0，tag 用 v0.1.0
VERSION="${VERSION#v}"
if [[ ! "${VERSION}" =~ ^[0-9]+\.[0-9]+\.[0-9]+([.-][A-Za-z0-9.]+)?$ ]]; then
  err "VERSION 格式不正确: ${VERSION}（示例: 0.1.0）"
  exit 1
fi
TAG="v${VERSION}"
TITLE="File2File ${TAG}"

cd "${PROJECT_ROOT}"

resolve_repo() {
  if [[ -n "${REPO}" ]]; then
    echo "${REPO}"
    return
  fi
  local url
  url="$(git remote get-url origin 2>/dev/null || true)"
  if [[ -z "${url}" ]]; then
    err "无法解析 git remote origin，请设置脚本顶部 REPO=owner/name"
    exit 1
  fi
  # https://github.com/owner/repo.git 或 git@github.com:owner/repo.git
  echo "${url}" \
    | sed -E 's#^git@github\.com:#https://github.com/#' \
    | sed -E 's#\.git$##' \
    | sed -E 's#^https://github.com/##'
}

check_prereqs() {
  if ! command -v gh >/dev/null 2>&1; then
    err "未找到 gh。请先执行: brew install gh && gh auth login"
    exit 1
  fi
  if ! gh auth status >/dev/null 2>&1; then
    err "gh 未登录。请先执行: gh auth login"
    exit 1
  fi
  if [[ ! -d "${DIST_DIR}" ]]; then
    err "找不到 ${DIST_DIR}，请先 ./build-all.sh"
    exit 1
  fi
}

warn_version_mismatch() {
  local cargo_ver conf_ver
  cargo_ver="$(sed -nE 's/^version[[:space:]]*=[[:space:]]*"([^"]+)".*/\1/p' "${PROJECT_ROOT}/src-tauri/Cargo.toml" | head -1)"
  conf_ver="$(sed -nE 's/^[[:space:]]*"version"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/p' "${PROJECT_ROOT}/src-tauri/tauri.conf.json" | head -1)"
  if [[ -n "${cargo_ver}" && "${cargo_ver}" != "${VERSION}" ]]; then
    warn "Cargo.toml version=${cargo_ver} 与发布 VERSION=${VERSION} 不一致（可继续发布）"
  fi
  if [[ -n "${conf_ver}" && "${conf_ver}" != "${VERSION}" ]]; then
    warn "tauri.conf.json version=${conf_ver} 与发布 VERSION=${VERSION} 不一致（可继续发布）"
  fi
}

prepare_assets() {
  rm -rf "${ASSETS_DIR}"
  mkdir -p "${ASSETS_DIR}"

  local macos_app win_exe deb_amd64 deb_arm64
  macos_app=""
  if [[ -d "${DIST_DIR}/macos/File2File.app" ]]; then
    macos_app="${DIST_DIR}/macos/File2File.app"
  elif [[ -d "${DIST_DIR}/File2File.app" ]]; then
    macos_app="${DIST_DIR}/File2File.app"
  fi

  win_exe="${DIST_DIR}/windows-x64/File2File.exe"
  # deb 文件名通常带编译时版本；优先精确匹配，其次通配
  deb_amd64="${DIST_DIR}/linux-amd64/File2File_${VERSION}_amd64.deb"
  deb_arm64="${DIST_DIR}/linux-arm64/File2File_${VERSION}_arm64.deb"
  if [[ ! -f "${deb_amd64}" ]]; then
    deb_amd64="$(ls -1 "${DIST_DIR}/linux-amd64/"File2File_*_amd64.deb 2>/dev/null | head -1 || true)"
  fi
  if [[ ! -f "${deb_arm64}" ]]; then
    deb_arm64="$(ls -1 "${DIST_DIR}/linux-arm64/"File2File_*_arm64.deb 2>/dev/null | head -1 || true)"
  fi

  local missing=0
  if [[ -z "${macos_app}" || ! -d "${macos_app}" ]]; then
    err "缺少 macOS 产物: dist-tauri/macos/File2File.app"
    missing=1
  fi
  if [[ ! -f "${win_exe}" ]]; then
    err "缺少 Windows 产物: ${win_exe}"
    missing=1
  fi
  if [[ -z "${deb_amd64}" || ! -f "${deb_amd64}" ]]; then
    err "缺少 Linux amd64 .deb"
    missing=1
  fi
  if [[ -z "${deb_arm64}" || ! -f "${deb_arm64}" ]]; then
    err "缺少 Linux arm64 .deb"
    missing=1
  fi
  if [[ "${missing}" -ne 0 ]]; then
    err "请先完整执行 ./build-all.sh"
    exit 1
  fi

  log "打包 macOS .app → zip..."
  ditto -c -k --sequesterRsrc --keepParent \
    "${macos_app}" \
    "${ASSETS_DIR}/File2File-macos-arm64.zip"

  cp -f "${win_exe}" "${ASSETS_DIR}/File2File-windows-x64.exe"
  cp -f "${deb_amd64}" "${ASSETS_DIR}/File2File-linux-amd64.deb"
  cp -f "${deb_arm64}" "${ASSETS_DIR}/File2File-linux-arm64.deb"

  log "待上传文件："
  ls -lh "${ASSETS_DIR}"
}

release_notes() {
  cat <<EOF
## File2File ${TAG}

### 下载

| 平台 | 文件 |
|------|------|
| macOS (Apple Silicon) | \`File2File-macos-arm64.zip\` |
| Windows x64 | \`File2File-windows-x64.exe\` |
| Linux amd64 | \`File2File-linux-amd64.deb\` |
| Linux arm64 | \`File2File-linux-arm64.deb\` |

### 安装提示

- **macOS**：解压后打开 \`File2File.app\`。若提示无法验证开发者，可执行：
  \`\`\`bash
  xattr -dr com.apple.quarantine /path/to/File2File.app
  \`\`\`
- **Windows**：直接运行 \`File2File-windows-x64.exe\`
- **Linux**：\`sudo dpkg -i File2File-linux-*.deb\`

个人网盘服务端请使用 [MyWebDisk](https://github.com/xiaoming-software/mywebdisk)。
EOF
}

publish() {
  local repo
  repo="$(resolve_repo)"
  log "仓库: ${repo}"
  log "版本: ${VERSION}  tag: ${TAG}"

  if [[ "${DRY_RUN}" == "1" ]]; then
    log "dry-run：已生成 ${ASSETS_DIR}，跳过 gh release create"
    return 0
  fi

  if gh release view "${TAG}" --repo "${repo}" >/dev/null 2>&1; then
    err "Release ${TAG} 已存在。请改 VERSION，或先删除：gh release delete ${TAG} --repo ${repo} --yes"
    exit 1
  fi

  local args=(
    release create "${TAG}"
    --repo "${repo}"
    --title "${TITLE}"
    --notes "$(release_notes)"
  )
  if [[ "${DRAFT}" == "1" ]]; then
    args+=(--draft)
  fi

  local f
  for f in "${ASSETS_DIR}"/*; do
    [[ -f "${f}" ]] || continue
    args+=("${f}")
  done

  log "正在创建 GitHub Release ${TAG} ..."
  gh "${args[@]}"
  log "发布完成: https://github.com/${repo}/releases/tag/${TAG}"
}

check_prereqs
warn_version_mismatch
prepare_assets
publish
