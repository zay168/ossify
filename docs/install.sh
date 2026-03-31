#!/bin/sh

set -eu

REPO="zay168/ossify"
VERSION="${OSSIFY_VERSION:-}"
INSTALL_DIR="${OSSIFY_INSTALL_DIR:-$HOME/.local/bin}"
TOOLS_DIR="${OSSIFY_TOOLS_DIR:-$HOME/.local/share/ossify/tools/bin}"
ACTIONLINT_REPO="rhysd/actionlint"
ACTIONLINT_VERSION="${OSSIFY_ACTIONLINT_VERSION:-}"

need_cmd() {
  if command -v "$1" >/dev/null 2>&1; then
    return 0
  fi

  echo "Missing required command: $1" >&2
  exit 1
}

detect_target() {
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Linux)
      platform="unknown-linux-gnu"
      ;;
    Darwin)
      platform="apple-darwin"
      ;;
    *)
      echo "Unsupported operating system: $os" >&2
      exit 1
      ;;
  esac

  case "$arch" in
    x86_64|amd64)
      target_arch="x86_64"
      ;;
    *)
      echo "This installer currently ships macOS/Linux builds for x64 only. Detected architecture: $arch" >&2
      exit 1
      ;;
  esac

  printf '%s-%s' "$target_arch" "$platform"
}

download_url() {
  asset="$1"

  if [ -n "$VERSION" ]; then
    case "$VERSION" in
      v*)
        tag="$VERSION"
        ;;
      *)
        tag="v$VERSION"
        ;;
    esac
    printf 'https://github.com/%s/releases/download/%s/%s' "$REPO" "$tag" "$asset"
    return
  fi

  printf 'https://github.com/%s/releases/latest/download/%s' "$REPO" "$asset"
}

latest_release_version() {
  repository="$1"
  api_url="https://api.github.com/repos/$repository/releases/latest"

  if command -v curl >/dev/null 2>&1; then
    payload="$(curl -fsSL -H 'Accept: application/vnd.github+json' -H 'User-Agent: ossify-installer' "$api_url")"
  elif command -v wget >/dev/null 2>&1; then
    payload="$(wget -qO- --header='Accept: application/vnd.github+json' --header='User-Agent: ossify-installer' "$api_url")"
  else
    echo "Install requires curl or wget." >&2
    exit 1
  fi

  version="$(printf '%s' "$payload" | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"v\{0,1\}\([^"]*\)".*/\1/p' | head -n 1)"
  if [ -z "$version" ]; then
    echo "Could not resolve latest release version for $repository" >&2
    exit 1
  fi

  printf '%s' "$version"
}

download_file() {
  url="$1"
  destination="$2"

  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$url" -o "$destination"
    return
  fi

  if command -v wget >/dev/null 2>&1; then
    wget -qO "$destination" "$url"
    return
  fi

  echo "Install requires curl or wget." >&2
  exit 1
}

actionlint_asset() {
  version="$1"
  os="$(uname -s)"

  case "$os" in
    Linux)
      printf 'actionlint_%s_linux_amd64.tar.gz' "$version"
      ;;
    Darwin)
      printf 'actionlint_%s_darwin_amd64.tar.gz' "$version"
      ;;
    *)
      echo "Unsupported operating system for actionlint: $os" >&2
      exit 1
      ;;
  esac
}

target="$(detect_target)"
asset="ossify-$target.tar.gz"
url="$(download_url "$asset")"
resolved_actionlint_version="${ACTIONLINT_VERSION:-}"
if [ -z "$resolved_actionlint_version" ]; then
  resolved_actionlint_version="$(latest_release_version "$ACTIONLINT_REPO")"
fi
actionlint_asset_name="$(actionlint_asset "$resolved_actionlint_version")"
actionlint_url="https://github.com/$ACTIONLINT_REPO/releases/download/v$resolved_actionlint_version/$actionlint_asset_name"

tmpdir="$(mktemp -d)"
archive="$tmpdir/$asset"
actionlint_archive="$tmpdir/$actionlint_asset_name"
trap 'rm -rf "$tmpdir"' EXIT HUP INT TERM

need_cmd tar
need_cmd mkdir
need_cmd chmod
need_cmd cp

echo "Downloading ossify from $url"
download_file "$url" "$archive"

mkdir -p "$INSTALL_DIR"
tar -xzf "$archive" -C "$tmpdir"
cp "$tmpdir/ossify" "$INSTALL_DIR/ossify"
chmod +x "$INSTALL_DIR/ossify"

echo "Downloading managed workflow engine from $actionlint_url"
download_file "$actionlint_url" "$actionlint_archive"
mkdir -p "$TOOLS_DIR"
tar -xzf "$actionlint_archive" -C "$tmpdir"
cp "$tmpdir/actionlint" "$TOOLS_DIR/actionlint"
chmod +x "$TOOLS_DIR/actionlint"

echo
echo "ossify installed successfully."
echo "Binary: $INSTALL_DIR/ossify"
echo "Managed tools: $TOOLS_DIR/actionlint"

case ":$PATH:" in
  *":$INSTALL_DIR:"*)
    echo "Next: ossify version"
    ;;
  *)
    echo "Add this directory to your shell PATH if needed:"
    echo "  export PATH=\"$INSTALL_DIR:\$PATH\""
    echo "Then run: ossify version"
    ;;
esac
