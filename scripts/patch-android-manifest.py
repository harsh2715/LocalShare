#!/usr/bin/env python3
"""Apply LocalShare's minimal Android LAN/notification permissions after Tauri init."""

from pathlib import Path


manifest = Path("src-tauri/gen/android/app/src/main/AndroidManifest.xml")
if not manifest.is_file():
    raise SystemExit(f"Android manifest not found: {manifest}")

text = manifest.read_text(encoding="utf-8")
anchor = '    <uses-permission android:name="android.permission.INTERNET" />\n'
permissions = [
    "ACCESS_NETWORK_STATE",
    "ACCESS_WIFI_STATE",
    "CHANGE_WIFI_MULTICAST_STATE",
    "POST_NOTIFICATIONS",
]
missing = [
    f'    <uses-permission android:name="android.permission.{permission}" />\n'
    for permission in permissions
    if f'android.permission.{permission}' not in text
]
if missing:
    if anchor not in text:
        raise SystemExit("Android manifest has no INTERNET permission anchor")
    text = text.replace(anchor, anchor + "".join(missing), 1)
    manifest.write_text(text, encoding="utf-8")
