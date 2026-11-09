"""Compile a packaged core from outside the desktop checkout; never publish it."""

import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile


ROOT = Path(__file__).resolve().parents[1]


def run(*args, cwd, env, capture=False):
    return subprocess.run(
        args, cwd=cwd, env=env, check=True, text=True,
        stdout=subprocess.PIPE if capture else None,
    ).stdout


def main():
    with tempfile.TemporaryDirectory(prefix="heyday-core-consumer-") as directory:
        temporary = Path(directory)
        env = {**os.environ, "CARGO_TARGET_DIR": str(temporary / "target")}
        run("cargo", "package", "--locked", "--offline", "--allow-dirty",
            "--no-verify", "--manifest-path",
            str(ROOT / "crates/heyday-core/Cargo.toml"), cwd=temporary, env=env)
        archives = list((temporary / "target/package").glob("heyday-core-*.crate"))
        if len(archives) != 1:
            raise RuntimeError("Expected exactly one packaged core")
        with tarfile.open(archives[0]) as archive:
            destination = temporary / "package"
            for member in archive.getmembers():
                target = destination / member.name
                if not target.resolve().is_relative_to(destination.resolve()):
                    raise RuntimeError(f"Invalid package path: {member.name}")
                if not member.isfile():
                    raise RuntimeError(f"Unexpected non-file in package: {member.name}")
                target.parent.mkdir(parents=True, exist_ok=True)
                with archive.extractfile(member) as source, target.open("wb") as output:
                    shutil.copyfileobj(source, output)
        packaged = next((temporary / "package").iterdir())
        consumer = temporary / "consumer"
        (consumer / "src").mkdir(parents=True)
        (consumer / "Cargo.toml").write_text(
            '[package]\nname = "core-consumer"\nversion = "0.0.0"\n'
            'edition = "2021"\n\n[dependencies]\n'
            f'heyday-core = {{ path = {json.dumps(packaged.as_posix())} }}\n',
            encoding="utf-8",
        )
        # Exercise a real public domain API from the packaged library.
        (consumer / "src/main.rs").write_text(
            'fn main() { assert!(heyday_core::domain::dates::valid_date("2024-02-29")); }\n', encoding="utf-8"
        )
        # Resolve the standalone consumer using the packaged lockfile's versions.
        shutil.copyfile(packaged / "Cargo.lock", consumer / "Cargo.lock")
        host = next(line.split(": ", 1)[1] for line in run(
            "rustc", "-vV", cwd=consumer, env=env, capture=True,
        ).splitlines() if line.startswith("host: "))
        metadata = json.loads(run(
            "cargo", "metadata", "--offline", "--format-version", "1", "--filter-platform", host,
            cwd=consumer, env=env, capture=True,
        ))
        forbidden = ("tauri", "objc", "cocoa", "keyring", "secret-service",
                     "security-framework", "web-sys", "js-sys", "wasm-bindgen")
        for package in metadata["packages"]:
            name = package["name"]
            if name == "heyday-money" or name.startswith(forbidden):
                raise RuntimeError(f"Platform dependency in core: {name}")
        run("cargo", "check", "--locked", "--offline", cwd=consumer, env=env)
        run("cargo", "test", "--locked", "--offline",
            cwd=packaged, env=env)
        print("Packaged core and independent consumer passed without platform dependencies.")


if __name__ == "__main__":
    main()
