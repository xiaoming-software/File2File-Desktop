#!/usr/bin/env bash
# 在本机编 macOS，在 Docker 里编 Linux / Windows。
# Docker 无法编 macOS（没有 Apple SDK），所以不能四端都进容器。
#
# 用法：
#   ./build-all.sh              # 全部
#   ./build-all.sh macos
#   ./build-all.sh linux-arm64
#   ./build-all.sh linux-amd64
#   ./build-all.sh windows
#
# 产物：dist-tauri/<平台>/

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DIST_DIR="${PROJECT_ROOT}/dist-tauri"
LINUX_IMAGE="file2file-tauri-linux"
WINDOWS_IMAGE="file2file-tauri-windows"

cd "${PROJECT_ROOT}"
export PATH="${HOME}/.cargo/bin:${PATH}"

log() { echo "[INFO] $*"; }
err() { echo "[ERROR] $*" >&2; }

prepare_frontend() {
  # 复用本机脚本里的前端同步与图标检查，但不编 macOS。
  if [[ ! -f "${PROJECT_ROOT}/index.html" ]]; then
    err "找不到 ${PROJECT_ROOT}/index.html"
    exit 1
  fi
  local ui_dir="${PROJECT_ROOT}/ui"
  mkdir -p "${ui_dir}/css" "${ui_dir}/js" "${ui_dir}/assets"
  cp -f "${PROJECT_ROOT}/index.html" "${ui_dir}/index.html"
  cp -f "${PROJECT_ROOT}/screenshot.html" "${ui_dir}/screenshot.html"
  cp -f "${PROJECT_ROOT}/css/app.css" "${ui_dir}/css/app.css"
  cp -f "${PROJECT_ROOT}/css/screenshot.css" "${ui_dir}/css/screenshot.css"
  cp -f "${PROJECT_ROOT}/js/app.js" "${ui_dir}/js/app.js"
  cp -f "${PROJECT_ROOT}/js/screenshot.js" "${ui_dir}/js/screenshot.js"
  if [[ -f "${PROJECT_ROOT}/assets/file2file_logo.png" ]]; then
    cp -f "${PROJECT_ROOT}/assets/file2file_logo.png" "${ui_dir}/assets/file2file_logo.png"
  fi
  log "已同步前端到 ${ui_dir}"

  local icons="${PROJECT_ROOT}/src-tauri/icons"
  local missing=0
  for f in 32x32.png 128x128.png "128x128@2x.png" icon.icns icon.ico; do
    if [[ ! -f "${icons}/${f}" ]]; then
      err "缺少图标: ${icons}/${f}"
      missing=1
    fi
  done
  if [[ "${missing}" -ne 0 ]]; then
    exit 1
  fi
}

ensure_docker() {
  if ! command -v docker >/dev/null 2>&1; then
    err "未找到 docker。"
    exit 1
  fi
  if ! docker info >/dev/null 2>&1; then
    err "Docker 未运行，请先启动 Docker Desktop。"
    exit 1
  fi
}

build_macos() {
  log "编译 macOS（本机，Docker 编不了 macOS）..."
  "${PROJECT_ROOT}/build-tauri.sh"
  mkdir -p "${DIST_DIR}/macos"
  if [[ -d "${DIST_DIR}/File2File.app" ]]; then
    rm -rf "${DIST_DIR}/macos/File2File.app"
    cp -R "${DIST_DIR}/File2File.app" "${DIST_DIR}/macos/File2File.app"
    log "macOS: ${DIST_DIR}/macos/File2File.app"
    return 0
  fi
  err "macOS 产物未找到"
  return 1
}

build_linux_image() {
  local platform="$1"
  local tag="$2"
  local base_image="$3"
  log "构建 Linux 镜像 ${tag} (${platform} / ${base_image})..."
  docker build \
    --progress=plain \
    --platform "${platform}" \
    --build-arg BASE_IMAGE="${base_image}" \
    -t "${tag}" \
    -f "${PROJECT_ROOT}/docker/Dockerfile.linux" \
    "${PROJECT_ROOT}/docker" || return 1
}

