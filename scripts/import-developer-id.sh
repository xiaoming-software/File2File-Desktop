#!/usr/bin/env bash
# 把 Apple 后台下载的 Developer ID Application .cer 与本地私钥导入钥匙串。
#
# 用法：
#   1. 在 developer.apple.com 用 signing/*.certSigningRequest 申请并下载 .cer
#   2. 把 .cer 放到 Downloads 或传入路径：
#      ./scripts/import-developer-id.sh ~/Downloads/developerID_application.cer
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SIGN_DIR="$ROOT/signing"
KEY="$SIGN_DIR/developerID_application.key"
CER="${1:-$HOME/Downloads/developerID_application.cer}"

if [[ ! -f "$KEY" ]]; then
  echo "缺少私钥: $KEY" >&2
  exit 1
fi
if [[ ! -f "$CER" ]]; then
  echo "找不到证书文件: $CER" >&2
  echo "请从 Apple Developer 下载 Developer ID Application 证书后重试。" >&2
  exit 1
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# Apple 下载的多为 DER
openssl x509 -inform DER -in "$CER" -out "$TMP/cert.pem" 2>/dev/null \
  || openssl x509 -inform PEM -in "$CER" -out "$TMP/cert.pem"

# 用密码导出 p12（仅本机导入；OpenSSL 3 需 -legacy 才能被 macOS security 接受）
openssl pkcs12 -export -legacy \
  -inkey "$KEY" \
  -in "$TMP/cert.pem" \
  -out "$TMP/developerID.p12" \
  -name "Developer ID Application" \
  -passout pass:file2file-temp

security import "$TMP/developerID.p12" \
  -k ~/Library/Keychains/login.keychain-db \
  -P file2file-temp \
  -T /usr/bin/codesign \
  -T /usr/bin/security \
  -T /usr/bin/productbuild

echo "==> 已导入。当前签名身份："
security find-identity -v -p codesigning | grep -i "Developer ID Application" || {
  echo "未看到 Developer ID Application，请打开「钥匙串访问」确认证书与私钥已成对。" >&2
  security find-identity -v -p codesigning
  exit 1
}

echo
echo "若名称与 tauri.conf.json 里 signingIdentity 不一致，请把配置改成上面显示的完整名称。"
echo "下一步公证（需 App 专用密码）："
echo "  xcrun notarytool store-credentials \"file2file-notary\" --apple-id \"你的AppleID\" --team-id \"B35S9Y7DUX\" --password \"app-specific-password\""
