#!/usr/bin/env bash
# Builds the desktop release files (Tauri app, desktop-tauri/) into desktop-tauri/dist/:
#   DeviceDeck<version>-windows-x64.zip   quickbuds.exe only (portable, static runtime)
#   DeviceDeck-linux-x64.AppImage         WebKitGTK inside. No version in the name: releases/latest/download/ always finds it.
# Usage: scripts/desktop-dist.sh <version>   (e.g. 5.7.0). QB_APPIMAGE_ONLY=1 builds the AppImage only.
# Needs: podman (unless QB_NATIVE=1), zip. Windows also needs cargo-xwin, clang, lld, llvm and the msvc rustup target.
# Linux builds in Ubuntu 24.04 (glibc 2.39): its WebKitGTK renders the transparent window cleanly (the 22.04 one ghosts text).
# The podman image (scripts/desktop-build.Containerfile) is built once.
set -euo pipefail
ver=${1:?version}
root=$(cd "$(dirname "$0")/.." && pwd)
app=$root/desktop-tauri
dist=$app/dist
rm -rf "$dist" && mkdir -p "$dist"

# Windows, in the background while Linux builds; its output goes to a log.
if [ -z "${QB_APPIMAGE_ONLY:-}" ]; then
  winlog=$app/target/windows-build.log
  mkdir -p "$app/target"
  (
    cd "$app"
    XWIN_ACCEPT_LICENSE=1 RUSTFLAGS="-C target-feature=+crt-static -C link-arg=libucrt.lib -C link-arg=libvcruntime.lib" \
      cargo xwin build --release --target x86_64-pc-windows-msvc
    cd target/x86_64-pc-windows-msvc/release && zip -q "$dist/DeviceDeck$ver-windows-x64.zip" quickbuds.exe
  ) >"$winlog" 2>&1 &
  winpid=$!
fi

# Linux. QB_NATIVE=1 builds in place, for CI, which already runs in Ubuntu 24.04 with the packages of the Containerfile.
build='cargo tauri build --bundles appimage'
build_linux() {
  if [ -n "${QB_NATIVE:-}" ]; then
    (cd "$app" && CARGO_TARGET_DIR=target/ubuntu22 APPIMAGE_EXTRACT_AND_RUN=1 NO_STRIP=1 $build)
  else
    img=quickbuds-build:tauri
    want=$(sha256sum "$root/scripts/desktop-build.Containerfile" | cut -c1-12)
    [ "$(podman image inspect -f '{{index .Labels "cf"}}' "$img" 2>/dev/null)" = "$want" ] ||
      podman build -q -t "$img" --label "cf=$want" -f "$root/scripts/desktop-build.Containerfile" "$root/scripts" >/dev/null
    podman run --rm --userns=keep-id -v "$root:/repo" -v quickbuds-cargo:/cargo-reg -w /repo/desktop-tauri --user root \
      -e CARGO_HOME=/cargo-reg -e CARGO_TARGET_DIR=/repo/desktop-tauri/target/ubuntu22 -e APPIMAGE_EXTRACT_AND_RUN=1 -e NO_STRIP=1 \
      "$img" bash -c '
        set -e
        export PATH=/cargo/bin:$PATH
        '"$build"'
        chown -R '"$(id -u):$(id -g)"' /repo/desktop-tauri/target/ubuntu22 /cargo-reg'
  fi
}

# Tauri's AppImage bundler expects AppRun in its cache. Keep the pinned runtime
# in source so builds do not depend on GitHub release asset availability.
if [ -n "${QB_NATIVE:-}" ]; then
  tauri_cache=${XDG_CACHE_HOME:-"$HOME/.cache"}/tauri
  apprun="$tauri_cache/AppRun-x86_64"
  apprun_sha256=f30140a43a0a59e46db21bdefdf749b9e9f2c6946e92afabbacf98b8ae73fb4f
  if ! [ -f "$apprun" ] || ! printf '%s  %s\n' "$apprun_sha256" "$apprun" | sha256sum --check --status; then
    bundled_apprun="$root/scripts/vendor/AppRun-x86_64"
    printf '%s  %s\n' "$apprun_sha256" "$bundled_apprun" | sha256sum --check --status
    mkdir -p "$tauri_cache"
    install -m 0755 "$bundled_apprun" "$apprun.tmp"
    mv -f "$apprun.tmp" "$apprun"
  fi
fi

# Tauri downloads AppRun while bundling. Retry the packaging step for transient
# upstream errors (e.g. GitHub Assets returning HTTP 504), keeping the build failure
# visible if all attempts fail.
max_attempts=3
for attempt in $(seq 1 "$max_attempts"); do
  if build_linux; then
    break
  fi
  if [ "$attempt" -eq "$max_attempts" ]; then
    echo "AppImage packaging failed after $max_attempts attempts." >&2
    exit 1
  fi
  echo "AppImage packaging attempt $attempt failed; retrying in $((attempt * 10))s." >&2
  sleep $((attempt * 10))
done
cp "$app"/target/ubuntu22/release/bundle/appimage/*.AppImage "$dist/DeviceDeck-linux-x64.AppImage"
if [ -n "${winpid:-}" ]; then wait "$winpid" || { cat "$winlog"; exit 1; }; tail -2 "$winlog"; fi
ls -la "$dist"
