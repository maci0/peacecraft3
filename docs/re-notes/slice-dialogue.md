# Transmission subtitle visibility

`ObjectHost::admit_presentation` accepts `ForceCinematicSubtitles(boolean)` when
that declaration shares the `CreateTimer` source. The executed flag is
`PresentationState::cinematic_subtitles`. It stays absent until a script sets it.
A refused call leaves the previous flag. The call does not hide an active
`SetCinematicScene`, clear timed messages, or supply a default of shown or hidden.

`SetCinematicScene` already retains the speaker, text, portrait, player color,
scene duration, and voiceover duration. This step does not replace those fields
and does not claim chapter intro parity.

Transmission voice still starts through the sound host (`StartSound` from
`PlaySoundBJ`). This lane does not own that host. An existing sound record stays
there. The subtitle flag does not start, stop, or require playback.
