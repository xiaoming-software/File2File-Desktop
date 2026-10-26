#!/usr/bin/env bash
# 对已签名的 File2File.app 做 Apple 公证并 staple。
#
# 前置（一次性）：
#   xcrun notarytool store-credentials "file2file-notary" \
#     --apple-id "你的AppleID" \
#     --team-id "B35S9Y7DUX" \
#     --password "app-specific-password"
#
# 用法：
#   ./scripts/notarize-mac.sh
#   ./scripts/notarize-mac.sh /path/to/File2File.app
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APP="${1:-}"
if [[ -z "$APP" ]]; then
  for cand in \
    "$ROOT/dist-tauri/macos/File2File.app" \
    "$ROOT/dist-tauri/File2File.app" \
    "$ROOT/src-tauri/target/release/bundle/macos/File2File.app"
  do
    if [[ -d "$cand" ]]; then APP="$cand"; break; fi
  done
fi

PROFILE="${NOTARY_PROFILE:-file2file-notary}"
IDENTITY="${SIGNING_IDENTITY:-Developer ID Application: Mingding Li (B35S9Y7DUX)}"

if [[ -z "${APP}" || ! -d "$APP" ]]; then
  echo "找不到 File2File.app，请先构建或传入路径。" >&2
  exit 1
fi

echo "==> App: $APP"
echo "==> 检查签名身份..."
security find-identity -v -p codesigning | grep -F "$IDENTITY" >/dev/null \
  || { echo "钥匙串中没有: $IDENTITY" >&2; exit 1; }

# 若尚未用 Developer ID 签过，这里补签（含子项）
echo "==> codesign (Developer ID + Hardened Runtime)..."
ENTITLEMENTS="$ROOT/src-tauri/Entitlements.plist"
codesign --force --deep --options runtime --timestamp \
  --entitlements "$ENTITLEMENTS" \
  --sign "$IDENTITY" \
  "$APP"

codesign -dv --verbose=4 "$APP" 2>&1 | head -20

ZIP="$(mktemp -t File2File-notarize).zip"
rm -f "$ZIP"
echo "==> 打包 zip: $ZIP"
ditto -c -k --keepParent "$APP" "$ZIP"

echo "==> 提交公证 (profile=$PROFILE)..."
xcrun notarytool submit "$ZIP" --keychain-profile "$PROFILE" --wait

echo "==> staple..."
xcrun stapler staple "$APP"
xcrun stapler validate "$APP"
spctl -a -vv "$APP" || true

rm -f "$ZIP"
echo "==> 完成。可把该 .app 打 zip/dmg 后发布。"
