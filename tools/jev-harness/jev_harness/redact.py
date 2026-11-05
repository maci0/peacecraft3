"""Redact credentials/headers from logs and errors."""
from __future__ import annotations
import re

# Patterns that must never appear in trace/log output
_PATTERNS = [
    (re.compile(r"(?i)(authorization\s*[:=]\s*)(?:bearer\s+)?\S+"), r"\1[REDACTED]"),
    (re.compile(r"(?i)(bearer\s+)\S+"), r"\1[REDACTED]"),
    (re.compile(r"(?i)(x-api-key\s*[:=]\s*)\S+"), r"\1[REDACTED]"),
    (re.compile(r"(?i)(api[_-]?key\s*[:=]\s*)\S+"), r"\1[REDACTED]"),
    (re.compile(r"(sk-[A-Za-z0-9_-]{8,})"), "[REDACTED_KEY]"),
    (re.compile(r"(JEV_[A-Z_]*KEY\s*[:=]\s*)\S+"), r"\1[REDACTED]"),
]

def redact_text(s: str) -> str:
    if not isinstance(s, str):
        s = str(s)
    for pat, repl in _PATTERNS:
        s = pat.sub(repl, s)
    return s

def redact(obj):
    """Recursively redact strings in dict/list/tuple."""
    if isinstance(obj, str):
        return redact_text(obj)
    if isinstance(obj, dict):
        return {redact_text(str(k)): redact(v) for k, v in obj.items()}
    if isinstance(obj, (list, tuple)):
        return [redact(v) for v in obj]
    return obj
