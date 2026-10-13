//! Once-per-launch playback of the user's original startup cinematic.
use bevy::prelude::*;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct IntroUpdate;

#[derive(Resource, Default)]
pub struct Intro {
    played: bool,
    requested: Option<bool>,
    playing: bool,
    finished: bool,
    #[cfg(not(target_arch = "wasm32"))]
    install: Option<std::path::PathBuf>,
    #[cfg(not(target_arch = "wasm32"))]
    decoder: Option<native::Decoder>,
}

impl Intro {
    pub fn request(&mut self, expansion: bool) -> bool {
        self.request_with_install(expansion, None)
    }
    pub fn request_with_install(
        &mut self,
        expansion: bool,
        install: Option<std::path::PathBuf>,
    ) -> bool {
        if self.played {
            return false;
        }
        self.played = true;
        self.requested = Some(expansion);
        self.playing = true;
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.install = install;
        }
        #[cfg(target_arch = "wasm32")]
        let _ = install;
        true
    }
    pub fn playing(&self) -> bool {
        self.playing
    }
    pub fn take_finished(&mut self) -> bool {
        std::mem::take(&mut self.finished)
    }
    fn finish(&mut self) {
        self.playing = false;
        self.requested = None;
        self.finished = true;
    }
}

pub fn install(app: &mut App) {
    crate::button_actions::install(app);
    app.init_resource::<Intro>();
    #[cfg(target_arch = "wasm32")]
    app.add_systems(Update, browser_update.in_set(IntroUpdate));
    #[cfg(not(target_arch = "wasm32"))]
    native::install(app);
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
extern "C" {
    #[wasm_bindgen(catch, js_namespace = peacecraft3Intro, js_name = play)]
    fn browser_play(expansion: bool) -> Result<bool, wasm_bindgen::JsValue>;
    #[wasm_bindgen(catch, js_namespace = peacecraft3Intro, js_name = take_finished)]
    fn browser_finished() -> Result<bool, wasm_bindgen::JsValue>;
}

#[cfg(target_arch = "wasm32")]
fn browser_update(mut intro: ResMut<Intro>) {
    if let Some(expansion) = intro.requested.take() {
        if !matches!(browser_play(expansion), Ok(true)) {
            intro.finish();
        }
    }
    if intro.playing && !matches!(browser_finished(), Ok(false)) {
        intro.finish();
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use super::*;
    use bevy::{
        asset::RenderAssetUsages,
        render::render_resource::{Extent3d, TextureDimension, TextureFormat},
        ui_widgets::Button,
    };
    use std::{
        io::Read,
        path::PathBuf,
        process::{Child, Command, Stdio},
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
            mpsc::{self, Receiver},
        },
        thread::JoinHandle,
    };

    const WIDTH: u32 = 800;
    const HEIGHT: u32 = 368;
    const FRAME_BYTES: usize = WIDTH as usize * HEIGHT as usize * 4;
    const MAX_AUDIO_BYTES: u64 = 32 * 1024 * 1024;

    pub(super) fn install(app: &mut App) {
        app.add_systems(Update, update.in_set(IntroUpdate));
    }

    pub struct Decoder {
        frames: Mutex<Receiver<Message>>,
        child: Arc<Mutex<Option<Child>>>,
        stopped: Arc<AtomicBool>,
        result: Arc<Mutex<Option<Result<(), String>>>>,
        worker: Option<JoinHandle<()>>,
        image: Handle<Image>,
        audio: Option<Vec<u8>>,
        video_ended: bool,
    }
    enum Message {
        Audio(Vec<u8>),
        Frame(Vec<u8>),
    }
    #[derive(Component)]
    struct Movie;
    #[derive(Component)]
    struct Skip;
    #[derive(Component)]
    struct MovieSound;

    fn source(expansion: bool, install: Option<&std::path::Path>) -> Option<PathBuf> {
        let install = install
            .map(std::path::Path::to_path_buf)
            .unwrap_or_else(|| pcx_gfx::viewer::resolve_install(None));
        let original = install.join("Movies").join(if expansion {
            "IntroX.mpq"
        } else {
            "TutorialOp.mpq"
        });
        if original.is_file() {
            return Some(original);
        }
        let name = if expansion {
            "intro-tft.mp4"
        } else {
            "intro-roc.mp4"
        };
        let adjacent = std::env::current_exe()
            .ok()?
            .parent()?
            .join("assets")
            .join(name);
        [adjacent, PathBuf::from("web/assets").join(name)]
            .into_iter()
            .find(|path| path.is_file())
    }

    fn chunks(mut bytes: &[u8]) -> impl Iterator<Item = (&[u8], &[u8])> {
        std::iter::from_fn(move || {
            let id = bytes.get(..4)?;
            let len = u32::from_le_bytes(bytes.get(4..8)?.try_into().ok()?) as usize;
            let end = 8usize.checked_add(len)?;
            let data = bytes.get(8..end)?;
            bytes = bytes.get(end.checked_add(len & 1)?..).unwrap_or_default();
            Some((id, data))
        })
    }

    fn avi_has_mp3_audio(bytes: &[u8]) -> bool {
        if bytes.get(..4) != Some(b"RIFF") || bytes.get(8..12) != Some(b"AVI ") {
            return false;
        }
        let mut audio_streams = 0;
        for (_, header) in chunks(&bytes[12..])
            .filter(|(id, data)| *id == b"LIST" && data.get(..4) == Some(b"hdrl"))
        {
            for (_, stream) in chunks(&header[4..])
                .filter(|(id, data)| *id == b"LIST" && data.get(..4) == Some(b"strl"))
            {
                let mut kind = None;
                let mut format = None;
                for (id, data) in chunks(&stream[4..]) {
                    if id == b"strh" {
                        kind = data.get(..4);
                    }
                    if id == b"strf" {
                        format = data.get(..2);
                    }
                }
                if kind == Some(b"auds") {
                    audio_streams += 1;
                    if format != Some(&[0x55, 0]) {
                        return false;
                    }
                }
            }
        }
        audio_streams > 0
    }

    fn copy_original_mp3(path: &std::path::Path) -> bool {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        if !name.eq_ignore_ascii_case("IntroX.mpq") && !name.eq_ignore_ascii_case("TutorialOp.mpq")
        {
            return false;
        }
        let Ok(file) = std::fs::File::open(path) else {
            return false;
        };
        let mut header = vec![];
        if file.take(64 * 1024).read_to_end(&mut header).is_err() {
            return false;
        }
        avi_has_mp3_audio(&header)
    }

    impl Decoder {
        fn start(path: PathBuf, image: Handle<Image>) -> Self {
            let (sender, receiver) = mpsc::sync_channel(2);
            let child = Arc::new(Mutex::new(None::<Child>));
            let stopped = Arc::new(AtomicBool::new(false));
            let result = Arc::new(Mutex::new(None));
            let shared_child = child.clone();
            let stop = stopped.clone();
            let completion = result.clone();
            let worker = std::thread::spawn(move || {
                let run = || -> Result<(), String> {
                    let copy_audio = copy_original_mp3(&path);
                    let spawn = |arguments: &[&str]| -> Result<std::process::ChildStdout, String> {
                        let mut slot = shared_child.lock().unwrap();
                        if stop.load(Ordering::Acquire) {
                            return Err("skipped".into());
                        }
                        let mut process = Command::new("ffmpeg")
                            .args(arguments)
                            .arg(&path)
                            .args(if arguments.contains(&"-re") {
                                vec![
                                    "-an",
                                    "-vf",
                                    "scale=800:368",
                                    "-fps_mode",
                                    "passthrough",
                                    "-pix_fmt",
                                    "rgba",
                                    "-f",
                                    "rawvideo",
                                    "pipe:1",
                                ]
                            } else if copy_audio {
                                vec![
                                    "-vn", "-map", "0:a:0", "-c:a", "copy", "-f", "mp3", "pipe:1",
                                ]
                            } else {
                                vec!["-vn", "-f", "mp3", "pipe:1"]
                            })
                            .stdin(Stdio::null())
                            .stdout(Stdio::piped())
                            .stderr(Stdio::null())
                            .spawn()
                            .map_err(|e| format!("FFmpeg unavailable: {e}"))?;
                        let stdout = process.stdout.take().ok_or("FFmpeg stdout unavailable")?;
                        *slot = Some(process);
                        Ok(stdout)
                    };
                    let reap = || -> Result<(), String> {
                        let mut slot = shared_child.lock().unwrap();
                        if let Some(mut process) = slot.take() {
                            let status = process.wait().map_err(|e| e.to_string())?;
                            if !status.success() && !stop.load(Ordering::Acquire) {
                                return Err(format!("FFmpeg exited {status}"));
                            }
                        }
                        Ok(())
                    };
                    let mut audio = vec![];
                    spawn(&["-v", "error", "-i"])?
                        .take(MAX_AUDIO_BYTES + 1)
                        .read_to_end(&mut audio)
                        .map_err(|e| e.to_string())?;
                    if audio.len() as u64 > MAX_AUDIO_BYTES {
                        return Err("Movie audio exceeds 32 MiB".into());
                    }
                    reap()?;
                    if stop.load(Ordering::Acquire) {
                        return Ok(());
                    }
                    let _ = sender.try_send(Message::Audio(audio));
                    let mut video = spawn(&["-v", "error", "-re", "-i"])?;
                    loop {
                        let mut frame = vec![0; FRAME_BYTES];
                        match video.read_exact(&mut frame) {
                            Ok(()) => {
                                let _ = sender.try_send(Message::Frame(frame));
                            }
                            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                            Err(e) => return Err(e.to_string()),
                        }
                        if stop.load(Ordering::Acquire) {
                            break;
                        }
                    }
                    reap()
                };
                *completion.lock().unwrap() = Some(run());
            });
            Self {
                frames: Mutex::new(receiver),
                child,
                stopped,
                result,
                worker: Some(worker),
                image,
                audio: None,
                video_ended: false,
            }
        }
        fn stop(&mut self) {
            self.stopped.store(true, Ordering::Release);
            if let Some(mut child) = self.child.lock().unwrap().take() {
                let _ = child.kill();
                let _ = child.wait();
            }
            if let Some(worker) = self.worker.take() {
                let _ = worker.join();
            }
        }
    }
    impl Drop for Decoder {
        fn drop(&mut self) {
            self.stop();
        }
    }

    fn update(
        mut commands: Commands,
        mut intro: ResMut<Intro>,
        keys: Res<ButtonInput<KeyCode>>,
        actions: Option<Res<crate::button_actions::ButtonActions>>,
        skip_buttons: Query<(), With<Skip>>,
        movie: Query<Entity, With<Movie>>,
        sound: Query<(Entity, Option<&AudioSink>), With<MovieSound>>,
        mut images: ResMut<Assets<Image>>,
        mut audio: ResMut<Assets<AudioSource>>,
        windows: Query<&Window>,
        mut movie_image: Query<&mut Node, With<MovieImage>>,
    ) {
        let skip = intro.playing
            && (keys.just_pressed(KeyCode::Escape)
                || actions.as_ref().is_some_and(|actions| {
                    actions
                        .entities
                        .iter()
                        .any(|entity| skip_buttons.contains(*entity))
                }));
        if skip {
            intro.requested = None;
        }
        if let Some(expansion) = intro.requested.take() {
            if let Some(path) = source(expansion, intro.install.as_deref()) {
                let image = images.add(Image::new_fill(
                    Extent3d {
                        width: WIDTH,
                        height: HEIGHT,
                        depth_or_array_layers: 1,
                    },
                    TextureDimension::D2,
                    &[0, 0, 0, 255],
                    TextureFormat::Rgba8UnormSrgb,
                    RenderAssetUsages::default(),
                ));
                intro.decoder = Some(Decoder::start(path, image.clone()));
                commands
                    .spawn((
                        Movie,
                        Node {
                            width: percent(100),
                            height: percent(100),
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            row_gap: Val::Rem(0.75),
                            ..default()
                        },
                        BackgroundColor(Color::BLACK),
                        GlobalZIndex(100),
                    ))
                    .with_children(|parent| {
                        parent.spawn((
                            MovieImage,
                            bevy::ui::widget::ImageNode::new(image),
                            Node {
                                width: Val::Vw(95.0),
                                height: Val::Vh(75.0),
                                flex_shrink: 0.0,
                                ..default()
                            },
                        ));
                        parent
                            .spawn((
                                Skip,
                                Button,
                                Node {
                                    padding: UiRect::axes(Val::Rem(1.0), Val::Rem(0.5)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.1, 0.1, 0.1)),
                            ))
                            .with_child((
                                Text::new("Skip cinematic (Esc)"),
                                TextColor(Color::srgb(1.0, 0.85, 0.1)),
                            ));
                    });
            } else {
                warn!("Original opening cinematic unavailable; continuing to menu");
                intro.finish();
            }
        }
        // The movie fits inside the window; its aspect is never stretched or cropped.
        if let Ok(window) = windows.single() {
            let h =
                (window.height() * 0.75).min(window.width() * 0.95 * HEIGHT as f32 / WIDTH as f32);
            let width = Val::Vw(h * WIDTH as f32 / HEIGHT as f32 / window.width().max(1.0) * 100.0);
            let height = Val::Vh(h / window.height().max(1.0) * 100.0);
            for mut node in &mut movie_image {
                if node.width != width || node.height != height {
                    node.width = width;
                    node.height = height;
                }
            }
        }
        let mut ended = false;
        if let Some(decoder) = &mut intro.decoder {
            let mut failed = false;
            if let Some(result) = decoder.result.lock().unwrap().take() {
                if let Err(error) = result {
                    warn!("Original movie playback failed: {error}; continuing to menu");
                    failed = true;
                } else {
                    decoder.video_ended = true;
                }
            }
            let mut started_audio = false;
            if !skip && !failed {
                for message in decoder.frames.lock().unwrap().try_iter() {
                    match message {
                        Message::Frame(bytes) => {
                            if let Some(bytes) =
                                decoder.audio.take().filter(|bytes| !bytes.is_empty())
                            {
                                commands.spawn((
                                    MovieSound,
                                    AudioPlayer(audio.add(AudioSource {
                                        bytes: bytes.into(),
                                    })),
                                    PlaybackSettings::DESPAWN,
                                ));
                                started_audio = true;
                            }
                            if let Some(mut image) = images.get_mut(&decoder.image) {
                                image.data = Some(bytes);
                            }
                        }
                        Message::Audio(bytes) => {
                            decoder.audio = Some(bytes);
                        }
                    }
                }
            }
            // Keep the final frame until its soundtrack ends; skip/error remains immediate.
            ended = failed
                || (decoder.video_ended
                    && !started_audio
                    && sound
                        .iter()
                        .all(|(_, sink)| sink.is_none_or(AudioSinkPlayback::empty)));
        }
        if skip || ended {
            let image = intro.decoder.take().map(|decoder| decoder.image.clone()); // Drop kills and reaps the decoder.
            for entity in &movie {
                commands.entity(entity).despawn();
            }
            for (entity, _) in &sound {
                commands.entity(entity).despawn();
            }
            if let Some(image) = image {
                images.remove(image.id());
            }
            intro.finish();
        }
    }
    #[derive(Component)]
    struct MovieImage;

    #[cfg(test)]
    mod tests {
        use super::*;
        fn chunk(id: &[u8; 4], bytes: &[u8]) -> Vec<u8> {
            let mut out = id.to_vec();
            out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            out.extend_from_slice(bytes);
            if bytes.len() & 1 != 0 {
                out.push(0);
            }
            out
        }
        fn avi(audio_codec: u16) -> Vec<u8> {
            let mut stream = b"strl".to_vec();
            stream.extend(chunk(b"strh", b"auds"));
            stream.extend(chunk(b"strf", &audio_codec.to_le_bytes()));
            let mut header = b"hdrl".to_vec();
            header.extend(chunk(b"LIST", &stream));
            let mut body = b"AVI ".to_vec();
            body.extend(chunk(b"LIST", &header));
            chunk(b"RIFF", &body)
        }
        #[test]
        fn codec_copy_requires_a_known_movie_name_and_an_mp3_avi_stream() {
            assert!(avi_has_mp3_audio(&avi(0x55)));
            assert!(!avi_has_mp3_audio(&avi(1)));
            assert!(!avi_has_mp3_audio(&avi(0x55)[..20]));
            assert!(!avi_has_mp3_audio(b"RIFF\0\0\0\0WAVE"));
            let dir =
                std::env::temp_dir().join(format!("peacecraft-intro-codec-{}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            let known = dir.join("IntroX.mpq");
            let staged = dir.join("intro-tft.mp4");
            std::fs::write(&known, avi(0x55)).unwrap();
            std::fs::write(&staged, avi(0x55)).unwrap();
            assert!(copy_original_mp3(&known));
            assert!(!copy_original_mp3(&staged));
            std::fs::write(&known, avi(1)).unwrap();
            assert!(!copy_original_mp3(&known));
            std::fs::remove_dir_all(dir).unwrap();
        }
        #[test]
        fn selected_install_movie_takes_precedence_over_fallbacks() {
            let dir = std::env::temp_dir().join(format!("peacecraft-intro-{}", std::process::id()));
            std::fs::create_dir_all(dir.join("Movies")).unwrap();
            let movie = dir.join("Movies/IntroX.mpq");
            std::fs::write(&movie, b"source-selection-test").unwrap();
            assert_eq!(source(true, Some(&dir)), Some(movie));
            std::fs::remove_dir_all(dir).unwrap();
        }
        #[test]
        #[ignore = "requires FFmpeg and a local original or staged intro movie"]
        fn original_movie_decodes_and_skip_reaps_the_process() {
            let path =
                source(true, None).expect("supply PEACECRAFT3_INSTALL or web/assets/intro-tft.mp4");
            let mut decoder = Decoder::start(path, Handle::default());
            let started = std::time::Instant::now();
            let mut saw_frame = false;
            while started.elapsed() < std::time::Duration::from_secs(10) {
                match decoder
                    .frames
                    .lock()
                    .unwrap()
                    .recv_timeout(std::time::Duration::from_millis(100))
                {
                    Ok(Message::Frame(bytes)) => {
                        assert_eq!(bytes.len(), FRAME_BYTES);
                        saw_frame = true;
                        break;
                    }
                    Ok(Message::Audio(bytes)) => assert!(!bytes.is_empty()),
                    Err(_) => {}
                }
                if let Some(result) = &*decoder.result.lock().unwrap() {
                    panic!("decoder ended before a frame: {result:?}");
                }
            }
            assert!(saw_frame, "original movie never produced a decoded frame");
            let stopped = std::time::Instant::now();
            decoder.stop();
            assert!(
                decoder.child.lock().unwrap().is_none(),
                "decoder child was not reaped"
            );
            assert!(decoder.worker.is_none(), "decoder thread was not joined");
            assert!(
                stopped.elapsed() < std::time::Duration::from_secs(2),
                "skip should promptly cancel decoding"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn completion_is_consumed_once_and_the_movie_is_not_replayed() {
        let mut intro = Intro::default();
        assert!(intro.request(true));
        assert!(intro.playing());
        assert!(!intro.request(false));
        intro.finish();
        assert!(!intro.playing());
        assert!(intro.take_finished());
        assert!(!intro.take_finished());
        assert!(!intro.request(true));
    }
}
