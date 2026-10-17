# Published script-sound playback

`peacecraft3::script_audio::install` registers playback in `ScriptAudioPlayback`,
after `campaign_startup::CampaignStartupSet`. It reads the published
`MapScriptSession<MapStartupHost>` queue. A failed replacement keeps the current
session, its voices and its queued requests.

Each voice keeps the session's weak witness, also used by `CallbackTicket`.
A successful replacement, even with the same parsed context, changes that
witness. Playback retires the previous session's voices on the next update.
`clear` retires them too. Menu and click entities are unaffected.

The queue contains sound identities. Playback samples each identity's current
creation flags, properties and parameter assignment. Unsupported 3D, EAX,
out-of-range stop, fade, duration, channel, distance, integer volume, pitch,
parameter rows and `CreateMIDISound` are reported before copying playback bytes.
The request is consumed without spawning a player. Starting an already active
handle, including twice in one batch, is also reported: native restart versus
mix behavior remains unqualified. A finished one-shot handle can start again.
Distinct sound handles with the same digest share one source buffer.

The default staging and live encoded-byte ceiling is the checked sum of the
published Host's already admitted MIDI-label, creation-label, PCM-file and
MP3-file banks. This reads retained inventory without cloning it. Repeated
labels/files count separately, making the ceiling conservative. Repeated handles
do not increase it. The queue can only name resources from those immutable banks;
source sharing therefore cannot grow the cache beyond that inventory.
`ScriptAudioLimits` is an optional caller override for a tighter quota. A failed
staging attempt releases its new references and leaves the queue unchanged.
These ceilings bound encoded source copies, not total renderer, backend decoder
or voice metadata memory. No measured game budget is copied into a constant.

A playable request spawns an `AudioPlayer`. The recorded looping flag selects
`PlaybackSettings::LOOP`; a one-shot uses `PlaybackSettings::REMOVE`. An empty
one-shot sink or removed player retires the voice and releases its source.
The existing effects volume and mute control applies before sink creation and
to running script voices on native and browser targets. Its registered system
runs after settings updates and script playback. Unmapped native volume, pitch
and parameter assignments remain refused; backend gain and speed are recreation
policy, not measured native levels.

PCM uses the fmt/data-first `DecoderWave` view. MP3 keeps its encoded bytes. The
voice receipt retains the original SHA-256, member, source label and edition/version.
Both targets enable the existing Bevy `wav` and `mp3` features and use the same
playback consumer, source cache, quotas and session retirement. Installed-decoder
tests consume synthetic PCM and MP3 samples without an audio device.

Browser test artifacts expose the existing decoder, published-session and gain
cases through test-only exports. Their `no_main` attribute suppresses the standard
test entrypoint on WASM so the explicit gates can run; native test harnesses and
production entrypoints are unchanged. Generated WASM and binding code are not
patched. Executing these artifacts in Node checks WASM behavior, not Web Audio
device output.

The browser consumer's before-change failure was reproduced after loading the
actual Cargo WASM test artifact. After sharing playback and the existing effective
gain helpers, 21 explicit WASM cases pass: 11 decoder/lifecycle cases, nine
published-session cases and one case using the registered browser gain system.
They cover decoded PCM samples, nonzero MP3 samples, source reuse, refusals,
quotas, session replacement/clear, finished handles and effects gain/mute before
sink creation. Evidence:
`.scratch/browser-script-audio-root-20261018/review-3`; the before-change failure
is retained in `review-2/explicit-regression-confirmed.json`.

All 19 native app-library tests, 109 native session tests and six native
audio-control tests pass. The latter retain one explicit local-original-install
ignore; the rest of the app test suite was filtered out. Strict native/browser
all-target Clippy, the production browser build and formatting pass on unchanged
Rust/manifests throughout validation. The lockfile is unchanged.

These checks do not establish audibility, device output, native timing or mixing
parity. Chapter startup still needs caller-qualified inputs. The original Human01
startup frontier was not rerun for this browser change.

The current review environment cannot verify browser output: loopback binding
refuses with `Operation not permitted`, Chromium exits with `SIGTRAP` after the
same socket error, and the connected browser inventory is empty. No permission
or launch retry was made. Browser `AudioPlugin` output remains an acceptance gap.
