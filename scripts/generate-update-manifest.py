"""Generate a cross-platform update manifest (dist/latest.json) from packaged artifacts."""
import hashlib
import json
import os
import sys
from datetime import datetime, timezone
from pathlib import Path

try:
    import tomllib
except ImportError:  # pragma: no cover
    tomllib = None


def read_workspace_version(root: Path) -> str:
    cargo_toml = (root / "Cargo.toml").read_text(encoding="utf-8")
    if tomllib is not None:
        return tomllib.loads(cargo_toml)["workspace"]["package"]["version"]
    for line in cargo_toml.splitlines():
        if line.strip().startswith("version"):
            return line.split("=", 1)[1].strip().strip('"')
    return "0.1.5"


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            h.update(chunk)
    return h.hexdigest().upper()


def classify_artifact(name: str) -> str | None:
    lower = name.lower()
    if lower.endswith("-setup.exe"):
        return "windows-x86_64-installer"
    if lower.endswith("windows-x64.zip"):
        return "windows-x86_64-portable"
    if "macos" in lower and ("arm64" in lower or "aarch64" in lower) and lower.endswith(".dmg"):
        return "darwin-aarch64"
    if "macos" in lower and "x86_64" in lower and lower.endswith(".dmg"):
        return "darwin-x86_64"
    if "macos" in lower and lower.endswith(".dmg"):
        return "darwin-universal"
    return None


def main() -> int:
    root = Path(__file__).resolve().parent.parent
    dist = Path(sys.argv[1]) if len(sys.argv) > 1 else root / "dist"
    version = os.environ.get("GLOW_RELEASE_VERSION") or read_workspace_version(root)
    base_url = os.environ.get(
        "GLOW_RELEASE_BASE_URL",
        f"https://github.com/wanglongan587/reading_light/releases/download/v{version}",
    ).rstrip("/")

    platforms: dict[str, dict[str, object]] = {}
    if dist.is_dir():
        for entry in sorted(dist.iterdir()):
            if not entry.is_file():
                continue
            key = classify_artifact(entry.name)
            if key is None:
                continue
            platforms[key] = {
                "filename": entry.name,
                "url": f"{base_url}/{entry.name}",
                "sha256": sha256_file(entry),
                "size": entry.stat().st_size,
            }

    manifest = {
        "schema_version": 1,
        "version": version,
        "pub_date": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "notes_url": f"https://github.com/wanglongan587/reading_light/releases/tag/v{version}",
        "platforms": platforms,
    }
    dist.mkdir(parents=True, exist_ok=True)
    out = dist / "latest.json"
    out.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"Wrote update manifest: {out} ({len(platforms)} platforms)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
