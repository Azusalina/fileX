#!/usr/bin/env python3
"""Measure the release TUI's own Linux RSS/HWM in real PTYs, with bounded fixtures."""
import hashlib
import fcntl
import json
import os
from pathlib import Path
import pty
import select
import struct
import subprocess
import tempfile
import termios
import time

ROOT = Path(__file__).resolve().parents[1]

def measure(root, actions):
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 32, 140, 0, 0))
    process = subprocess.Popen([str(ROOT / "filex.sh"), str(root), "--config", str(ROOT / "fileX.example.conf")], stdin=slave, stdout=slave, stderr=slave, env={**{k: v for k, v in os.environ.items() if not k.startswith("KONSOLE_DBUS_") and k not in ("TMUX", "TMUX_PANE", "NO_COLOR")}, "TERM": "xterm-256color"})
    os.close(slave)
    observed_rss = peak = 0
    output = bytearray()
    def collect(duration):
        nonlocal observed_rss, peak
        deadline = time.monotonic() + duration
        while time.monotonic() < deadline:
            try:
                if os.readlink(f"/proc/{process.pid}/exe") == str(ROOT / "target/release/filex"):
                    values = dict(line.split(":", 1) for line in Path(f"/proc/{process.pid}/status").read_text().splitlines() if ":" in line)
                    observed_rss = max(observed_rss, int(values.get("VmRSS", "0 kB").split()[0]))
                    peak = max(peak, int(values.get("VmHWM", "0 kB").split()[0]))
            except (FileNotFoundError, ProcessLookupError):
                pass
            readable, _, _ = select.select([master], [], [], .01)
            if readable:
                try:
                    data = os.read(master, 65536)
                except OSError:
                    break
                output.extend(data)
                # Keep the capture bounded while retaining error and final-screen evidence.
                if len(output) > 1048576:
                    del output[:-524288]
            if process.poll() is not None:
                break
    try:
        collect(.5)
        executable = os.readlink(f"/proc/{process.pid}/exe")
        assert executable == str(ROOT / "target/release/filex"), executable
        for keys in actions:
            os.write(master, keys.encode())
            collect(.5)
        os.write(master, b"Q")
        collect(.15)
        assert process.wait(timeout=5) == 0, output.decode(errors="replace")[-4000:]
        assert b"panicked" not in output
        return {"max_observed_rss_kib": observed_rss, "peak_rss_kib": peak, "peak_rss_mb": round(peak * 1024 / 1_000_000, 3), "peak_rss_mib": round(peak / 1024, 3), "under_15_mb": peak * 1024 < 15_000_000, "actions": actions}
    finally:
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=5)
        os.close(master)

with tempfile.TemporaryDirectory(prefix="filex-memory-") as directory:
    temp = Path(directory)
    flat = temp / "flat-20000"
    flat.mkdir()
    for i in range(20000):
        (flat / f"文件_{i:05d}_with_a_long_name.txt").touch()
    branches = temp / "branches-7200"
    branches.mkdir()
    for i in range(60):
        folder = branches / f"branch{i:03d}"
        folder.mkdir()
        for j in range(120):
            (folder / f"needle_B{i:03d}_I{j:03d}.txt").touch()
    soft = temp / "symlinks-7200"
    soft.mkdir()
    (soft / "target.txt").write_text("symlink target\n")
    for i in range(60):
        folder = soft / f"branch{i:03d}"
        folder.mkdir()
        for j in range(120):
            (folder / f"soft_B{i:03d}_I{j:03d}.js").symlink_to("../target.txt")
    linked = temp / "hard-links-2400"
    linked.mkdir()
    anchor = linked / "anchor.js"
    anchor.write_text("const test = 1;\nconsole.log(test);\n")
    for i in range(2400):
        os.link(anchor, linked / f"hard{i:04d}.mjs")
    report = {"version": "1.5.0", "binary_sha256": hashlib.sha256((ROOT / "target/release/filex").read_bytes()).hexdigest(), "date": time.strftime("%Y-%m-%d"), "metric": "Linux /proc/<pid>/status VmHWM; release application process only, excludes terminal, measurement driver, compiler and launched external applications", "settings": {"max_entries": 1000, "max_loaded_nodes_per_pane": 6000, "max_expand_nodes": 5000}, "scenarios": {
        "dual_branches_7200_files": measure(branches, ["v", "E", "\t", "E", "q", "\x06needle_B059_I119\r", "k", "j", "\t", "\x06needle_B059_I119\r", "q", "\t", "v", "\x12"]),
        "symlinks_7200": measure(soft, ["E", "q", "\x06soft_B059_I119\r", "k", "j", "I", "\x1b", "\x12"]),
        "hard_links_2400": measure(linked, ["\x1b[C", "q", "I", "\x1b", "\x06hard2399\r", "\x12"]),
        "project_root": measure(ROOT, ["q", "]" * 12, "[" * 12, "E", "C", "\x12"]),
        "flat_20000_files": measure(flat, ["q", "\x06文件_19999\r", "p", "\x1b", "\x1b[D", "\x1b[D", "\x1b[D", "\x12"]),
        "branches_7200_files": measure(branches, ["E", "q", "\x06needle_B059_I119\r", "k", "j", "\x1b[D", "\x1b[D", "\x1b[D", "\x12"]),
    }}
    destination = ROOT / "memory-validation.json"
    destination.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps(report, ensure_ascii=False, indent=2))
    assert all(value["under_15_mb"] for value in report["scenarios"].values()), "RSS budget exceeded"
