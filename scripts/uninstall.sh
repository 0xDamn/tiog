#!/bin/sh
set -eu

install_dir="${TIOG_INSTALL_DIR:-$HOME/.local/bin}"
bin="$install_dir/tiog"

echo "tiog uninstaller: target $bin"

if [ -d "$bin" ]; then
  echo "tiog uninstaller: refusing to remove directory: $bin" >&2
  exit 1
fi

if [ -f "$bin" ] || [ -L "$bin" ]; then
  rm -f "$bin"
  echo "tiog uninstaller: removed $bin"
else
  echo "tiog uninstaller: $bin was not found"
fi

cat >&2 <<'EOF'
tiog uninstaller: config and local state were left untouched.
tiog uninstaller: common paths are:
  ~/.config/tiog
  ~/.local/state/tiog
EOF
