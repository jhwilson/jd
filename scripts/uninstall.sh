#!/usr/bin/env bash
set -euo pipefail
TARGET_ZSHRC="$HOME/.zshrc"
if grep -q "# BEGIN jd wrapper" "$TARGET_ZSHRC" 2>/dev/null; then
  tmpfile=$(mktemp)
  awk '/# BEGIN jd wrapper/{flag=1; next} /# END jd wrapper/{flag=0; next} !flag{print}' "$TARGET_ZSHRC" > "$tmpfile"
  mv "$tmpfile" "$TARGET_ZSHRC"
  echo "Removed jd wrapper from $TARGET_ZSHRC. Run: source \"$TARGET_ZSHRC\""
else
  echo "No jd wrapper block found in $TARGET_ZSHRC"
fi


