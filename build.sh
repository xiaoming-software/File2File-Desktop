#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LIB_DIR="${PROJECT_ROOT}/lib"
ASSETS_DIR="${PROJECT_ROOT}/assets"
DIST_DIR="${PROJECT_ROOT}/dist"
APP_NAME="file2file"
MAC_APP_NAME="File2File.app"
MAC_EXECUTABLE_NAME="File2File"
WIN_EXE_NAME="File2File.exe"
LINUX_X86_NAME="File2File-linux-x86_64"
LINUX_ARM_NAME="File2File-linux-arm64"
APP_BUNDLE_ID="${APP_BUNDLE_ID:-cn.webrpc.file2file}"
RUSTUP_BIN="${RUSTUP_BIN:-${HOME}/.cargo/bin/rustup}"
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-${PROJECT_ROOT}/target}"
export CARGO_TARGET_DIR

MAC_TARGET="${MAC_TARGET:-aarch64-apple-darwin}"
WINDOWS_TARGET="${WINDOWS_TARGET:-x86_64-pc-windows-gnu}"
LINUX_X86_TARGET="${LINUX_X86_TARGET:-x86_64-unknown-linux-gnu}"
LINUX_ARM_TARGET="${LINUX_ARM_TARGET:-aarch64-unknown-linux-gnu}"

mkdir -p "${DIST_DIR}"
mkdir -p "${CARGO_TARGET_DIR}"

if [[ ! -x "${RUSTUP_BIN}" ]]; then
  echo "[ERROR] 未找到可执行的 rustup: ${RUSTUP_BIN}"
  echo "[ERROR] 请先安装 rustup，或通过 RUSTUP_BIN 指向正确路径。"
  exit 1
fi

RUSTUP_TOOLCHAIN="${RUSTUP_TOOLCHAIN:-$("${RUSTUP_BIN}" show active-toolchain | awk '{print $1}')}"
if [[ -z "${RUSTUP_TOOLCHAIN}" ]]; then
  echo "[ERROR] 无法解析 rustup active toolchain"
  exit 1
fi

RUSTC_BIN="$("${RUSTUP_BIN}" which --toolchain "${RUSTUP_TOOLCHAIN}" rustc)"
CARGO_BIN="$("${RUSTUP_BIN}" which --toolchain "${RUSTUP_TOOLCHAIN}" cargo)"
if [[ ! -x "${RUSTC_BIN}" || ! -x "${CARGO_BIN}" ]]; then
  echo "[ERROR] 无法定位 toolchain 二进制"
  echo "  rustc: ${RUSTC_BIN}"
  echo "  cargo: ${CARGO_BIN}"
  exit 1
fi

require_file() {
  local file="$1"
  if [[ ! -f "${file}" ]]; then
    echo "[ERROR] 缺少文件: ${file}"
    exit 1
  fi
}

required_webrpc_symbols() {
  cat <<'EOF'
WebrpcClient_New
WebrpcClient_LoginStatus
WebrpcClient_GetReceivePort
WebrpcClient_OpenSession
WebrpcClient_SessionSize
WebrpcClient_CloseSession
WebrpcClient_SendData
WebrpcClient_SendFile
WebrpcClient_Free
EOF
}

resolve_nm_tool() {
  if command -v llvm-nm >/dev/null 2>&1; then
    echo "llvm-nm"
    return 0
  fi
  if command -v nm >/dev/null 2>&1; then
    echo "nm"
    return 0
  fi
  echo ""
}

