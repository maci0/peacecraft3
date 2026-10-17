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

`ScriptAudioLimits` must be supplied. A non-empty queue stays queued if limits
are absent or staging admission fails. `max_staged_bytes` bounds source bytes
copied in one batch; `max_live_bytes` bounds the retained encoded source cache.
Both are checked before `DecoderWave` or MP3 copies. These are not total renderer,
backend decoder or voice metadata memory limits. A failed attempt releases its
new source references and leaves the queue unchanged.

A playable request spawns an `AudioPlayer`. The recorded looping flag selects
`PlaybackSettings::LOOP`; a one-shot uses `PlaybackSettings::REMOVE`. An empty
one-shot sink or removed player retires the voice and releases its source.
The existing effects volume and mute control applies before sink creation and
to running script voices. Unmapped native volume, pitch and parameter assignments
remain refused; backend gain and speed are recreation policy, not measured native
levels.

PCM uses the fmt/data-first `DecoderWave` view. MP3 keeps its encoded bytes. The
voice receipt retains the original SHA-256, member, source label and edition/version.
Installed-decoder tests consume synthetic PCM and MP3 samples without an audio
device. The browser build has no Bevy `wav` or `mp3` feature: playback reports a
non-empty queue and leaves it queued.

These checks do not establish audibility, device output, native timing or mixing
parity. Chapter startup still needs caller-qualified inputs and currently stops
at unsupported `TimerStart` on the retained Human01 gate.
