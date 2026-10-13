# Windows and macOS builds

The desktop matrix builds the actual `peacecraft3` game executable with the
workspace's pinned Rust 1.99.0 compiler and Cargo.lock. Each job compiles and
executes the binary's unit tests on the target OS and CPU, then verifies the
executable header before uploading an archive and SHA-256 checksum.

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
Checkout v5 and upload-artifact v6 both use the Node.js 24 action runtime;
[upload-artifact v6](https://github.com/actions/upload-artifact/tree/v6) requires
Actions runner 2.327.1 or newer, as provided by the hosted runners.

## Run a build

On a Windows or macOS machine of the matching architecture, install Rust through
rustup and the platform compiler (Visual Studio C++ build tools on Windows;
`xcode-select --install` on macOS). Substitute the target from the table:

```sh
rustup toolchain install 1.99.0-aarch64-apple-darwin --profile minimal
python3 tools/build-desktop.py aarch64-apple-darwin
```

On Windows use `python` instead of `python3`, for example:

```powershell
rustup toolchain install 1.99.0-aarch64-pc-windows-msvc --profile minimal
python tools/build-desktop.py aarch64-pc-windows-msvc
```

The helper rejects a different compiler host, builds release binaries, runs unit
tests in that same release profile, checks PE / Mach-O OS and CPU headers, and
packages into `dist/`. Tests reuse the built dependencies instead of compiling
Bevy again in a second profile.
macOS tar archives preserve executable permissions during artifact download.
The workflow `.github/workflows/desktop-builds.yml` runs this same helper on
pushes / pull requests affecting the build, and offers a manual **Desktop builds**
action. Download the four separately named artifacts from a successful workflow
run. Each job is independent, so one failed target does not cancel the others.

## Validation status and limits

The matrix passes actionlint and YAML validation; the packaging helper's check
covers all four binary headers and rejects wrong-CPU / malformed inputs. On the
Linux development host, the Windows x64 binary and test code passed Rust 1.99.0
cross-target checks using clang-cl and llvm-lib:

```sh
CC_x86_64_pc_windows_msvc=clang-cl AR_x86_64_pc_windows_msvc=llvm-lib \
  cargo +1.99.0 check --locked --target x86_64-pc-windows-msvc \
  -p peacecraft3 --tests -j 2
```

That checks the Windows x64 Rust code and builds Blake3's target assembly, but
does not link a Windows executable. ARM64 checking stopped at Blake3's NEON C
code because the Microsoft CRT header `assert.h` is absent on this Linux host;
no code or features were changed to bypass that dependency.

Source is in the private [maci0/peacecraft3 repository](https://github.com/maci0/peacecraft3).
The [first native four-target run](https://github.com/maci0/peacecraft3/actions/runs/38409132391)
compiled Windows ARM64 and macOS ARM64. Their test phase failed because a
local-install integration test assumed developer folders existed on the runner;
that test is now explicitly ignored by default. The [corrected four-target run](https://github.com/maci0/peacecraft3/actions/runs/38419274743) passed compilation, native unit tests, executable-header checks and artifact upload on **all four OS/CPU combinations** at commit `3055771d80a8b4f28fb06162c52652051757ca5d`. All four downloaded archives passed independent checksum and executable-header checks; both macOS tar archives preserve executable permissions. These artifacts predate the later TFT selection, startup movie and coordinate-layout changes; subsequent source pushes trigger a new matrix run. Native GPU/audio behavior on Windows/macOS is not established by build and unit-test success. The native Windows runners include the actual CRT headers and SDK missing from the Linux cross-check environment.

Archives contain only the executable; players supply their own Warcraft III
assets. Point `PEACECRAFT3_INSTALL` at a directory containing the original MPQs:

```powershell
$env:PEACECRAFT3_INSTALL = 'C:\Games\Warcraft III'
.\peacecraft3.exe
```

```sh
PEACECRAFT3_INSTALL='/path/to/Warcraft III' ./peacecraft3
```

The original TFT music can be read from that installation. For decoded menu
audio files, create an `assets` directory beside the executable containing
`menu-music.mp3` and `menu-click.wav`, extracted from the player's own install.
The current native reader cannot decompress the click's Huffman/ADPCM sectors,
so the decoded WAV is required for click feedback.

Asset tests skip when the original installation is absent. Compilation
and unit tests do not prove menu rendering or sound on the target machine: that
still requires running the game with a real GPU and the user's installation.
These are unsigned executables, without installers, notarization, or signing.
The workflow runs on macOS 15; compatibility with older macOS versions has not
been verified.
