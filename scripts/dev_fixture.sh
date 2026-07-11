#!/usr/bin/env bash
set -euo pipefail
dest=/tmp/jd_fixture/T99_Test_Root
rm -rf /tmp/jd_fixture
mkdir -p "$dest"
while IFS=$'\t' read -r kind rel; do
  [[ -z "${rel:-}" ]] && continue
  case "$kind" in
    D) mkdir -p "$dest/$rel" ;;
    F) mkdir -p "$(dirname "$dest/$rel")"; touch "$dest/$rel" ;;
  esac
done < "$(dirname "$0")/../tests/fixtures/T99_tree.txt"
bash "$(dirname "$0")/../tests/fixtures/T99_contents.sh" "$dest"
echo 'JD_ROOTS=/tmp/jd_fixture/T99_Test_Root jd'
