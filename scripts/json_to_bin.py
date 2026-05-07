#!/usr/bin/env python3
"""Convert gn_static_dict.json to gn_static_dict.bin format."""
import json, struct, sys
from pathlib import Path

inp = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("/tmp/gn_static_dict.json")
out = Path(sys.argv[2]) if len(sys.argv) > 2 else Path("/home/boot/glasik-core/scripts/gn_static_dict.bin")

d = json.loads(inp.read_text())
entries = d["entries"]

buf = bytearray()
buf += b"GNSD"
buf += struct.pack("<I", 1)
buf += struct.pack("<I", len(entries))
for e in entries:
    b = bytes(e["bytes"])
    buf += struct.pack("B", len(b))
    buf += b
    buf += struct.pack("<Q", int(e["freq"]))
    buf += struct.pack("<Q", int(e["saving"]))

out.write_bytes(buf)
print(f"Wrote {len(entries)} entries ({len(buf)} bytes) to {out}")
