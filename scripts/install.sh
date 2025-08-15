#!/usr/bin/env bash
set -euo pipefail

WRAPPER_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WRAPPER_FILE="$WRAPPER_DIR/jd.zsh"
TARGET_ZSHRC="$HOME/.zshrc"

if ! grep -q "# BEGIN jd wrapper" "$TARGET_ZSHRC" 2>/dev/null; then
  {
    echo "# BEGIN jd wrapper"
    echo "source \"$WRAPPER_FILE\""
    echo "# END jd wrapper"
  } >> "$TARGET_ZSHRC"
  echo "Installed jd wrapper to $TARGET_ZSHRC. Run: source \"$TARGET_ZSHRC\""
else
  echo "jd wrapper already installed in $TARGET_ZSHRC"
fi


