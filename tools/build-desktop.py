#!/usr/bin/env python3
"""Compile and test on the target OS, then package only the game executable."""

import argparse
import hashlib
import os
from pathlib import Path
import struct
import subprocess
import tarfile
import zipfile


TARGETS = (
    "x86_64-pc-windows-msvc",
    "aarch64-pc-windows-msvc",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
)


def verify_header(data, target):
    """Reject an executable for the wrong OS or CPU before naming its archive."""
    arm = target.startswith("aarch64")
    if "windows" in target:
        if data[:2] != b"MZ" or len(data) < 64:
            raise ValueError("expected a Windows PE executable")
        offset = struct.unpack_from("<I", data, 60)[0]
        if data[offset:offset + 4] != b"PE\0\0" or len(data) < offset + 26:
            raise ValueError("invalid PE header")
        machine = struct.unpack_from("<H", data, offset + 4)[0]
        expected = 0xAA64 if arm else 0x8664
        if machine != expected or struct.unpack_from("<H", data, offset + 24)[0] != 0x20B:
            raise ValueError("expected a 64-bit PE executable for " + target)
    else:
        if data[:4] != b"\xcf\xfa\xed\xfe" or len(data) < 32:
            raise ValueError("expected a 64-bit Mach-O executable")
        cpu, _, filetype = struct.unpack_from("<III", data, 4)
        if cpu != (0x0100000C if arm else 0x01000007) or filetype != 2:
            raise ValueError("expected a Mach-O executable for " + target)


def self_test():
    for target in TARGETS:
        arm = target.startswith("aarch64")
        if "windows" in target:
            data = bytearray(128)
            data[:2] = b"MZ"
            struct.pack_into("<I", data, 60, 64)
            data[64:68] = b"PE\0\0"
            struct.pack_into("<H", data, 68, 0xAA64 if arm else 0x8664)
            struct.pack_into("<H", data, 88, 0x20B)
        else:
            data = bytearray(32)
            data[:4] = b"\xcf\xfa\xed\xfe"
            struct.pack_into("<III", data, 4, 0x0100000C if arm else 0x01000007, 0, 2)
        verify_header(data, target)
        wrong_arch = target.replace("aarch64", "x86_64") if arm else target.replace("x86_64", "aarch64")
        for other in (wrong_arch, TARGETS[2] if "windows" in target else TARGETS[0]):
            try:
                verify_header(data, other)
            except ValueError:
                pass
            else:
                raise AssertionError("accepted an executable for " + other)
        for malformed in (b"", data[:10], b"\0" * 128):
            try:
                verify_header(malformed, target)
            except ValueError:
                pass
            else:
                raise AssertionError("accepted a malformed executable")
    print("All four executable formats checked; wrong architecture and truncated inputs rejected.")


def build(target):
    root = Path(__file__).resolve().parent.parent
    os.chdir(root)
    toolchain = "1.99.0-" + target
    version = subprocess.check_output(["rustup", "run", toolchain, "rustc", "-Vv"], text=True)
    if "host: " + target not in version.splitlines():
        raise RuntimeError("build on the target OS with its native Rust toolchain")
    print(version, flush=True)
    cargo = ["rustup", "run", toolchain, "cargo"]
    args = ["--locked", "-p", "peacecraft3", "--bin", "peacecraft3", "--target", target]
    subprocess.run(cargo + ["build", "--release"] + args, check=True)
    subprocess.run(cargo + ["test", "--release"] + args, check=True)
    binary_name = "peacecraft3.exe" if "windows" in target else "peacecraft3"
    binary = Path(os.environ.get("CARGO_TARGET_DIR", "target")) / target / "release" / binary_name
    verify_header(binary.read_bytes(), target)
    dist = root / "dist"
    dist.mkdir(exist_ok=True)
    basename = "peacecraft3-" + target
    if "windows" in target:
        archive = dist / (basename + ".zip")
        with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as bundle:
            bundle.write(binary, binary_name)
    else:
        archive = dist / (basename + ".tar.gz")
        with tarfile.open(archive, "w:gz") as bundle:
            bundle.add(binary, arcname=binary_name)
    checksum = hashlib.sha256(archive.read_bytes()).hexdigest()
    archive.with_name(archive.name + ".sha256").write_text(checksum + "  " + archive.name + "\n")
    print("Built, tested, and verified " + str(archive), flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("target", choices=TARGETS, nargs="?")
    parser.add_argument("--self-test", action="store_true")
    options = parser.parse_args()
    if options.self_test:
        self_test()
    elif options.target:
        build(options.target)
    else:
        parser.error("supply a target or --self-test")
