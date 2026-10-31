# Filename-and-label sound construction

`CreateSound` already binds a caller filename from the admitted file bank or
`admit_file_sound_loader`. `CreateSoundFilenameWithLabel` is a separate
constructor. Its last argument is an SLK label, not EAX text. The 1.29 declaration is `native CreateSoundFilenameWithLabel takes string fileName, boolean looping, boolean is3D, boolean stopwhenoutofrange, integer fadeInRate, integer fadeOutRate, string SLKEntryName returns sound`.

When the loaded program declares it, `with_file_sounds` requires the same
nonconstant signature and declaration source as `CreateSound`. A call resolves
that label in the admitted parameter banks before any file-source read. An
empty, NUL-containing, or unadmitted label refuses without a sound identity and
without invoking the file loader. The filename then uses the existing
`CreateSound` lookup. Repeated filenames share resource bytes and get distinct
sound identities.

The identity stores the executed looping, 3D, out-of-range, and fade-rate
arguments, an empty EAX string, and the parameter-row key. The row's volume,
pitch, channel, distances, and `EAXFlags` stay on that assignment. They are
not copied into sound properties or the EAX string. Playback, fades, and EAX
application are unchanged. Campaign completion and audible output are not
established.
