from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


class CommandTests(unittest.TestCase):
    def test_init_config_refuses_overwrite_and_explicit_missing_config(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "settings.conf"
            command = [str(ROOT / "filex.sh"), "--config", str(path)]
            missing = subprocess.run(command + ["--print-tree"], cwd=ROOT, capture_output=True)
            self.assertEqual(missing.returncode, 1)
            created = subprocess.run(command + ["--init-config"], cwd=ROOT, capture_output=True)
            self.assertEqual(created.returncode, 0, created.stderr)
            original = path.read_bytes()
            repeated = subprocess.run(command + ["--init-config"], cwd=ROOT, capture_output=True)
            self.assertEqual(repeated.returncode, 1)
            self.assertEqual(path.read_bytes(), original)

    def test_install_launcher_from_other_cwd_and_uninstall(self):
        with tempfile.TemporaryDirectory(prefix="filex installer ") as folder:
            prefix = Path(folder) / "local"
            command = [str(ROOT / "scripts/install.sh"), "--prefix", str(prefix)]
            result = subprocess.run(command, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            result = subprocess.run(command, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            launcher = prefix / "bin/filex"
            result = subprocess.run([str(launcher), "--print-tree"], cwd=folder, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn(Path(folder).name, result.stdout)
            self.assertTrue((prefix / "share/applications/filex.desktop").exists())
            result = subprocess.run([str(ROOT / "scripts/uninstall.sh"), "--prefix", str(prefix)], capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertFalse(launcher.exists())
            self.assertFalse((prefix / "share/applications/filex.desktop").exists())
            launcher.write_text("preserve unrelated entry")
            result = subprocess.run(command, capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(launcher.read_text(), "preserve unrelated entry")


if __name__ == "__main__":
    unittest.main()
