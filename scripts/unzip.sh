#!/usr/bin/env bash
# Arguments remain data; no archive path is interpolated into shell source.
set -euo pipefail
[[ $# == 1 ]] || { printf 'Usage: unzip.sh ARCHIVE.zip\n' >&2; exit 2; }
command -v unzip >/dev/null || { printf 'Install unzip and add it to PATH.\n' >&2; exit 1; }
filex_python=""
for filex_candidate in python3 python; do
    if command -v "$filex_candidate" >/dev/null; then filex_python="$filex_candidate"; break; fi
done
[[ -n "$filex_python" ]] || { printf 'Python 3 is required to validate ZIP paths before extraction.\n' >&2; exit 1; }
filex_archive="$1"
if command -v cygpath >/dev/null; then filex_archive="$(cygpath -u "$filex_archive")"; fi
filex_archive="$(realpath -- "$filex_archive")"
filex_scripts="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
"$filex_python" "$filex_scripts/check_zip.py" "$filex_archive"
unzip -tqq "$filex_archive"
filex_destination="$(mktemp -d "${filex_archive%.*}.unpacked.XXXXXX")"
if unzip -n "$filex_archive" -d "$filex_destination"; then
    printf '\nExtracted into: %s\n' "$filex_destination"
else
    printf '\nExtraction failed; partial output retained at: %s\n' "$filex_destination" >&2
    exit 1
fi
