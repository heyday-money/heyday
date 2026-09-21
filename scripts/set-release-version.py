"""Apply a validated release tag to the CI checkout; never commit these edits."""
import json
import os
import re
from pathlib import Path


def set_version(root: Path, tag: str) -> str:
    number = r"(?:0|[1-9][0-9]*)"
    identifier = rf"(?:{number}|[0-9A-Za-z-]*[A-Za-z-][0-9A-Za-z-]*)"
    pattern = rf"v({number}\.{number}\.{number}(?:-{identifier}(?:\.{identifier})*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?)"
    match = re.fullmatch(pattern, tag)
    if not match:
        raise ValueError("Release tag must be v followed by a semantic version, e.g. v0.0.0-alpha.0")
    version = match.group(1)
    for filename in ["package.json", "src-tauri/tauri.conf.json"]:
        path = root / filename
        data = json.loads(path.read_text())
        data["version"] = version
        path.write_text(json.dumps(data, indent=2) + "\n")
    for filename, header in [
        ("src-tauri/Cargo.toml", r'\[package\]\nname = "heyday-money"\n'),
        ("src-tauri/Cargo.lock", r'\[\[package\]\]\nname = "heyday-money"\n'),
    ]:
        path = root / filename
        text, count = re.subn(rf'({header}version = ")[^"]+("\n)', lambda m: m[1] + version + m[2], path.read_text())
        if count != 1:
            raise ValueError(f"Could not locate the app version in {filename}")
        path.write_text(text)
    return version


if __name__ == "__main__":
    version = set_version(Path(__file__).resolve().parents[1], os.environ["RELEASE_TAG"])
    if "GITHUB_OUTPUT" in os.environ:
        with open(os.environ["GITHUB_OUTPUT"], "a") as output:
            output.write("prerelease=" + str("-" in version.split("+")[0]).lower() + "\n")
