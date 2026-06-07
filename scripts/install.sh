#!/bin/sh
set -eu

repo="${TIOG_REPO:-0xDamn/tiog}"
version="${TIOG_VERSION:-latest}"
install_dir="${TIOG_INSTALL_DIR:-$HOME/.local/bin}"

need() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "tiog installer: missing required command: $1" >&2
    exit 1
  fi
}

detect_target() {
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Linux) os_part="unknown-linux-gnu" ;;
    Darwin) os_part="apple-darwin" ;;
    *)
      echo "tiog installer: unsupported OS: $os" >&2
      exit 1
      ;;
  esac

  case "$arch" in
    x86_64 | amd64) arch_part="x86_64" ;;
    arm64 | aarch64) arch_part="aarch64" ;;
    *)
      echo "tiog installer: unsupported architecture: $arch" >&2
      exit 1
      ;;
  esac

  printf '%s-%s' "$arch_part" "$os_part"
}

download() {
  url="$1"
  out="$2"

  if command -v curl >/dev/null 2>&1; then
    curl --proto '=https' --tlsv1.2 -fsSL "$url" -o "$out"
  elif command -v wget >/dev/null 2>&1; then
    wget -q "$url" -O "$out"
  else
    echo "tiog installer: curl or wget is required" >&2
    exit 1
  fi
}

base_url() {
  if [ "$version" = "latest" ]; then
    printf 'https://github.com/%s/releases/latest/download' "$repo"
  else
    printf 'https://github.com/%s/releases/download/%s' "$repo" "$version"
  fi
}

verify_checksum() {
  checksums="$1"
  archive="$2"
  archive_path="$3"

  expected="$(grep "  $archive\$" "$checksums" | awk '{print $1}')"
  if [ -z "$expected" ]; then
    echo "tiog installer: checksum not found for $archive" >&2
    exit 1
  fi

  if command -v sha256sum >/dev/null 2>&1; then
    printf '%s  %s\n' "$expected" "$archive_path" | sha256sum -c - >/dev/null
  elif command -v shasum >/dev/null 2>&1; then
    printf '%s  %s\n' "$expected" "$archive_path" | shasum -a 256 -c - >/dev/null
  else
    echo "tiog installer: sha256sum or shasum is required for checksum verification" >&2
    exit 1
  fi
}

tmux_hint() {
  if ! command -v tmux >/dev/null 2>&1; then
    echo "tiog installer: note: tmux was not found" >&2
    echo "tiog installer: tiog still works, but tmux is recommended for full terminal context capture" >&2
    return
  fi

  if [ -z "${TMUX:-}" ]; then
    echo "tiog installer: note: tmux is installed, but this shell is not inside tmux" >&2
    echo "tiog installer: run tiog inside a tmux session for the best command/output context" >&2
  fi
}

target="$(detect_target)"
archive="tiog-$target.tar.gz"
url_base="$(base_url)"
tmp="${TMPDIR:-/tmp}/tiog-install.$$"

cleanup() {
  rm -rf "$tmp"
}
trap cleanup EXIT INT HUP TERM

mkdir -p "$tmp"

echo "tiog installer: downloading $archive"
download "$url_base/$archive" "$tmp/$archive"
download "$url_base/checksums.txt" "$tmp/checksums.txt"
verify_checksum "$tmp/checksums.txt" "$archive" "$tmp/$archive"

need tar
need install
tar -xzf "$tmp/$archive" -C "$tmp"

if [ ! -f "$tmp/tiog" ]; then
  echo "tiog installer: archive did not contain tiog binary" >&2
  exit 1
fi

mkdir -p "$install_dir"
install -m 755 "$tmp/tiog" "$install_dir/tiog"

echo "tiog installer: installed $install_dir/tiog"
case ":$PATH:" in
  *":$install_dir:"*) ;;
  *)
    echo "tiog installer: warning: $install_dir is not in PATH" >&2
    echo "tiog installer: add this to your shell profile:" >&2
    echo "  export PATH=\"$install_dir:\$PATH\"" >&2
    ;;
esac

tmux_hint
