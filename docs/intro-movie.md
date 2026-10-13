# Original opening cinematic

The PeaceCraft settings screen appears first. Its Main menu action requests the startup cinematic once per launch, then the menu entrance starts after natural movie ending, Escape, the visible Skip cinematic button, or an unavailable/failed decoder. Returning to settings and entering the menu again does not replay the movie.

`intro::Intro` exposes `request(expansion: bool) -> bool`, `playing() -> bool`, and `take_finished() -> bool`. `request_with_install(expansion, Option<PathBuf>)` also lets the native caller pass the selected installation directory, which takes precedence over the viewer lookup. `request` returns true only on the first request; that true result means the menu must wait. `take_finished` consumes one completion notification. `intro::install(&mut App)` registers browser/native playback systems. The caller hides settings while `playing()` and enters the menu when completion is consumed. Menu music pauses during playback and resumes after completion; the cinematic owns its soundtrack.

## Original content and local preparation

The supplied legacy installation's `Movies/IntroX.mpq` is the TFT opening: its extension is misleading; it is RIFF AVI containing MPEG-4 video and MP3 audio, 800×368, 24 fps, 188.834844 seconds. Its SHA-256 is `fc1689eeff44b657bb0e93a950bf00d7e712a5c869a81f7aba958fb50b61acef`, identical to the supplied older installation's movie. This byte identity does not independently establish the original encoders or native startup dispatcher for 1.29b.

`Movies/TutorialOp.mpq` is the base-game cinematic selected for RoC: locally decoded frames show the sunlit field and Thrall's prophetic vision, distinct from Illidan's TFT cinematic. It is also MPEG-4 plus MP3, 800×368, approximately 173.45 seconds. SHA-256: `a3e47b9f86f01f0b5d2a2f6289af103a70013ca7535543d32693bcaa44847346`. The native startup call selecting this name has not been traced; this selection is based on the original file's inspected cinematic content.

Browsers receive supported local H.264/AAC copies, with the original image and soundtrack rather than replacement art. Set the installation path and convert only files you already have:

```sh
ffmpeg -i "$PEACECRAFT3_INSTALL/Movies/IntroX.mpq" -map 0:v:0 -map 0:a:0 -c:v libx264 -preset fast -crf 19 -pix_fmt yuv420p -c:a aac -b:a 160k -movflags +faststart web/assets/intro-tft.mp4
ffmpeg -i "$PEACECRAFT3_INSTALL/Movies/TutorialOp.mpq" -map 0:v:0 -map 0:a:0 -c:v libx264 -preset fast -crf 19 -pix_fmt yuv420p -c:a aac -b:a 160k -movflags +faststart web/assets/intro-roc.mp4
```

The generated `web/assets` movie files and analysis frames are ignored by Git. They are not downloaded, included in source publication, or uploaded to a remote service. Browser playback currently reads these staged local URLs; direct ISO/movie import into browser memory is a separate asset-import requirement, not implemented by this player.

## Playback

The browser uses its video decoder with `object-fit: contain`, preserving the original 800:368 aspect ratio and black letterboxing. CSS uses normal flex flow and relative dimensions. Escape and the visible skip control always remain available. The movie inherits the existing sound setting and follows changes while playing. Missing media, denied playback or a decoder error completes the flow, and a ten-second startup timeout handles a load that never starts. JavaScript calls `peacecraft3Audio.pauseForIntro()` and `resumeAfterIntro()` around playback; callers must provide these music hooks. Add `intro.css` to the page and load `intro.js` before the WASM launcher.

The native player decodes through an installed `ffmpeg` executable, with no build-time FFmpeg dependency. It reads the original movie from the caller's selected installation, or the same `PEACECRAFT3_INSTALL`/`PEACECRAFT3_LEGACY_DIR` lookup as the viewer when no override is passed; staged movies beside the executable in `assets` or under `web/assets` are fallbacks. For the named original `IntroX.mpq` and `TutorialOp.mpq`, the native player checks a bounded AVI header for `RIFF`/`AVI ` and audio stream format tag `0x0055` (MP3), then uses FFmpeg codec copy to preserve the existing compressed soundtrack without another lossy encoding. A generic RIFF file is not assumed to be MP3. Unrecognized headers and staged H.264/AAC MP4 files take the MP3 transcode path for Bevy. A worker streams RGBA video paced by FFmpeg's original input clock. The first frame starts its Bevy soundtrack. A two-message bounded queue drops video frames when presentation is slower than decoding, and the audio allocation is capped at 32 MiB. Native video dimensions scale relative to the window and preserve aspect. On natural video EOF, the final frame remains until the existing movie AudioSink finishes; a newly queued soundtrack is allowed to start first. A missing sink permits completion, so unavailable audio output cannot strand the launcher. Skip, failure and dropping the app immediately kill/reap the decoder process and join its worker. No external player window or per-frame subprocess is used.

FFmpeg is a runtime requirement for native cinematic playback; if it is missing, the menu still opens. This is separate from compiling and running the game. The FFmpeg input clock starts before the first decoded frame starts the Bevy soundtrack, so decoder/render/audio-output latency still permits a small A/V offset. Native image/audio output has not been compared against an original client for frame-perfect A/V synchronization or original skip-key timing. The extra visible skip control is the requested accessible control.

## Verification scope

The state-machine regression checks that completion is consumed once and subsequent requests cannot replay the movie. The ignored `original_movie_decodes_and_skip_reaps_the_process` test requires local media and FFmpeg: it checks decoded frame size, nonempty soundtrack, prompt cancellation, child reaping and worker joining. Browser checks play the actual converted local movie, verify aspect containment, and exercise Escape, skip button, natural ending after seeking near the end, and missing-media fallback. The browser fixture uses the real menu-audio module and verifies that music pauses during playback and resumes with a progressing clock after all four finish paths; this is distinct from end-to-end launcher input and scene verification.

```sh
cargo test --locked -p peacecraft3 --bin peacecraft3 intro::tests
cargo test --locked -p peacecraft3 --bin peacecraft3 original_movie_decodes_and_skip_reaps_the_process -- --ignored
```
