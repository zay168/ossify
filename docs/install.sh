#!/bin/sh

set -eu

REPO="zay168/ossify"
VERSION="${OSSIFY_VERSION:-}"
INSTALL_DIR="${OSSIFY_INSTALL_DIR:-$HOME/.local/bin}"

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

target="$(detect_target)"
asset="ossify-$target.tar.gz"
url="$(download_url "$asset")"

tmpdir="$(mktemp -d)"
archive="$tmpdir/$asset"
trap 'rm -rf "$tmpdir"' EXIT HUP INT TERM

need_cmd tar
need_cmd mkdir
need_cmd chmod

echo "Downloading ossify from $url"
download_file "$url" "$archive"

mkdir -p "$INSTALL_DIR"
tar -xzf "$archive" -C "$tmpdir"
cp "$tmpdir/ossify" "$INSTALL_DIR/ossify"
chmod +x "$INSTALL_DIR/ossify"

echo
echo "ossify installed successfully."
echo "Binary: $INSTALL_DIR/ossify"

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
