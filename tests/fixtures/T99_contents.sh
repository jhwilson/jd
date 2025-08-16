#!/usr/bin/env bash
set -euo pipefail
DEST="${1:-.}"
DECODE_OPT="--decode"
if base64 --help 2>&1 | grep -q -- ' -D '; then DECODE_OPT="-D"; fi

mkdir -p "${DEST}/99-99_Test_Range/99_TestCat"
base64 "$DECODE_OPT" > "${DEST}/99-99_Test_Range/99_TestCat/99.02_Example.url" <<'__JD_BASE64__'
W0ludGVybmV0U2hvcnRjdXRdClVSTD1odHRwczovL2V4YW1wbGUuY29tCg==
__JD_BASE64__

mkdir -p "${DEST}/99-99_Test_Range/99_TestCat"
base64 "$DECODE_OPT" > "${DEST}/99-99_Test_Range/99_TestCat/99.03_Website.webloc" <<'__JD_BASE64__'
PD94bWwgdmVyc2lvbj0iMS4wIiBlbmNvZGluZz0iVVRGLTgiPz4KPCFET0NUWVBFIHBsaXN0IFBV
QkxJQyAiLS8vQXBwbGUvL0RURCBQTElTVCAxLjAvL0VOIiAiaHR0cDovL3d3dy5hcHBsZS5jb20v
RFREcy9Qcm9wZXJ0eUxpc3QtMS4wLmR0ZCI+CjxwbGlzdCB2ZXJzaW9uPSIxLjAiPjxkaWN0Pjxr
ZXk+VVJMPC9rZXk+PHN0cmluZz5odHRwczovL2V4YW1wbGUub3JnPC9zdHJpbmc+PC9kaWN0Pjwv
cGxpc3Q+Cg==
__JD_BASE64__

