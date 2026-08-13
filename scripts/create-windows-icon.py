"""Build the checked-in Windows ICO from the existing LocalShare PNG asset.

Windows accepts PNG-compressed images inside an ICO container. Keeping this
small generator in the repository makes the derived binary reproducible on
macOS, Windows, and CI without adding an image-processing dependency.
"""

from pathlib import Path
import struct


ROOT = Path(__file__).resolve().parents[1]
source = ROOT / "src-tauri" / "icons" / "icon.png"
target = ROOT / "src-tauri" / "icons" / "icon.ico"
png = source.read_bytes()

# ICONDIR: reserved, type (1 = icon), image count.
header = struct.pack("<HHH", 0, 1, 1)
# ICONDIRENTRY: 128x128 RGBA PNG, one color plane, 32 bits per pixel.
entry = struct.pack("<BBBBHHII", 128, 128, 0, 0, 1, 32, len(png), 6 + 16)
target.write_bytes(header + entry + png)
print(f"wrote {target} ({target.stat().st_size} bytes)")
