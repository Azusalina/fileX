from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


class CommandTests(unittest.TestCase):
    def test_lowercase_default_config_and_legacy_fallback(self):
        import os
        with tempfile.TemporaryDirectory(prefix="filex-config-") as directory:
            base = Path(directory)
            root = base / "files"
            root.mkdir()
            (root / ".hidden-probe").touch()
            settings = base / "settings/fileX"
            settings.mkdir(parents=True)
            canonical, legacy = settings / "filex.conf", settings / "fileX.conf"
            canonical.write_text('[browser]\nshow_hidden=false\n')
            legacy.write_text('[browser]\nshow_hidden=true\n')
            env = {**os.environ, "XDG_CONFIG_HOME": str(settings.parent)}
            for command in ([str(ROOT / "filex.sh")], [sys.executable, "-m", "filex"]):
                result = subprocess.run([*command, str(root), "--print-tree"], cwd=ROOT, env=env, capture_output=True, text=True)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertNotIn(".hidden-probe", result.stdout)
            canonical.unlink()
            for command in ([str(ROOT / "filex.sh")], [sys.executable, "-m", "filex"]):
                result = subprocess.run([*command, str(root), "--print-tree"], cwd=ROOT, env=env, capture_output=True, text=True)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertIn(".hidden-probe", result.stdout)
            legacy.unlink()
            result = subprocess.run([str(ROOT / "filex.sh"), "--init-config"], cwd=ROOT, env=env, capture_output=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(canonical.read_bytes(), (ROOT / "fileX.example.conf").read_bytes())
            self.assertFalse(legacy.exists())

    def test_init_config_refuses_overwrite_and_explicit_missing_config(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "settings.conf"
            command = [str(ROOT / "filex.sh"), "--config", str(path)]
            missing = subprocess.run(command + ["--print-tree"], cwd=ROOT, capture_output=True)
            self.assertEqual(missing.returncode, 1)
            created = subprocess.run(command + ["--init-config"], cwd=ROOT, capture_output=True)
            self.assertEqual(created.returncode, 0, created.stderr)
            self.assertEqual(path.read_bytes(), (ROOT / "fileX.example.conf").read_bytes())
            original = path.read_bytes()
            repeated = subprocess.run(command + ["--init-config"], cwd=ROOT, capture_output=True)
            self.assertEqual(repeated.returncode, 1)
            self.assertEqual(path.read_bytes(), original)
            removed = subprocess.run([str(ROOT / "filex.sh"), "--gui"], capture_output=True)
            self.assertEqual(removed.returncode, 1)
            self.assertIn("未知选项".encode(), removed.stderr)

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
            self.assertFalse((prefix / "share/applications/filex.desktop").exists())
            obsolete = prefix / "share/applications/filex.desktop"
            obsolete.parent.mkdir(parents=True)
            obsolete.write_text("# Managed by fileX installer\nExec=" + str(ROOT / "filex.sh") + " --gui\n")
            subprocess.run(command, check=True, capture_output=True)
            self.assertFalse(obsolete.exists())
            result = subprocess.run([str(ROOT / "scripts/uninstall.sh"), "--prefix", str(prefix)], capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertFalse(launcher.exists())
            self.assertFalse((prefix / "share/applications/filex.desktop").exists())
            launcher.write_text("preserve unrelated entry")
            result = subprocess.run(command, capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(launcher.read_text(), "preserve unrelated entry")

    def test_shell_function_exits_into_a_quoted_unicode_directory(self):
        import fcntl, os, pty, select, shlex, struct, termios, time
        with tempfile.TemporaryDirectory(prefix="filex-shell-") as directory:
            root = Path(directory)
            target = root / "aa ' ; $(literal)\n目录"
            target.mkdir()
            temporary = root / ".temporary"
            temporary.mkdir()
            rc = root / ".bashrc"
            rc.write_text("PS1='FILEX-TEST$ '\n")
            subprocess.run([sys.executable, str(ROOT / "scripts/install_alias.py"), "--rc", str(rc)], check=True, capture_output=True)
            first = rc.read_text()
            subprocess.run([sys.executable, str(ROOT / "scripts/install_alias.py"), "--rc", str(rc)], check=True, capture_output=True)
            self.assertEqual(first, rc.read_text())
            config = root / ".settings.conf"
            config.write_text('[input_method]\nauto_english=false\n')
            record = root / ".cwd-result"
            env = {k: v for k, v in os.environ.items() if k not in {"TMUX", "TMUX_PANE"} and not k.startswith("KONSOLE_DBUS_")}
            env.update(TERM="xterm-256color", TMPDIR=str(temporary))
            master, slave = pty.openpty()
            fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 26, 130, 0, 0))
            process = subprocess.Popen(["bash", "--noprofile", "--rcfile", str(rc), "-i"], cwd=root,
                                       stdin=slave, stdout=slave, stderr=slave, env=env)
            os.close(slave)
            output = bytearray()
            def drain(duration):
                end = time.monotonic() + duration
                while time.monotonic() < end:
                    if select.select([master], [], [], .02)[0]:
                        try: output.extend(os.read(master, 65536))
                        except OSError: break
            try:
                drain(.2)
                os.write(master, ("filex --config " + shlex.quote(str(config)) + "\r").encode())
                end = time.monotonic() + 5
                while "↑↓".encode() not in output and time.monotonic() < end: drain(.05)
                self.assertIn("↑↓".encode(), output)
                os.write(master, b"\x1b[Cx")
                drain(.3)
                command = "printf '%s' \"$PWD\" > " + shlex.quote(str(record)) + "\r"
                os.write(master, command.encode())
                end = time.monotonic() + 5
                while not record.exists() and time.monotonic() < end: drain(.05)
                self.assertTrue(record.exists(), output.decode(errors="replace")[-1500:])
                self.assertEqual(record.read_text(), str(target))
                self.assertEqual(list(temporary.iterdir()), [])
                os.write(master, b"exit\r")
                self.assertEqual(process.wait(timeout=3), 0)
            finally:
                if process.poll() is None:
                    process.terminate(); process.wait(timeout=3)
                os.close(master)


if __name__ == "__main__":
    unittest.main()
