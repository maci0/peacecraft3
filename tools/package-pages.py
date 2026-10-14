#!/usr/bin/env python3
"""Package only demo code and compiled WASM; never copy game asset directories."""

import argparse
from pathlib import Path, PurePosixPath
import re
import tempfile


FILES = (
    ("demo.html", "index.html"),
    ("demo.css", "demo.css"),
    ("intro.css", "intro.css"),
    ("loader.js", "loader.js"),
    ("loading.js", "loading.js"),
    ("menu-audio.js", "menu-audio.js"),
    ("intro.js", "intro.js"),
    ("iso.js", "iso.js"),
    ("iso/import.js", "iso/import.js"),
    ("iso/audio.js", "iso/audio.js"),
    ("pkg/peacecraft3.js", "pkg/peacecraft3.js"),
    ("pkg/peacecraft3_bg.wasm", "pkg/peacecraft3_bg.wasm"),
)
IMPORT = re.compile(r"\b(?:import|export)\s+(?:[^;\n]*?\s+from\s+)?['\"]([^'\"]+)['\"]|\bimport\s*\(\s*['\"]([^'\"]+)['\"]")


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


def collect(source: Path) -> dict[str, bytes]:
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
    result[".nojekyll"] = b""
    return result


def package(source: Path, output: Path) -> list[str]:
    # Check every source before creating output. Output is disposable only when
    # empty; this command never removes a caller's files.
    for path in [output, *output.parents]:
        if path.is_symlink():
            raise ValueError(f"Symlink output path refused: {path}")
    if output.exists() and (not output.is_dir() or any(output.iterdir())):
        raise ValueError(f"Output must be absent or empty: {output}")
    if output.resolve() == source.resolve() or source.resolve() in output.resolve().parents:
        raise ValueError("Output must be outside the source tree")
    files = collect(source)
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
        names = package(source, root / "dist")
        assert names == sorted([target for _, target in FILES] + ["pkg/snippets/fixture/module.js", ".nojekyll"])
        assert not (root / "dist/assets").exists()
        assert not (root / "dist/pkg/snippets/fixture/unreferenced.js").exists()

        def refuses(output: str) -> None:
            try:
                package(source, root / output)
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
    print("Package self-test passed: allowlist, reachable snippets, nonempty output, missing files, symlinks and escaping imports.")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, default=Path(__file__).resolve().parents[1] / "web")
    parser.add_argument("--output", type=Path, default=Path(__file__).resolve().parents[1] / "dist")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    try:
        if args.self_test:
            self_test()
        else:
            names = package(args.source, args.output)
            print(f"Packaged {len(names)} allowlisted files into {args.output}")
            print("\n".join(names))
    except (OSError, ValueError) as error:
        parser.exit(2, f"package-pages: {error}\n")


if __name__ == "__main__":
    main()
