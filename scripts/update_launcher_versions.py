#!/usr/bin/env python3
"""Refresh the bundled official unobfuscated catalogue; verify each metadata SHA-1."""
import concurrent.futures
import datetime
import hashlib
import json
from pathlib import Path
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
URL = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json"


def main():
    manifest = json.load(urllib.request.urlopen(URL, timeout=30))
    entries = [v for v in manifest["versions"] if v["id"][:2].isdigit()
               and 26 <= int(v["id"][:2]) <= 99 and v["id"][2:3] in (".", "w") and v["type"] in ("release", "snapshot")]
    cache = ROOT / "build/versions-qa"
    cache.mkdir(parents=True, exist_ok=True)

    def verify(v):
        path = cache / (v["id"] + ".json")
        body = path.read_bytes() if path.is_file() else b""
        if hashlib.sha1(body).hexdigest() != v["sha1"]:
            body = urllib.request.urlopen(v["url"], timeout=30).read()
        if hashlib.sha1(body).hexdigest() != v["sha1"]:
            raise ValueError("Metadata checksum mismatch: " + v["id"])
        meta = json.loads(body)
        if meta["id"] != v["id"] or any("mappings" in key for key in meta["downloads"]):
            raise ValueError("Not an unobfuscated metadata set: " + v["id"])
        if not meta["arguments"]["game"] or "client" not in meta["downloads"]:
            raise ValueError("Unsupported launch metadata: " + v["id"])
        path.write_bytes(body)
        return {k: v[k] for k in ("id", "type", "url", "sha1", "releaseTime")}

    with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
        versions = list(pool.map(verify, entries))
    result = {"source": URL, "checked_at": datetime.datetime.now(datetime.timezone.utc).isoformat(), "versions": versions}
    target = ROOT / "launcher/versions.json"
    target.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(f"Verified {len(versions)} versions; {sum(v['type'] == 'release' for v in versions)} releases: {target}")


if __name__ == "__main__":
    main()
