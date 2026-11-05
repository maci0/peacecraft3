"""Credential boundary. Key loaded in-process only at documented auth boundary.
Never printed, never sent as model text, never sourced as shell.

Configured selection (explicit, no filesystem search, no shell sourcing):

1. Explicit ``path`` argument wins when given.
2. Otherwise the documented ``TYPESAFE_API_KEY`` environment variable,
   read in-process via ``os.environ`` (never logged, never sourced).
3. Otherwise ``KEY_PATH`` (``/home/maci/Desktop/peacecraft3/.secrets``,
   parent checkout, not worktree copy) as legacy default.

If both an explicit ``path`` and a non-empty env var are supplied,
``path`` takes precedence; the env value is ignored when ``path`` is
given (explicit > env > default file). When neither path nor env is
given and the default file is missing, raise SecretsError naming the
env var to set or the path to create.

Do NOT search other credential files, do NOT try alternate access after
a read refusal.
"""
from __future__ import annotations
import os
from pathlib import Path
from typing import Mapping

KEY_PATH = Path("/home/maci/Desktop/peacecraft3/.secrets")
ENV_NAME = "TYPESAFE_API_KEY"

class SecretsError(RuntimeError):
    pass

def _parse_key_file(text: str, path: Path) -> str:
    stripped = text.strip()
    if not stripped:
        raise SecretsError("key file is empty")
    for line in stripped.splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        if "=" in line:
            _, _, v = line.partition("=")
            v = v.strip().strip('"').strip("'")
            if v:
                return v
            raise SecretsError("key file has empty value")
        elif ":" in line and len(line.split(":", 1)[0]) < 40:
            _, _, v = line.partition(":")
            v = v.strip().strip('"').strip("'")
            if v:
                return v
            raise SecretsError("key file has empty value")
        else:
            return line
    raise SecretsError("no key value found in key file")

def load_key_in_process(path: Path | None = None, *, environ: Mapping[str, str] | None = None) -> str:
    """Read key in-process. Returns stripped key; caller must not log it.

    Precedence: explicit path > TYPESAFE_API_KEY env > KEY_PATH default.
    ``environ`` is injectable for tests (defaults to os.environ); production
    callers omit it. Raises SecretsError on missing/empty/malformed —
    never the value, never falls back to another mechanism after refusal.
    """
    if path is not None:
        p = Path(path)
        try:
            text = p.read_text(encoding="utf-8")
        except FileNotFoundError:
            raise SecretsError(f"key file not found: {p}") from None
        except PermissionError:
            raise SecretsError(f"key file not readable: {p}") from None
        except OSError as e:
            raise SecretsError(f"key file read failed: {p}: {type(e).__name__}") from None
        return _parse_key_file(text, p)
    env = os.environ if environ is None else environ
    val = env.get(ENV_NAME, "")
    if isinstance(val, str) and val.strip():
        return val.strip()
    p = KEY_PATH
    try:
        text = p.read_text(encoding="utf-8")
    except FileNotFoundError:
        raise SecretsError(
            f"key file not found: {p}; set {ENV_NAME} or pass an explicit path"
        ) from None
    except PermissionError:
        raise SecretsError(f"key file not readable: {p}") from None
    except OSError as e:
        raise SecretsError(f"key file read failed: {p}: {type(e).__name__}") from None
    return _parse_key_file(text, p)