verify_webrpc_sdk() {
  local header_path="$1"
  local static_lib_path="$2"

  require_file "${header_path}"
  require_file "${static_lib_path}"
  if [[ ! -s "${static_lib_path}" ]]; then
    echo "[ERROR] 静态库为空文件: ${static_lib_path}"
    exit 1
  fi

  local nm_tool
  nm_tool="$(resolve_nm_tool)"
  if [[ -z "${nm_tool}" ]]; then
    echo "[ERROR] 未找到 nm/llvm-nm，无法校验 webrpc 静态库导出符号"
    exit 1
  fi

  local symbols_dump
  symbols_dump="$("${nm_tool}" -g "${static_lib_path}" 2>/dev/null || "${nm_tool}" "${static_lib_path}" 2>/dev/null || true)"
  if [[ -z "${symbols_dump}" ]]; then
    echo "[ERROR] 无法读取静态库符号: ${static_lib_path}"
    echo "[ERROR] 请检查库格式是否与当前构建链匹配。"
    exit 1
  fi

  local missing=()
  local symbol
  while IFS= read -r symbol; do
    [[ -z "${symbol}" ]] && continue
    if [[ "${symbols_dump}" != *"${symbol}"* ]]; then
      missing+=("${symbol}")
    fi
  done < <(required_webrpc_symbols)

  if [[ ${#missing[@]} -gt 0 ]]; then
    echo "[ERROR] webrpc 静态库缺少关键符号: ${static_lib_path}"
    for symbol in "${missing[@]}"; do
      echo "  - ${symbol}"
    done
    exit 1
  fi

  echo "[INFO] webrpc SDK 预检通过: $(basename "${header_path}") + $(basename "${static_lib_path}")"
}

copy_runtime_assets() {
  local dest_dir="$1"
  mkdir -p "${dest_dir}/assets"
  if [[ -f "${ASSETS_DIR}/file2file_logo.png" ]]; then
    cp -f "${ASSETS_DIR}/file2file_logo.png" "${dest_dir}/assets/"
  fi
  if [[ -f "${ASSETS_DIR}/file2file_icon.ico" ]]; then
    cp -f "${ASSETS_DIR}/file2file_icon.ico" "${dest_dir}/assets/"
  fi
}

generate_macos_icns() {
  local output_icns="$1"
  local persist_icns="${ASSETS_DIR}/file2file_icon.icns"
  local existing_icns="${ASSETS_DIR}/file2file_icon.icns"
  local source_ico="${ASSETS_DIR}/file2file_icon.ico"
  local fallback_png="${ASSETS_DIR}/file2file_logo.png"

  if [[ -f "${existing_icns}" ]]; then
    cp -f "${existing_icns}" "${output_icns}"
    return 0
  fi

  if ! command -v sips >/dev/null 2>&1 || ! command -v iconutil >/dev/null 2>&1; then
    return 1
  fi

  local temp_dir
  temp_dir="$(mktemp -d)"
  local source_png="${temp_dir}/source.png"
  local iconset_dir="${temp_dir}/AppIcon.iconset"
  mkdir -p "${iconset_dir}"

  if [[ -f "${source_ico}" ]]; then
    sips -s format png "${source_ico}" --out "${source_png}" >/dev/null 2>&1 || true
  fi
  if [[ ! -f "${source_png}" && -f "${fallback_png}" ]]; then
    cp -f "${fallback_png}" "${source_png}"
  fi
  if [[ ! -f "${source_png}" ]]; then
    rm -rf "${temp_dir}"
    return 1
  fi

  for size in 16 32 128 256 512; do
    sips -z "${size}" "${size}" "${source_png}" \
      --out "${iconset_dir}/icon_${size}x${size}.png" >/dev/null
    local double_size=$((size * 2))
    sips -z "${double_size}" "${double_size}" "${source_png}" \
      --out "${iconset_dir}/icon_${size}x${size}@2x.png" >/dev/null
  done

  iconutil -c icns "${iconset_dir}" -o "${output_icns}" >/dev/null 2>&1 || {
    rm -rf "${temp_dir}"
    return 1
  }
  if [[ ! -f "${persist_icns}" ]]; then
    cp -f "${output_icns}" "${persist_icns}" || true
  fi
  rm -rf "${temp_dir}"
  return 0
}

resolve_existing_file() {
  local found=""
  for candidate in "$@"; do
    if [[ -f "${candidate}" ]]; then
      found="${candidate}"
      break
    fi
  done
  if [[ -z "${found}" ]]; then
    echo "[ERROR] 以下文件都不存在:"
    for candidate in "$@"; do
      echo "  - ${candidate}"
    done
    exit 1
  fi
  echo "${found}"
}

target_env_key() {
  echo "$1" | tr '[:lower:]-' '[:upper:]_'
}

resolve_linker_for_target() {
  local target="$1"
  case "${target}" in
    x86_64-unknown-linux-gnu)
      echo "${LINUX_LINKER_X86_64:-x86_64-unknown-linux-gnu-gcc}"
      ;;
    aarch64-unknown-linux-gnu)
      echo "${LINUX_LINKER_AARCH64:-aarch64-unknown-linux-gnu-gcc}"
      ;;
    x86_64-pc-windows-gnu)
      echo "${WINDOWS_LINKER_X86_64:-x86_64-w64-mingw32-gcc}"
      ;;
    aarch64-pc-windows-gnu)
      echo "${WINDOWS_LINKER_AARCH64:-aarch64-w64-mingw32-gcc}"
      ;;
    *)
      echo ""
      ;;
  esac
}

