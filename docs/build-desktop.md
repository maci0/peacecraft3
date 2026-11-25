# Windows and macOS builds

The desktop matrix builds the actual `peacecraft3` game executable with the
latest stable Rust compiler and committed `Cargo.lock`. Build and test commands
treat warnings as errors, including when `RUSTFLAGS` is already set. Workspace
crates apply the same policy to local Cargo commands. Each job builds and tests
the executable on its target OS and CPU, verifies its header, and uploads an
archive with a SHA-256 checksum.

| OS | CPU | Rust target | GitHub runner | Download |
| --- | --- | --- | --- | --- |
| Windows | Intel / AMD x64 | `x86_64-pc-windows-msvc` | `windows-2025` | `.zip` |
| Windows | ARM64 | `aarch64-pc-windows-msvc` | `windows-11-arm` | `.zip` |
| macOS | Intel x64 | `x86_64-apple-darwin` | `macos-15-intel` | `.tar.gz` |
| macOS | Apple Silicon | `aarch64-apple-darwin` | `macos-15` | `.tar.gz` |

These explicit runner labels match the architectures documented in
[GitHub's runner reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners).
Windows uses the
[MSVC targets](https://doc.rust-lang.org/rustc/platform-support/windows-msvc.html)
and the runner's Visual Studio C++ tools / Windows SDK; macOS uses the
[Darwin targets](https://doc.rust-lang.org/rustc/platform-support/apple-darwin.html)
and Xcode's SDK. The local Python helper uses only the standard library.
[upload-artifact v6](https://github.com/actions/upload-artifact/tree/v6) requires
Actions runner 2.327.1 or newer, as provided by the hosted runners.

## Run a build

On a Windows or macOS machine of the matching architecture, install Python 3,
Rust through rustup, and the platform compiler. Use Visual Studio C++ build
tools on Windows or `xcode-select --install` on macOS. Substitute the target from the table:

```sh
rustup toolchain install stable-aarch64-apple-darwin --profile minimal
python3 tools/build-desktop.py aarch64-apple-darwin
```

On Windows use `python` instead of `python3`, for example:

```powershell
rustup toolchain install stable-aarch64-pc-windows-msvc --profile minimal
python tools/build-desktop.py aarch64-pc-windows-msvc
```

The helper rejects a different compiler host, builds release binaries, runs unit
tests in that same release profile, checks PE / Mach-O OS and CPU headers, and
packages into `dist/`. Tests reuse the built dependencies instead of compiling
Bevy again in a second profile.
macOS tar archives preserve executable permissions during artifact download.
The workflow `.github/workflows/desktop-builds.yml` runs this same helper on
pushes / pull requests affecting the build, and offers a manual **Desktop builds**
action. Download the target archive and SHA-256 checksum from its successful job.
Artifacts are retained for 14 days. Each job is independent, so a failure on
one target does not cancel the others.

## Run the packaged game

Archives contain only the executable; players supply their own Warcraft III
assets. Point `PEACECRAFT3_INSTALL` at a directory containing the original MPQs:

```powershell
$env:PEACECRAFT3_INSTALL = 'C:\Games\Warcraft III'
.\peacecraft3.exe
```

```sh
PEACECRAFT3_INSTALL='/path/to/Warcraft III' ./peacecraft3
```

The startup cinematic uses `ffmpeg` on `PATH`. The game menu can open
without it.

## Validation status

The [0.19.0 desktop run](https://github.com/maci0/peacecraft3/actions/runs/44262745099)
passed both Windows jobs. Its macOS ARM64 job failed in the
`audio_wall_sample_declares_its_actual_platform_origin` test. The macOS code
path returns an unsupported-clock error before supplying a boot-relative
sample. The macOS Intel job was cancelled after hitting its 90-minute timeout.
The run did not produce verified macOS artifacts.

In October 2026, four-target runs passed at
[`3055771`](https://github.com/maci0/peacecraft3/actions/runs/38419274743)
and [`d31dc67`](https://github.com/maci0/peacecraft3/actions/runs/38439725394).
All four archives from each run passed independent SHA-256 and executable
header checks. The macOS archives preserved execute permissions. An
[earlier run](https://github.com/maci0/peacecraft3/actions/runs/38409132391)
found a test that depended on a local installation. That test is now ignored by
default. Billing blocked the later October run at
[`122ecd0`](https://github.com/maci0/peacecraft3/actions/runs/38456169192)
before its jobs started.

The packaging self-test covers all four executable headers and rejects
malformed or wrong-architecture inputs. A historical Linux cross-check with
Rust 1.99.0 checked the Windows x64 Rust code and built Blake3 assembly without
linking a Windows executable. The ARM64 cross-check stopped at missing
Microsoft CRT headers. The native Windows runners supply the SDK and CRT.

The runners do not supply original game assets. Tests that need them are
ignored or return early without exercising those paths. Native rendering and
audio behavior remain unverified.
The executables are unsigned and ship without installers or notarization.
Compatibility with macOS versions older than the macOS 15 runners is unverified.
