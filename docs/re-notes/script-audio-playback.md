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

The initial restricted review could not launch Chromium or bind loopback. After
an explicit environment permission change, Chromium 154 could run the actual
factory/consumer with Bevy's `AudioPlugin`. Creating the plugin inside a click
worked, but matching launcher startup exposed a failure: a context created before
interaction stayed suspended after the click. The existing menu gesture handler
only unlocked HTML media.

`web/menu-audio.js` now retains live Web Audio contexts through a constructor
subclass and resumes suspended contexts from its existing pointer/key handler,
before the HTML-media guards. Both launcher pages load that module before the
loader. Closed contexts leave the collection; resume failures use the existing
diagnostic path. Native playback and sound gain/mute policy are unchanged.

The browser regression loads the actual menu module and Cargo WASM test artifact
over loopback, using only synthetic PCM/MP3. Each codec uses a fresh document and
passive protocol reads with `userGesture: false`: the context is suspended and
the document unactivated before a trusted mouse click. Before the fix it remains
suspended with both voices active. Afterward the one-shot retires naturally and
the remaining loop produces nonzero samples at the real Web Audio output graph.
Session clear releases all voices and the shared source. Nine published-session
cases also execute in each document. Eleven JavaScript audio tests, 109 native
session tests, strict native/browser Clippy and formatting pass on unchanged
sources during validation. Evidence:
`.scratch/browser-script-audio-root-20261018/device-6-after/acceptance.json`;
before-change state: `device-5-before/failure-pcm.json`.

This verifies the browser output graph, not physical audibility or original
sound levels, mixing and timing. The public site package was not rebuilt.