build_target() {
  local target="$1"
  local header="$2"
  local static_lib_path="$3"

  echo "=============================================="
  echo "[INFO] 开始构建 target=${target}"
  echo "[INFO] toolchain=${RUSTUP_TOOLCHAIN}"
  echo "[INFO] 使用头文件: ${header}"
  echo "[INFO] 使用静态库: $(basename "${static_lib_path}")"

  verify_webrpc_sdk "${LIB_DIR}/${header}" "${static_lib_path}"

  "${RUSTUP_BIN}" target add --toolchain "${RUSTUP_TOOLCHAIN}" "${target}"
  local linker
  linker="$(resolve_linker_for_target "${target}")"
  if [[ -n "${linker}" ]]; then
    if ! command -v "${linker}" >/dev/null 2>&1; then
      echo "[ERROR] 未找到目标平台 linker: ${linker}"
      echo "[ERROR] 请先安装交叉编译工具链，或通过环境变量覆盖："
      echo "        LINUX_LINKER_X86_64 / LINUX_LINKER_AARCH64 / WINDOWS_LINKER_X86_64 / WINDOWS_LINKER_AARCH64"
      exit 1
    fi
    local env_key
    env_key="$(target_env_key "${target}")"
    echo "[INFO] 使用 linker: ${linker}"
    env "CARGO_TARGET_${env_key}_LINKER=${linker}" RUSTC="${RUSTC_BIN}" \
      "${CARGO_BIN}" build --release --target "${target}"
  else
    RUSTC="${RUSTC_BIN}" "${CARGO_BIN}" build --release --target "${target}"
  fi
}

package_linux_app() {
  local target="$1"
  local header="$2"
  local static_lib_path="$3"
  local output_name="$4"

  build_target "${target}" "${header}" "${static_lib_path}"

  local out_dir="${DIST_DIR}/${target}"
  local built_bin="${CARGO_TARGET_DIR}/${target}/release/${APP_NAME}"
  local app_dir="${out_dir}/${output_name}"
  local dist_bundle="${DIST_DIR}/${output_name}"
  local dist_launcher="${DIST_DIR}/${output_name}.desktop"
  local launcher_bin="${app_dir}/File2File"
  local desktop_file="${app_dir}/File2File.desktop"
  local icon_png="${PROJECT_ROOT}/assets/file2file_logo.png"

  mkdir -p "${app_dir}" "${out_dir}/sdk"
  cp -f "${built_bin}" "${launcher_bin}"
  chmod +x "${launcher_bin}"
  copy_runtime_assets "${app_dir}"

  if [[ -f "${icon_png}" ]]; then
    cp -f "${icon_png}" "${app_dir}/file2file.png"
  fi

  cat > "${desktop_file}" <<'EOF'
[Desktop Entry]
Version=1.0
Type=Application
Name=File2File
Comment=File2File Desktop App
Exec=sh -c 'DIR="$(dirname "$1")"; exec "$DIR/File2File"' _ %k
Terminal=false
StartupNotify=true
Categories=Network;Utility;
Icon=file2file.png
EOF
  chmod +x "${desktop_file}"

  cp -f "${LIB_DIR}/${header}" "${out_dir}/sdk/"
  cp -f "${static_lib_path}" "${out_dir}/sdk/"

  rm -rf "${dist_bundle}"
  cp -R "${app_dir}" "${dist_bundle}"
  cat > "${dist_launcher}" <<EOF
[Desktop Entry]
Version=1.0
Type=Application
Name=File2File
Comment=File2File Desktop App
Exec=sh -c 'DIR="\$(dirname "\$1")"; exec "\$DIR/${output_name}/File2File"' _ %k
Terminal=false
StartupNotify=true
Categories=Network;Utility;
Icon=${output_name}/file2file.png
EOF
  chmod +x "${dist_launcher}"
  echo "[OK] Linux 桌面包已输出: ${dist_bundle}"
}

