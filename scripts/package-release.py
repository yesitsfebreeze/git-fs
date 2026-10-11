#!/usr/bin/env python3
"""Package a native release binary and verify that the extracted ZIP runs."""

import hashlib
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import zipfile


def main():
    if len(sys.argv) != 5:
        raise SystemExit("usage: package-release.py BINARY TARGET VERSION OUTPUT_DIR")
    binary = Path(sys.argv[1]).resolve()
    target, version = sys.argv[2:4]
    for value in (target, version):
        if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._+-]*", value):
            raise SystemExit("target and version must be safe filename components")
    root = Path(__file__).resolve().parents[1]
    output = Path(sys.argv[4]).resolve()
    output.mkdir(parents=True, exist_ok=True)
    archive = output / f"git-fs-{version}-{target}.zip"
    with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as bundle:
        for source, name, mode in (
            (binary, "git-fs", 0o100755),
            (root / "LICENSE", "LICENSE", 0o100644),
            (root / "install.txt", "install.txt", 0o100644),
        ):
            info = zipfile.ZipInfo.from_file(source, name)
            info.create_system = 3
            info.external_attr = mode << 16
            info.compress_type = zipfile.ZIP_DEFLATED
            bundle.writestr(info, source.read_bytes())

    # Exercise the artifact users download, not just the build output.
    with tempfile.TemporaryDirectory(prefix="git-fs-release-") as scratch:
        with zipfile.ZipFile(archive) as bundle:
            bundle.extractall(scratch)
            mode = bundle.getinfo("git-fs").external_attr >> 16
        installed = Path(scratch) / "git-fs"
        os.chmod(installed, mode & 0o777)
        result = subprocess.run(
            [str(installed), "tools"], check=True, capture_output=True, text=True
        )
        if "read" not in result.stdout.splitlines():
            raise SystemExit("extracted binary did not list the read tool")

    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    archive.with_suffix(".zip.sha256").write_text(
        f"{digest}  {archive.name}\n", encoding="utf-8"
    )
    print(archive)


if __name__ == "__main__":
    main()