build_windows_image() {
  log "构建 Windows 交叉编译镜像 ${WINDOWS_IMAGE} (linux/arm64)..."
  docker build \
    --progress=plain \
    --platform linux/arm64 \
    -t "${WINDOWS_IMAGE}" \
    -f "${PROJECT_ROOT}/docker/Dockerfile.windows" \
    "${PROJECT_ROOT}/docker" || return 1
}

docker_linux_build() {
  local platform="$1"
  local rust_target="$2"
  local dist_name="$3"
  local image_tag="${LINUX_IMAGE}:${dist_name}"
  local volume="file2file-target-${dist_name}"
  local base_image="golang:1.25-bookworm"
  if [[ "${dist_name}" == "linux-amd64" ]]; then
    base_image="file2file-desktop-linux-amd64-builder:24.04"
  fi

  build_linux_image "${platform}" "${image_tag}" "${base_image}" || return 1

  log "Docker 编译 ${dist_name} (${platform} / ${rust_target})..."
  if [[ "${platform}" == "linux/amd64" ]]; then
    log "Apple Silicon 上编 amd64 会走 QEMU，第一次可能要很久。"
  fi

  docker run --rm \
    --platform "${platform}" \
    --name "file2file-build-${dist_name}" \
    -e CARGO_TERM_COLOR=always \
    -e CARGO_HOME="/root/.cargo" \
    -e CARGO_TARGET_DIR="/cache/target" \
    -e CI=true \
    -v "${PROJECT_ROOT}:/work" \
    -v "file2file-cargo-registry:/root/.cargo/registry" \
    -v "file2file-cargo-git:/root/.cargo/git" \
    -v "${volume}:/cache/target" \
    -w /work \
    "${image_tag}" \
    bash -lc "set -euo pipefail
      git config --global --add safe.directory /work || true
      cd /work/src-tauri
      rustup target add ${rust_target} >/dev/null
      cargo tauri build --ci --target ${rust_target} --bundles deb
    " || return 1

  mkdir -p "${DIST_DIR}/${dist_name}"
  docker run --rm \
    --platform "${platform}" \
    -v "${PROJECT_ROOT}:/work" \
    -v "${volume}:/cache/target" \
    -w /work \
    "${image_tag}" \
    bash -lc "set -euo pipefail
      mkdir -p /work/dist-tauri/${dist_name}
      shopt -s nullglob
      files=(/cache/target/${rust_target}/release/bundle/deb/*.deb)
      if [[ \${#files[@]} -eq 0 ]]; then
        echo '未找到 deb 产物' >&2
        ls -la /cache/target/${rust_target}/release/bundle || true
        exit 1
      fi
      cp -f \"\${files[@]}\" /work/dist-tauri/${dist_name}/
      ls -la /work/dist-tauri/${dist_name}
    " || return 1
  log "Linux ${dist_name}: ${DIST_DIR}/${dist_name}"
}

docker_windows_build() {
  build_windows_image || return 1

  log "Docker 交叉编译 Windows x64（MSVC，静态链 WebView2Loader + webrpc .a）..."
  docker run --rm \
    --platform linux/arm64 \
    --name file2file-build-windows \
    -e CARGO_TERM_COLOR=always \
    -e CARGO_HOME="/root/.cargo" \
    -e CARGO_TARGET_DIR="/cache/target" \
    -e XWIN_CACHE_DIR="/xwin-cache" \
    -e CI=true \
    -v "${PROJECT_ROOT}:/work" \
    -v "file2file-cargo-registry:/root/.cargo/registry" \
    -v "file2file-cargo-git:/root/.cargo/git" \
    -v "file2file-target-windows:/cache/target" \
    -v "file2file-xwin-cache:/xwin-cache" \
    -w /work \
    "${WINDOWS_IMAGE}" \
    bash -lc "set -euo pipefail
      git config --global --add safe.directory /work || true
      cd /work/src-tauri
      cargo tauri build --ci --runner cargo-xwin --target x86_64-pc-windows-msvc --no-bundle
    " || return 1

  docker run --rm \
    --platform linux/arm64 \
    -v "${PROJECT_ROOT}:/work" \
    -v "file2file-target-windows:/cache/target" \
    -w /work \
    "${WINDOWS_IMAGE}" \
    bash -lc "set -euo pipefail
      mkdir -p /work/dist-tauri/windows-x64
      rm -f /work/dist-tauri/windows-x64/*.dll /work/dist-tauri/windows-x64/*.exe
      src=/cache/target/x86_64-pc-windows-msvc/release/file2file-tauri.exe
      if [[ ! -f \"\${src}\" ]]; then
        echo '未找到 Windows exe' >&2
        ls -la /cache/target/x86_64-pc-windows-msvc/release || true
        exit 1
      fi
      cp -f \"\${src}\" /work/dist-tauri/windows-x64/File2File.exe
      echo '=== 依赖的 DLL（不应再有 WebView2Loader.dll）==='
      x86_64-w64-mingw32-objdump -p /work/dist-tauri/windows-x64/File2File.exe | grep -i 'DLL Name' || true
      ls -la /work/dist-tauri/windows-x64
    " || return 1
  log "Windows x64: ${DIST_DIR}/windows-x64/File2File.exe"
}

usage() {
  cat <<EOF
用法: ./build-all.sh [目标...]

目标:
  macos         本机编译 macOS .app
  linux-arm64   Docker 编译 Linux ARM64 .deb
  linux-amd64   Docker 编译 Linux AMD64 .deb（QEMU，慢；镜像缓存后会快很多）
  windows       Docker 交叉编译 Windows x64 单文件 exe（静态链 webrpc .a + WebView2Loader）
  all           以上全部（默认）

说明:
  macOS 不能在 Linux 容器里编译，必须走本机 Xcode/SDK。
  Linux / Windows 在 Docker 中编译。
EOF
}

TARGETS=("${@:-all}")
if [[ "${#TARGETS[@]}" -eq 1 && "${TARGETS[0]}" == "all" ]]; then
  TARGETS=(macos linux-arm64 windows linux-amd64)
fi
if [[ "${TARGETS[0]:-}" == "-h" || "${TARGETS[0]:-}" == "--help" ]]; then
  usage
  exit 0
fi

prepare_frontend
mkdir -p "${DIST_DIR}"

NEED_DOCKER=0
for t in "${TARGETS[@]}"; do
  case "$t" in
    linux-arm64|linux-amd64|windows) NEED_DOCKER=1 ;;
    macos) ;;
    *) err "未知目标: $t"; usage; exit 1 ;;
  esac
done
if [[ "${NEED_DOCKER}" -eq 1 ]]; then
  ensure_docker
fi

FAILED=()
set +e
for t in "${TARGETS[@]}"; do
  log "======== 开始: ${t} ========"
  case "$t" in
    macos) build_macos ;;
    linux-arm64) docker_linux_build linux/arm64 aarch64-unknown-linux-gnu linux-arm64 ;;
    linux-amd64) docker_linux_build linux/amd64 x86_64-unknown-linux-gnu linux-amd64 ;;
    windows) docker_windows_build ;;
  esac
  if [[ $? -ne 0 ]]; then
    FAILED+=("$t")
  fi
done
set -e

echo
if [[ "${#FAILED[@]}" -gt 0 ]]; then
  err "失败: ${FAILED[*]}"
  exit 1
fi
log "全部完成。产物目录: ${DIST_DIR}"
ls -la "${DIST_DIR}" || true
for d in macos linux-arm64 linux-amd64 windows-x64; do
  if [[ -d "${DIST_DIR}/${d}" ]]; then
    echo "  ${d}:"
    ls -la "${DIST_DIR}/${d}" | sed 's/^/    /'
  fi
done
