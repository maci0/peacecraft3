# Encoded MPEG audio on script sound objects

`file_sounds::Assets::read_encoded` admits integer PCM and MP3 into the existing
filename bank. `Assets::read` remains PCM-only. Whole-bank input, identity,
duplicate-member and metadata checks run before decoding or retaining source
copies. The existing `ObjectHost::with_file_sounds` consumes either bank.

PCM resources and their APIs remain unchanged. MP3 resources retain the original
encoded bytes, member spelling, source label, edition/version identity and
SHA-256. Their frame report comes from decoding, without making a synthetic WAV
or replacing the input's identity. File extensions do not choose the codec.
`file_mp3_state` exposes the same creation inputs as `file_sound_state`, using
the latter's generic resource type. PCM-only access refuses an MP3 identity.

`StartSound`, `SetSoundDuration` and `SetSoundParamsFromLabel` validate either
resource against the same shared handle arena. Creation flags, fade rates, EAX
text and assigned duration stay the exact script arguments. Parameter rows keep
their own table/source receipts; no gain, channel, distance or timing convention
is inferred from MP3 metadata. Start requests are still a queue, not playback.

## Decoder and limits

The direct dependency is [Symphonia 0.6.1](https://docs.rs/symphonia/0.6.1/symphonia/),
with only its MP3 feature enabled. The application's existing rodio dependency
still requires its separate 0.5.5 dependency; their buffer types are not mixed.

Caller limits cover encoded input, the owned input copy/read-ahead ring, decoder
calls, packets, decoded frames and samples. Platform layout and checked-arithmetic
failures refuse before the corresponding retained allocation or count update.
Decoder buffers belong to the library; the input-allocation limit is not a total
heap bound. Decode limits apply to each resource, while filename-bank byte and
metadata limits apply to the whole collection.

Samples are inspected packet by packet and hashed in native little-endian planar
order; no decoded waveform is retained. The reported sample kind, channel count
and rate come from the decoder, and inconsistent signal changes refuse. Gapless
trimming is explicitly disabled for inspection; delay and padding remain
separate metadata. `Track::num_frames` may be estimated and is not substituted
for decoded frames or interpreted as an authoritative duration.

MPEG audio has no explicit stream end. The
[pinned reader](https://github.com/pdeljanov/Symphonia/blob/v0.6.1/symphonia-bundle-mp3/src/demuxer.rs)
can accept a cut input containing complete frames. Tests retain that distinction:
short/non-audio inputs refuse, while a decodable prefix keeps its own length,
hash and smaller frame count. This is not a completeness or integrity validator;
archive extraction and the caller's source digest remain separate checks.

## Executed evidence

Three self-authored tone fixtures cover mono/stereo and tagged/untagged MPEG.
Their complete metadata reports and decoded sample hashes are identical on native
and executed 32-bit WASI. No game recording is included in these fixtures; the
generation recipe is beside them.

An explicit ignored gate reads one caller-selected local archive/member under
packed/decoded-byte limits and checks the caller's expected encoded SHA-256.
The selected `War3Local.mpq` member
`Sound\Dialogue\HumanCampaign\Human01\H01Arthas02.mp3` has 44,669 encoded bytes,
SHA-256 `f62e18571aeba54d35e79cef1144f8ac00d8f101da080df56b10aa97734f7cb8`.
The current decoder reports mono, 44,100 Hz, 171 packets and 196,992 untrimmed
frames; its format-reader frame estimate is 195,840. No original encoded or
decoded audio was saved by this gate. This identifies the selected archive
member, not an archive release or native lookup priority.

The same pinned Human01 Program/localization probe completes globals and config.
All 58 direct file bindings admit: 23 PCM and 35 MP3. All 51 distinct parameter
labels admit through the existing multi-table parameter bank. Actual execution
then creates 58 sound objects and performs 58 label and duration assignments.
The next refusal is `Rect` at map span `76819..76861`, in region initialization.
The probe uses an explicitly identified compatibility object catalog; it is not
target-native catalog or full campaign acceptance.

Private commands, inputs and complete outputs are retained under
`.scratch/cursor-gameplay-20261016/root-mp3-integration/`. Tests also cover mixed
bank provenance, preflight-before-decode failures, codec limits, PCM API retention,
and a real VM retaining an MP3's parameters, duration and queued start after a
later unsupported native. Strict native/WASM checks are distinct from playback.

Campaign startup now attaches executed direct-file source loading; automatic
label/parameter banks and the other Host inputs remain open. The browser playback
dependency, device output, native decoder/
gapless behavior, 3D effects and complete campaign remain unverified.
