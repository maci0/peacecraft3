#!/usr/bin/env python3
"""Package only demo code and compiled WASM; never copy game asset directories."""

import argparse
import gzip
import hashlib
import io
from pathlib import Path, PurePosixPath
import re
import tarfile
import tempfile


FILES = (
    ("demo.html", "index.html"),
    ("demo.css", "demo.css"),
    ("intro.css", "intro.css"),
    ("loader.js", "loader.js"),
    ("local-archives.js", "local-archives.js"),
    ("loading.js", "loading.js"),
    ("map-configuration.js", "map-configuration.js"),
    ("menu-audio.js", "menu-audio.js"),
    ("menu-settings.js", "menu-settings.js"),
    ("intro.js", "intro.js"),
    ("iso.js", "iso.js"),
    ("iso/import.js", "iso/import.js"),
    ("iso/audio.js", "iso/audio.js"),
    ("iso/installer.js", "iso/installer.js"),
    ("iso/freearc.js", "iso/freearc.js"),
    ("iso/arc-extract.js", "iso/arc-extract.js"),
    ("iso/arc-worker.js", "iso/arc-worker.js"),
    ("iso/engine-member.js", "iso/engine-member.js"),
    ("iso/installer-import.js", "iso/installer-import.js"),
    ("pkg/freearc/freearc-codec.mjs", "pkg/freearc/freearc-codec.mjs"),
    ("pkg/freearc/freearc-codec.wasm", "pkg/freearc/freearc-codec.wasm"),
    ("pkg/peacecraft3.js", "pkg/peacecraft3.js"),
    ("pkg/peacecraft3_bg.wasm", "pkg/peacecraft3_bg.wasm"),
    ("licenses/bevy-particle-shader.txt", "licenses/bevy-particle-shader.txt"),
)
IMPORT = re.compile(r"\b(?:import|export)\s+(?:[^;\n]*?\s+from\s+)?['\"]([^'\"]+)['\"]|\bimport\s*\(\s*['\"]([^'\"]+)['\"]")
CODEC_LOCAL = ("README.md", "build.sh", "upstream.sha256", "upstream-exe.json", ".gitignore",
               "src/codec.cpp", "src/lzma_dec.c", "src/aes.c",
               "js/freearc-codec.mjs", "tests/native_test.cpp", "tests/codec.test.mjs",
               "tests/exe_stage_test.cpp", "tests/exe-stage.test.mjs")
CODEC_OFFER = "pkg/freearc/freearc-codec-source.tar.gz"


def regular_file(root: Path, relative: str) -> Path:
    parts = PurePosixPath(relative).parts
    if not parts or any(part in ("..", ".") for part in parts) or relative.startswith("/"):
        raise ValueError(f"Unsafe source path: {relative}")
    path = root
    for part in parts:
        path /= part
        if path.is_symlink():
            raise ValueError(f"Symlink source refused: {path}")
    if not path.is_file():
        raise ValueError(f"Missing regular source file: {path}")
    return path


def codec_source(root: Path) -> bytes:
    """Offer only checked corresponding decoder source, never build outputs."""
    if any(path.is_symlink() for path in [root, *root.parents]):
        raise ValueError(f"Symlink codec source refused: {root}")
    files = {name: regular_file(root, name).read_bytes() for name in CODEC_LOCAL}
    for line in files["upstream.sha256"].decode("ascii").splitlines():
        match = re.fullmatch(r"([0-9a-f]{64})  (\S+)", line)
        if not match:
            raise ValueError("Malformed codec upstream manifest")
        digest, relative = match.groups()
        name = f"upstream/{relative}"
        if name in files:
            raise ValueError("Duplicate codec source path")
        data = regular_file(root, name).read_bytes()
        if hashlib.sha256(data).hexdigest() != digest:
            raise ValueError(f"Codec upstream source hash mismatch: {relative}")
        files[name] = data
    if "upstream/LICENSE" not in files:
        raise ValueError("Codec license is missing from pinned source manifest")
    output = io.BytesIO()
    with gzip.GzipFile(fileobj=output, mode="wb", mtime=0) as compressed:
        with tarfile.open(fileobj=compressed, mode="w") as archive:
            for name, data in sorted(files.items()):
                member = tarfile.TarInfo(f"freearc-codec/{name}")
                member.size = len(data)
                member.mode = 0o755 if name == "build.sh" else 0o644
                archive.addfile(member, io.BytesIO(data))
    return output.getvalue()


