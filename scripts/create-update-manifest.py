#!/usr/bin/env python3
"""Create the static Tauri updater manifest from release assets."""

from argparse import ArgumentParser
from datetime import datetime, timezone
from pathlib import Path
import json


parser = ArgumentParser()
parser.add_argument("--version", required=True)
parser.add_argument("--assets", type=Path, required=True)
parser.add_argument("--output", type=Path, required=True)
args = parser.parse_args()

assets = {path.name: path for path in args.assets.iterdir() if path.is_file()}

def one(suffix: str) -> Path:
    matches = sorted(path for name, path in assets.items() if name.endswith(suffix))
    if len(matches) != 1:
        raise SystemExit(f"expected exactly one asset ending {suffix!r}, found {len(matches)}")
    return matches[0]


windows = assets.get("LocalShare-Setup.exe")
if windows is None:
    windows = one("-setup.exe")
windows_sig = assets.get(windows.name + ".sig")
macos = one(".app.tar.gz")
macos_sig = assets.get(macos.name + ".sig")
if windows_sig is None or macos_sig is None:
    raise SystemExit("missing updater signature beside a Windows or macOS updater bundle")

base = f"https://github.com/harsh2715/LocalShare/releases/download/v{args.version}/"
manifest = {
    "version": args.version,
    "notes": "LocalShare signed release",
    "pub_date": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
    "platforms": {
        "windows-x86_64": {
            "url": base + windows.name,
            "signature": windows_sig.read_text().strip(),
        },
        "darwin-aarch64": {
            "url": base + macos.name,
            "signature": macos_sig.read_text().strip(),
        },
    },
}
args.output.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
print(f"wrote {args.output}")
