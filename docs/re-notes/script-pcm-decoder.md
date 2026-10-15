# Checked script PCM decoder view

`pcx_play::pcm_decoder::DecoderWave::read` derives a bounded backend byte view
from an already admitted `PcmResource`. It does not consume `StartSound` work,
initialize a device, or wire playback into the launcher.

The PCM inspector accepts framing that the installed native Bevy 0.20
`AudioSource` decoder rejects. In particular, the installed hound WAV reader
requires format before data and does not skip odd unknown-chunk padding while
searching for data. The view orders the original fmt chunk, original data chunk,
then all ancillary chunks in their original relative order. Every chunk header,
payload and padding byte is retained verbatim; the RIFF size and PCM samples do
not change. Original source bytes and receipt remain attached through `source()`.
The derived byte digest must not be substituted for the original digest.

The current decoder profile admits integer mono/stereo 8/16-bit PCM and fmt
lengths 16, 18 or 40, following the installed hound PCM framing contract. Nonzero
PCM cbSize and extension bytes are retained without interpretation. Other fmt
lengths fail explicitly, even if the broader source inspector accepts them.
Malformed source data fails resource admission. A zero-length data chunk stays
empty and decodes to no samples; no audible event or silence duration is inferred.

Before the one additional RIFF buffer is allocated/copied, the view checks its
byte/platform Layout budget, revalidates source framing and retained metadata,
and checks the decoder profile. Allocation is fallible. Errors retain source-label
context. Work is linear in chunks plus copied bytes. The copy budget is not a
total heap/RSS bound; application caching and Vec-to-Arc conversion need their
own aggregate and peak-allocation limits. Transferring the derived bytes also
requires the caller to retain the original provenance beside the backend cache.

This is a sample-decoding compatibility view. Opaque ancillary file-offset
semantics are not interpreted or rewritten. It does not replace the original
resource for other consumers, establish effective gain/pitch/loop/fade/channel/
spatial settings, or prove audible/native fidelity. Browser format support,
gesture handling, session cancellation and delivery acknowledgement remain
separate integration boundaries.

Synthetic core tests require no Bevy and check PCM/resource/receipt/chunk/padding
preservation, geometry, empty data, malformed input, profile refusal and platform
limits. Native application tests compare actual installed Bevy decoder sample
bits with canonical synthetic controls, including allowed PCM extension bytes;
they also retain independent regressions for the rejected original framing.
These tests create no App, audio sink or device and use no original payloads.
Normal locked core native/WASM builds and strict Clippy pass. The three core test
groups execute in actual 32-bit WASM; all seven installed native decoder consumer
groups pass against the freshly built dependency graph. No device is opened.
Command receipts are local in `.scratch/audio-banks-root/` and
`.scratch/script-audio-pcm-integration-20261015/`.