def collect(source: Path, codec: Path) -> dict[str, bytes]:
    if any(path.is_symlink() for path in [source, *source.parents]) or not source.is_dir():
        raise ValueError(f"Source must be a non-symlink directory: {source}")
    result = {}
    for original, target in FILES:
        if target in result:
            raise ValueError(f"Duplicate output name: {target}")
        data = regular_file(source, original).read_bytes()
        if target.endswith(".wasm"):
            if not data.startswith(b"\0asm\x01\0\0\0"):
                raise ValueError(f"Invalid WebAssembly header: {original}")
        else:
            try:
                data.decode("utf-8")
            except UnicodeDecodeError as error:
                raise ValueError(f"Non-text code file refused: {original}") from error
        result[target] = data

    # wasm-bindgen sometimes emits imported JS snippets. Include only reachable
    # modules under pkg/snippets, never an entire generated directory.
    pending = ["pkg/peacecraft3.js"]
    while pending:
        importer = pending.pop()
        for match in IMPORT.finditer(result[importer].decode("utf-8")):
            reference = match.group(1) or match.group(2)
            if not reference.startswith("."):
                raise ValueError(f"Nonlocal generated module import: {reference}")
            relative = PurePosixPath(importer).parent / reference
            if ".." in relative.parts or not str(relative).startswith("pkg/snippets/") or relative.suffix != ".js":
                raise ValueError(f"Generated module import outside snippet allowlist: {reference}")
            target = str(relative)
            if target in result:
                continue
            data = regular_file(source, target).read_bytes()
            data.decode("utf-8")
            result[target] = data
            pending.append(target)
    result[CODEC_OFFER] = codec_source(codec)
    result[".nojekyll"] = b""
    return result


def package(source: Path, output: Path, codec: Path) -> list[str]:
    # Check every source before creating output. Output is disposable only when
    # empty; this command never removes a caller's files.
    for path in [output, *output.parents]:
        if path.is_symlink():
            raise ValueError(f"Symlink output path refused: {path}")
    if output.exists() and (not output.is_dir() or any(output.iterdir())):
        raise ValueError(f"Output must be absent or empty: {output}")
    if output.resolve() == source.resolve() or source.resolve() in output.resolve().parents:
        raise ValueError("Output must be outside the source tree")
    files = collect(source, codec)
    output.mkdir(parents=True, exist_ok=True)
    for relative, data in files.items():
        target = output / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        with target.open("xb") as stream:
            stream.write(data)
    return sorted(files)


def self_test() -> None:
    with tempfile.TemporaryDirectory(prefix="pages-package-") as directory:
        root = Path(directory)
        source = root / "web"
        source.mkdir()
        codec = root / "codec"
        codec.mkdir()
        for name in CODEC_LOCAL:
            path = codec / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("synthetic source fixture\n")
        license = codec / "upstream/LICENSE"
        license.parent.mkdir(parents=True)
        license.write_text("synthetic license fixture\n")
        (codec / "upstream.sha256").write_text(f"{hashlib.sha256(license.read_bytes()).hexdigest()}  LICENSE\n")
        for original, _ in FILES:
            path = source / original
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b"\0asm\x01\0\0\0" if path.suffix == ".wasm" else b"synthetic fixture\n")
        forbidden = source / "assets" / "original.mpq"
        forbidden.parent.mkdir()
        forbidden.write_bytes(b"MPQ\x1aoriginal must stay local")
        snippet = source / "pkg/snippets/fixture/module.js"
        snippet.parent.mkdir(parents=True)
        snippet.write_text("export const fixture = true;\n")
        (source / "pkg/peacecraft3.js").write_text("import { fixture } from './snippets/fixture/module.js';\n")
        stray = snippet.parent / "unreferenced.js"
        stray.write_text("not shipped")
        (source / "iso/engine-member.js").write_text("export function planEngineMembers() { return null; }\n")
        (source / "iso/arc-worker.js").write_text("import { planEngineMembers } from './engine-member.js';\n")
        names = package(source, root / "dist", codec)
        assert names == sorted([target for _, target in FILES] + ["pkg/snippets/fixture/module.js", CODEC_OFFER, ".nojekyll"])
        assert (root / "dist/iso/engine-member.js").read_text() == "export function planEngineMembers() { return null; }\n"
        assert (root / "dist/iso/arc-worker.js").read_text() == "import { planEngineMembers } from './engine-member.js';\n"
        assert not (root / "dist/assets").exists()
        assert not (root / "dist/pkg/snippets/fixture/unreferenced.js").exists()

        def refuses(output: str) -> None:
            try:
                package(source, root / output, codec)
            except ValueError:
                return
            raise AssertionError(f"Expected refusal for {output}")

        refuses("dist")
        target = source / "demo.css"
        target.unlink()
        refuses("missing")
        assert not (root / "missing").exists()
        target.symlink_to(forbidden)
        refuses("symlink")
        assert not (root / "symlink").exists()
        target.unlink()
        target.write_text("synthetic fixture")
        (source / "pkg/peacecraft3.js").write_text("import '../../assets/original.mpq';")
        refuses("escape")
        assert not (root / "escape").exists()
        (source / "pkg/peacecraft3.js").write_text("synthetic module")
        license.write_text("changed source")
        refuses("codec-hash")
        assert not (root / "codec-hash").exists()
    print("Package self-test passed: allowlist, reachable snippets, nonempty output, missing files, symlinks and escaping imports.")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, default=Path(__file__).resolve().parents[1] / "web")
    parser.add_argument("--output", type=Path, default=Path(__file__).resolve().parents[1] / "dist")
    parser.add_argument("--codec-source", type=Path, default=Path(__file__).resolve().parent / "freearc-codec")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    try:
        if args.self_test:
            self_test()
        else:
            names = package(args.source, args.output, args.codec_source)
            print(f"Packaged {len(names)} allowlisted files into {args.output}")
            print("\n".join(names))
    except (OSError, ValueError) as error:
        parser.exit(2, f"package-pages: {error}\n")


if __name__ == "__main__":
    main()
