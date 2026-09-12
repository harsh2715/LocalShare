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

# Android filters Wi-Fi multicast traffic unless an app explicitly holds a
# MulticastLock. LocalShare uses mDNS for LAN discovery, so install the lock
# in the generated activity before the Rust service starts.
activity_files = list(
    (manifest.parent / "java").rglob("MainActivity.kt")
)
if len(activity_files) != 1:
    raise SystemExit(
        f"Expected exactly one generated MainActivity.kt, found {len(activity_files)}"
    )

activity = activity_files[0]
activity_text = activity.read_text(encoding="utf-8")
if "LocalShareMulticastLock" not in activity_text:
    activity_text = activity_text.replace(
        "import android.os.Bundle\n",
        "import android.content.Context\n"
        "import android.net.wifi.WifiManager\n"
        "import android.os.Bundle\n",
        1,
    )
    activity_text = activity_text.replace(
        "class MainActivity : TauriActivity() {\n",
        "class MainActivity : TauriActivity() {\n"
        "  private var localShareMulticastLock: WifiManager.MulticastLock? = null\n\n",
        1,
    )
    activity_text = activity_text.replace(
        "    enableEdgeToEdge()\n",
        "    val wifiManager = applicationContext.getSystemService(Context.WIFI_SERVICE) as? WifiManager\n"
        "    localShareMulticastLock = wifiManager?.createMulticastLock(\"LocalShareMulticastLock\")?.apply {\n"
        "      setReferenceCounted(false)\n"
        "      acquire()\n"
        "    }\n"
        "    enableEdgeToEdge()\n",
        1,
    )
    activity_text = activity_text.replace(
        "  }\n}",
        "  }\n\n"
        "  override fun onDestroy() {\n"
        "    localShareMulticastLock?.let { lock ->\n"
        "      if (lock.isHeld) lock.release()\n"
        "    }\n"
        "    localShareMulticastLock = null\n"
        "    super.onDestroy()\n"
        "  }\n}",
        1,
    )
    activity.write_text(activity_text, encoding="utf-8")
