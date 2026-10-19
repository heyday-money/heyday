"""Set an MSI-compatible installer version without changing the app version."""
import json
import re
from pathlib import Path


def msi_version(version: str) -> str:
    # MSI ProductVersion compares only major.minor.patch. Keep prerelease and
    # build metadata in the app version, not in the Windows Installer version.
    match = re.fullmatch(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?", version)
    if not match:
        raise ValueError(f"Invalid application version: {version}")
    parts = tuple(int(part) for part in match.groups())
    if any(part > limit for part, limit in zip(parts, (255, 255, 65535))):
        raise ValueError("MSI version must fit major <= 255, minor <= 255, patch <= 65535")
    return ".".join(str(part) for part in parts)


def configure(root: Path) -> str:
    app = json.loads((root / "src-tauri/tauri.conf.json").read_text())
    version = msi_version(app["version"])
    path = root / "src-tauri/tauri.windows.conf.json"
    config = json.loads(path.read_text())
    config.setdefault("bundle", {}).setdefault("windows", {}).setdefault("wix", {})["version"] = version
    path.write_text(json.dumps(config, indent=2) + "\n")
    return version


if __name__ == "__main__":
    version = configure(Path(__file__).resolve().parents[1])
    print(f"MSI installer version: {version}; application version unchanged.")
