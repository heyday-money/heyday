"""Release stamping must not change the independently versioned core."""
from pathlib import Path
import json
import runpy
import shutil
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
set_version = runpy.run_path(str(ROOT / "scripts/set-release-version.py"))["set_version"]

class ReleaseVersionTest(unittest.TestCase):
    def test_app_release_preserves_core_manifests_and_lockfiles(self):
        files = ["package.json", "src-tauri/tauri.conf.json", "src-tauri/Cargo.toml",
                 "src-tauri/Cargo.lock", "crates/heyday-core/Cargo.toml", "crates/heyday-core/Cargo.lock"]
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name in files:
                target = root / name
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(ROOT / name, target)
            self.assertEqual(set_version(root, "v0.0.2-beta.1"), "0.0.2-beta.1")
            for name in files[:2]:
                self.assertEqual(json.loads((root / name).read_text())["version"], "0.0.2-beta.1")
            old_version = json.loads((ROOT / "package.json").read_text())["version"]
            for name in files[2:4]:
                expected = (ROOT / name).read_text().replace(
                    f'name = "heyday-money"\nversion = "{old_version}"',
                    'name = "heyday-money"\nversion = "0.0.2-beta.1"')
                self.assertEqual((root / name).read_text(), expected)
            for name in files[4:]:
                self.assertEqual((root / name).read_bytes(), (ROOT / name).read_bytes())

if __name__ == "__main__":
    unittest.main()
