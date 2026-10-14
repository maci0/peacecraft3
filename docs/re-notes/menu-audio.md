# Original menu audio

The TFT menu uses the local game's `Sound\Music\mp3Music\War3XMainScreen.mp3`.
The extracted file is stereo, 44,100 Hz, 80.152786 seconds; SHA-256
`554256e6448e40de01946b9636c6c6f4243c84541cf2de61622a5ecc585b55b3`.
It loops in the browser and native launcher. Its initial volume is full scale;
the original engine's user music-volume setting is not yet imported.

`UI\SoundInfo\UISounds.slk` maps both `GlueScreenClick` and `MenuButtonClick`
to `Sound\Interface\BigButtonClick.wav`, volume 100, pitch 1, channel 8,
and `NODUPLICATES`. The launcher uses volume `100/127` and suppresses overlapping
click playback. No hover sound is added without an original mapping.
The decoded WAV is 17,336 bytes, PCM 16-bit mono, 22,050 Hz, 0.390295 seconds;
SHA-256 `1a7db9257b84022f03f79f53b9e6dd8f3c8c3d791c93338bf0571d58cea3affb`.

The music extracts with the existing archive reader:

```sh
./target/debug/mpqxtract get wc3/game/app/War3x.mpq \
  'Sound\Music\mp3Music\War3XMainScreen.mp3' web/assets/menu-music.mp3
```

The click uses compression mask `0x41`, which the Rust reader does not yet
decode. Its verified sector offsets and decryption can be exported separately:

```sh
cargo run -p pcx-mpq --example export_audio_sectors -- \
  wc3/game/app/war3.mpq 'Sound\Interface\BigButtonClick.wav' /tmp/click-sectors
i686-w64-mingw32-gcc -O2 -o /tmp/stormsector.exe tools/probe/stormsector.c
```

Place a copy of the **local game's** Storm.dll beside that executable, then
run it under Wine or 32-bit Windows with sector-directory and output-WAV
arguments. The probe calls ordinal 552 (RVA `0xE090`) using the measured four
argument stdcall ABI, verifies every decoded sector length and total output
length, and writes the original PCM WAV. Full archive lookup through Storm is
not required. This does not implement or claim to fix the Rust Huffman/ADPCM
codec gap.

The verified local files are in ignored `web/assets/menu-music.mp3` and
`web/assets/menu-click.wav`. Desktop distribution reads `assets/` beside the
executable, or these local development paths; music can also be read directly
from the installation configured by `PEACECRAFT3_INSTALL`.
No original game files are included in source or desktop build artifacts.
Browser playback starts on the first pointer or keyboard gesture, and the page
has a mute control. Native playback requires an audio device.

Menu panel event sounds (the hanging panel's pop-down) are not configured here:
`menu_sprites.rs` resolves each model `SNDX` event through `AnimLookups.slk`
and `AnimSounds.slk` and plays the WAV at `Volume / 127`, natively through Bevy
audio and in the browser through `peacecraft3Audio.event`. See
[sprite rendering](menu-sprite-rendering.md).