package_macos_app() {
  local target="$1"
  local header="$2"
  local static_lib_path="$3"

  build_target "${target}" "${header}" "${static_lib_path}"

  local target_out="${DIST_DIR}/${target}"
  local app_dir="${target_out}/${MAC_APP_NAME}"
  local contents_dir="${app_dir}/Contents"
  local macos_dir="${contents_dir}/MacOS"
  local resources_dir="${contents_dir}/Resources"
  local built_bin="${CARGO_TARGET_DIR}/${target}/release/${APP_NAME}"
  local icon_icns="${resources_dir}/file2file_icon.icns"

  mkdir -p "${macos_dir}" "${resources_dir}"
  cp -f "${built_bin}" "${macos_dir}/${MAC_EXECUTABLE_NAME}"
  chmod +x "${macos_dir}/${MAC_EXECUTABLE_NAME}"
  copy_runtime_assets "${resources_dir}"

  generate_macos_icns "${icon_icns}" || true

  cat > "${contents_dir}/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDisplayName</key>
  <string>File2File</string>
  <key>CFBundleExecutable</key>
  <string>${MAC_EXECUTABLE_NAME}</string>
  <key>CFBundleIdentifier</key>
  <string>${APP_BUNDLE_ID}</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>File2File</string>
  <key>CFBundleIconFile</key>
  <string>file2file_icon</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>1.0.0</string>
  <key>CFBundleVersion</key>
  <string>1.0.0</string>
  <key>LSMinimumSystemVersion</key>
  <string>11.0</string>
</dict>
</plist>
EOF

  if command -v codesign >/dev/null 2>&1; then
    codesign --force --deep --sign - "${app_dir}" >/dev/null
  fi
  touch "${app_dir}"

  rm -rf "${DIST_DIR}/${MAC_APP_NAME}"
  cp -R "${app_dir}" "${DIST_DIR}/${MAC_APP_NAME}"
  touch "${DIST_DIR}/${MAC_APP_NAME}/Contents/Info.plist" "${DIST_DIR}/${MAC_APP_NAME}"

  mkdir -p "${target_out}/sdk"
  cp -f "${LIB_DIR}/${header}" "${target_out}/sdk/"
  cp -f "${static_lib_path}" "${target_out}/sdk/"
  echo "[OK] macOS .app 打包完成: ${app_dir}"
}

package_windows_exe() {
  local target="$1"
  local header="$2"
  local static_lib_path="$3"

  build_target "${target}" "${header}" "${static_lib_path}"

  local out_dir="${DIST_DIR}/${target}"
  mkdir -p "${out_dir}"
  cp -f "${CARGO_TARGET_DIR}/${target}/release/${APP_NAME}.exe" "${out_dir}/${WIN_EXE_NAME}"
  copy_runtime_assets "${out_dir}"
  cp -f "${out_dir}/${WIN_EXE_NAME}" "${DIST_DIR}/${WIN_EXE_NAME}"
  copy_runtime_assets "${DIST_DIR}"
  mkdir -p "${out_dir}/sdk"
  cp -f "${LIB_DIR}/${header}" "${out_dir}/sdk/"
  cp -f "${static_lib_path}" "${out_dir}/sdk/"
  echo "[OK] Windows 可执行文件已输出: ${out_dir}/${WIN_EXE_NAME}"
}

cd "${PROJECT_ROOT}"

MAC_LIB="$(resolve_existing_file "${LIB_DIR}/libwebrpc-Mac.a")"
LINUX_X86_LIB="$(resolve_existing_file "${LIB_DIR}/libwebrpc-Linux.a")"
LINUX_ARM_LIB="$(resolve_existing_file "${LIB_DIR}/libwebrpc-Linux-arm64.a")"
WINDOWS_LIB="$(resolve_existing_file "${LIB_DIR}/libwebrpc-Windows.a" "${LIB_DIR}/libwebrpc-Windows.lib")"

package_macos_app "${MAC_TARGET}" "libwebrpc-Mac.h" "${MAC_LIB}"
package_linux_app "${LINUX_X86_TARGET}" "libwebrpc-Linux.h" "${LINUX_X86_LIB}" "${LINUX_X86_NAME}"
package_linux_app "${LINUX_ARM_TARGET}" "libwebrpc-Linux-arm64.h" "${LINUX_ARM_LIB}" "${LINUX_ARM_NAME}"
package_windows_exe "${WINDOWS_TARGET}" "libwebrpc-Windows.h" "${WINDOWS_LIB}"

echo "=============================================="
echo "[SUCCESS] 构建完成。输出如下："
echo "  - ${DIST_DIR}/${MAC_APP_NAME}"
echo "  - ${DIST_DIR}/${LINUX_X86_NAME}"
echo "  - ${DIST_DIR}/${LINUX_ARM_NAME}"
echo "  - ${DIST_DIR}/${WIN_EXE_NAME}"
