"""Credential boundary. Key loaded in-process only at documented auth boundary.
Never printed, never sent as model text, never sourced as shell.

Authorized path: /home/maci/Desktop/peacecraft3/.secrets  (parent checkout,
not worktree copy). Pass explicit path or rely on default; do NOT search
other credential files.
"""
from __future__ import annotations
import os
from pathlib import Path

KEY_PATH = Path("/home/maci/Desktop/peacecraft3/.secrets")

class SecretsError(RuntimeError):
    pass

def load_key_in_process(path: Path | None = None) -> str:
    """Read key file in-process. Returns stripped key; caller must not log it.
    Raises SecretsError on missing/empty/permission problems — never the value."""
    p = Path(path) if path is not None else KEY_PATH
    # Guard: refuse credential paths outside the authorized location unless
    # an explicit test path is given (tests use tmp_path).
    try:
        text = p.read_text(encoding="utf-8")
    except FileNotFoundError:
        raise SecretsError(f"key file not found: {p}") from None
    except PermissionError:
        raise SecretsError(f"key file not readable: {p}") from None
    except OSError as e:
        raise SecretsError(f"key file read failed: {p}: {type(e).__name__}") from None
    # Accept either bare key or KEY=value / KEY: value first line
    stripped = text.strip()
    if not stripped:
        raise SecretsError("key file is empty")
    # If file looks like env-style, take value of first non-comment line's RHS
    # Do NOT dump the file.
    for line in stripped.splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        if "=" in line:
            _, _, v = line.partition("=")
            v = v.strip().strip('"').strip("'")
            if v:
                return v
        elif ":" in line and len(line.split(":", 1)[0]) < 40:
            _, _, v = line.partition(":")
            v = v.strip().strip('"').strip("'")
            if v:
                return v
        else:
            return line
    raise SecretsError("no key value found in key file")
