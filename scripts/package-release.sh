#!/usr/bin/env bash
# Package a native binary, then smoke-test the extracted ZIP.
set -euo pipefail

if [[ $# != 4 ]]; then
  echo "usage: package-release.sh BINARY TARGET VERSION OUTPUT_DIR" >&2
  exit 1
fi
binary=$1
target=$2
version=$3
for value in "$target" "$version"; do
  [[ "$value" =~ ^[A-Za-z0-9][A-Za-z0-9._+-]*$ ]] || {
    echo "target and version must be safe filename components" >&2
    exit 1
  }
done
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
mkdir -p "$4"
output="$(cd "$4" && pwd)"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
name="git-fs-$version-$target.zip"

mkdir "$scratch/package" "$scratch/extracted"
install -m 755 "$binary" "$scratch/package/git-fs"
install -m 644 "$root/LICENSE" "$root/install.txt" "$scratch/package/"
(
  cd "$scratch/package"
  zip -q "$scratch/$name" git-fs LICENSE install.txt
)
unzip -q "$scratch/$name" -d "$scratch/extracted"
"$scratch/extracted/git-fs" tools > "$scratch/tools"
grep -qx read "$scratch/tools"
mv "$scratch/$name" "$output/$name"
(
  cd "$output"
  shasum -a 256 "$name" > "$name.sha256"
)
echo "$output/$name"
