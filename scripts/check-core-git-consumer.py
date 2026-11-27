"""Build a temporary consumer of a full Git revision, without sibling paths."""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
URL = "https://github.com/heyday-money/heyday.git"

def run(*args, cwd, capture=False):
    return subprocess.run(args, cwd=cwd, check=True, text=True,
                          stdout=subprocess.PIPE if capture else None).stdout

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rev", required=True)
    parser.add_argument("--offline", action="store_true", help="Use an already cached Git revision and registry")
    args = parser.parse_args()
    if not re.fullmatch(r"[0-9a-f]{40}", args.rev):
        parser.error("--rev must be a full 40-character commit SHA")
    offline = ["--offline"] if args.offline else []
    with tempfile.TemporaryDirectory(prefix="heyday-git-consumer-") as directory:
        consumer = Path(directory)
        (consumer / "src").mkdir()
        (consumer / "Cargo.toml").write_text(
            '[package]\nname = "core-git-consumer"\nversion = "0.0.0"\nedition = "2021"\n'
            '[dependencies]\n'
            f'heyday-core = {{ git = "{URL}", rev = "{args.rev}" }}\n', encoding="utf-8")
        (consumer / "src/main.rs").write_text('''fn main() {
    use heyday_core::{database, domain, crypto::encryption as crypto};
    assert!(domain::dates::valid_date("2024-02-29"));
    assert!(database::MIGRATOR.iter().next().is_some());
    assert!(!database::INSTITUTIONS.is_empty());
    assert!(!database::SUBSCRIPTION_PROVIDERS.is_empty());
    assert!(!database::SYNC_METADATA_SEEDS.is_empty());
    let key = crypto::Secret::new([7; 32]);
    let recovery = crypto::Secret::new([9; 32]);
    let envelope = crypto::wrap("test-account", "test-dataset", &key, &recovery).unwrap();
    assert_eq!(*crypto::unwrap("test-account", "test-dataset", &envelope, &recovery).unwrap(), *key);
}
''', encoding="utf-8")
        # Seed dependency versions from the independently maintained core lockfile.
        # Cargo adds the Git source and consumer; it must not need a desktop workspace.
        shutil.copyfile(ROOT / "crates/heyday-core/Cargo.lock", consumer / "Cargo.lock")
        host = next(line.split(": ", 1)[1] for line in run(
            "rustc", "-vV", cwd=consumer, capture=True).splitlines() if line.startswith("host: "))
        metadata = json.loads(run("cargo", "metadata", "--format-version", "1",
                                  "--filter-platform", host, *offline, cwd=consumer, capture=True))
        core = next(p for p in metadata["packages"] if p["name"] == "heyday-core")
        if core["source"] != f"git+{URL}?rev={args.rev}#{args.rev}":
            raise RuntimeError("Core was not resolved from the requested Git revision")
        forbidden = ("tauri", "objc", "cocoa", "keyring", "secret-service", "security-framework", "web-sys", "js-sys", "wasm-bindgen")
        for p in metadata["packages"]:
            if p["name"] == "heyday-money" or p["name"].startswith(forbidden):
                raise RuntimeError("Desktop/platform dependency in Git consumer: " + p["name"])
        target = os.environ.get("CARGO_TARGET_DIR", str(consumer / "target"))
        run("cargo", "run", "--locked", "--target-dir", target, *offline, cwd=consumer)
        # Exercise embedded fixtures from the Git checkout itself, not the worktree.
        run("cargo", "test", "--locked", "--manifest-path", core["manifest_path"],
            "--target-dir", target, *offline, cwd=consumer)
        print("Independent Git consumer passed at", args.rev)

if __name__ == "__main__":
    main()
