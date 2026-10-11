"""Restore the prior Fcitx5 input method after the optional Python TUI exits."""
from contextlib import contextmanager
import subprocess


def remote(*args):
    try:
        result = subprocess.run(["fcitx5-remote", "--check", *args], capture_output=True, text=True)
        return result.stdout.strip() if result.returncode == 0 else None
    except OSError:
        return None


@contextmanager
def english(enabled=True):
    previous = remote("-n") if enabled else None
    switched = previous and (previous == "keyboard-us" or remote("-s", "keyboard-us") is not None)
    try:
        yield
    finally:
        if switched:
            remote("-s", previous)
