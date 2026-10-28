use super::*;
use crate::{
    campaign_startup::CampaignHostSupply,
    map_requests::{MapOutcome, MapRequests, RequestFailure},
};
use pcx_assets::{Addon, AssetMode, Behaviour, Build, GameMode, OverrideDirectory};
use std::path::PathBuf;

const TABLE: &str = "[LoadingScreens]\nNumScreens=2\n\
    00=0,WESTRING_A,0,UI\\Loading\\A.mdl\n01=0,WESTRING_B,3,UI\\Loading\\B.mdl\n";

const RULES: ProgressRules = ProgressRules {
    fast_step: 0.5,
    slow_step: 0.125,
    full: 2.0,
};

const LOADED: Signals = Signals {
    admitted: true,
    prepared: true,
    published: true,
    ready: true,
    failed: false,
};

static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// A selected source holding the given files, removed when dropped.
struct Source(PathBuf);

impl Source {
    fn new(name: &str, files: &[(&str, &str)]) -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../.scratch/loading-fixtures")
            .join(format!(
                "{}-{}-{name}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        for (path, text) in files {
            let at = root.join(path.replace('\\', "/"));
            std::fs::create_dir_all(at.parent().unwrap()).unwrap();
            std::fs::write(at, text).unwrap();
        }
        Self(root)
    }

    fn resolver(&self) -> Arc<Resolver> {
        let mode = GameMode {
            build: Build::V1_29b,
            addon: Addon::Tft,
            assets: AssetMode::Vanilla,
            behaviour: Behaviour::Vanilla,
        };
        Arc::new(
            Resolver::new(
                mode,
                Some(OverrideDirectory::open(&self.0, "loading-test", None).unwrap()),
                Vec::new(),
            )
            .unwrap(),
        )
    }
}

impl Drop for Source {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A version 25 header whose loading fields are the arguments.
fn info(background: u32, custom: &str, text: &str, title: &str) -> MapInfo {
    let mut data = Vec::new();
    for word in [25u32, 11, 12] {
        data.extend(word.to_le_bytes());
    }
    for string in ["map", "author", "description", "recommended"] {
        data.extend(string.as_bytes());
        data.push(0);
    }
    for value in 0..8 {
        data.extend((value as f32).to_le_bytes());
    }
    for word in [1u32, 2, 3, 4, 32, 64, 7] {
        data.extend(word.to_le_bytes());
    }
    data.push(b'L');
    data.extend(background.to_le_bytes());
    for string in [custom, text, title, "sub"] {
        data.extend(string.as_bytes());
        data.push(0);
    }
    data.extend(0u32.to_le_bytes());
    data.extend([0u8; 4]);
    data.extend(2u32.to_le_bytes());
    for value in [1f32, 2., 3.] {
        data.extend(value.to_le_bytes());
    }
    data.extend(0u32.to_le_bytes());
    data.extend(0u32.to_le_bytes());
    data.extend(b"sound\0");
    data.push(b'A');
    data.extend(0u32.to_le_bytes());
    for _ in 0..5 {
        data.extend(0u32.to_le_bytes());
    }
    data.extend(1u32.to_le_bytes());
    data.extend(42u32.to_le_bytes());
    data.extend(b"items\0");
    for word in [2u32, 1, 33, 0x4142_4344, 0] {
        data.extend(word.to_le_bytes());
    }
    MapInfo::from_bytes(data).unwrap()
}

fn request() -> LoadingRequest {
    let source = Source::new("request", &[(TABLE_PATH, TABLE)]);
    LoadingRequest::read(
        source.resolver(),
        &info(1, "", "words", "Title"),
        None,
        RULES,
    )
    .unwrap()
    .unwrap()
}

fn begun() -> CampaignLoading {
    let mut loading = CampaignLoading::default();
    loading.begin(request(), &MapRequests::default(), None);
    loading
}

#[test]
fn a_request_follows_the_header_through_the_selected_table() {
    let source = Source::new("selects", &[(TABLE_PATH, TABLE)]);
    let request = LoadingRequest::read(
        source.resolver(),
        &info(1, "", "words", "Title"),
        None,
        RULES,
    )
    .unwrap()
    .unwrap();
    assert_eq!(request.screen.model, "UI\\Loading\\B.mdl");
    assert_eq!(request.screen.sequence, 3);
    assert_eq!(request.text.text, "words");
    assert_eq!(request.text.title, "Title");
    assert_eq!(request.text.subtitle, "sub");
    // No number and no custom path is the melee panel, which has no request.
    let melee = info(u32::MAX, "", "words", "Title");
    assert!(
        LoadingRequest::read(source.resolver(), &melee, None, RULES)
            .unwrap()
            .is_none()
    );
}

#[test]
fn a_missing_table_or_screen_refuses_with_its_name() {
    let empty = Source::new("empty", &[]);
    let error = LoadingRequest::read(empty.resolver(), &info(0, "", "", ""), None, RULES)
        .err()
        .unwrap();
    assert!(error.contains(TABLE_PATH), "{error}");
    let source = Source::new("range", &[(TABLE_PATH, TABLE)]);
    let error = LoadingRequest::read(source.resolver(), &info(2, "", "", ""), None, RULES)
        .err()
        .unwrap();
    assert!(error.contains("loading screen 2"), "{error}");
}

#[test]
fn stages_fill_the_bar_in_equal_shares_and_the_view_ends_the_load() {
    let mut loading = begun();
    assert_eq!(loading.phase(), Phase::Loading);
    assert!(loading.holds_simulation());
    assert_eq!(loading.fraction(), 0.0);
    loading.observe(Signals {
        admitted: true,
        ..Signals::default()
    });
    assert_eq!(loading.reached, 1);
    // A stage reached out of order waits for the ones before it.
    loading.observe(Signals {
        published: true,
        ..Signals::default()
    });
    assert_eq!(loading.reached, 1);
    loading.observe(LOADED);
    assert_eq!(loading.reached, STAGES);
    assert_eq!(loading.phase(), Phase::AwaitKey);
    assert_eq!(loading.fraction(), 1.0);
    assert!(loading.holds_simulation());
}

#[test]
fn only_a_loaded_screen_ends_on_a_press() {
    let mut loading = begun();
    assert!(!loading.press());
    assert_eq!(loading.phase(), Phase::Loading);
    loading.observe(LOADED);
    assert!(loading.press());
    assert_eq!(loading.phase(), Phase::Idle);
    assert!(!loading.holds_simulation());
    assert!(loading.request().is_none());
}

#[test]
fn a_failed_stage_drops_the_screen_without_starting() {
    let mut loading = begun();
    loading.observe(Signals {
        failed: true,
        ..Signals::default()
    });
    assert_eq!(loading.phase(), Phase::Idle);
}

#[test]
fn a_failure_already_on_display_at_the_launch_is_not_this_load() {
    let failure = |label: &str| MapOutcome {
        label: label.into(),
        identity: None,
        binding_revision: None,
        result: Err(RequestFailure::MissingBindings),
    };
    let mut requests = MapRequests {
        last_outcome: Some(failure("old")),
        ..MapRequests::default()
    };
    let mut loading = CampaignLoading::default();
    loading.begin(request(), &requests, None);
    assert!(!loading.fresh_outcome_failed(&requests));
    requests.last_outcome = Some(failure("new"));
    assert!(loading.fresh_outcome_failed(&requests));
}

#[test]
fn the_signals_wait_for_this_loads_revision_and_session() {
    let mut loading = begun();
    let requests = MapRequests::default();
    let supply = CampaignHostSupply::default();
    let startup = CampaignStartup::default();
    let ready = ViewState {
        ready: true,
        failed: false,
    };
    let none = signals(&loading, &requests, &supply, &startup, None, ready);
    assert_eq!(none, Signals::default());
    // A session that was already running at the launch is not this load's.
    let old = Arc::new(());
    loading.session = Some(Arc::downgrade(&old));
    let same = signals(
        &loading,
        &requests,
        &supply,
        &startup,
        Some(Arc::downgrade(&old)),
        ready,
    );
    assert!(!same.published && !same.ready);
}

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>();
    crate::map_requests::install(&mut app);
    crate::campaign_startup::install(&mut app);
    install(&mut app);
    app
}

#[test]
fn a_press_ends_the_loaded_screen_and_never_reaches_the_game() {
    let mut app = app();
    let mut loading = begun();
    loading.observe(LOADED);
    app.insert_resource(loading);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();
    assert_eq!(
        app.world().resource::<CampaignLoading>().phase(),
        Phase::Idle
    );
    assert!(
        !app.world()
            .resource::<ButtonInput<MouseButton>>()
            .just_pressed(MouseButton::Left),
        "the press that ends the screen is consumed"
    );
}

#[test]
fn a_press_while_the_map_is_still_loading_is_consumed_and_ends_nothing() {
    let mut app = app();
    app.insert_resource(begun());
    for key in [KeyCode::KeyA, KeyCode::Escape] {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        app.update();
        assert_eq!(
            app.world().resource::<CampaignLoading>().phase(),
            Phase::Loading
        );
        assert!(
            !app.world()
                .resource::<ButtonInput<KeyCode>>()
                .just_pressed(key)
        );
    }
}

#[test]
fn a_map_without_a_camera_setup_still_ends_the_load_on_its_terrain() {
    let context = Arc::new(());
    // Terrain published for the running session: ready, whatever the camera does.
    let ok = view_state(true, false, Some(&context), Some(&context));
    assert_eq!(
        ok,
        ViewState {
            ready: true,
            failed: false
        }
    );
    let mut loading = begun();
    loading.observe(Signals {
        admitted: true,
        prepared: true,
        published: true,
        ready: ok.ready,
        failed: ok.failed,
    });
    assert_eq!(loading.phase(), Phase::AwaitKey);
}

#[test]
fn a_refused_terrain_for_this_load_ends_the_screen_but_the_mounted_one_does_not() {
    let (mounted, current) = (Arc::new(()), Arc::new(()));
    // An error beside another context's ground: this load's terrain was refused.
    assert!(view_state(false, true, Some(&mounted), Some(&current)).failed);
    // An error left over from a replacement that this ground already supersedes.
    assert!(!view_state(true, true, Some(&current), Some(&current)).failed);
    assert!(!view_state(false, false, Some(&mounted), Some(&current)).failed);
    // No ground yet and an error is a refusal as well.
    assert!(view_state(false, true, None, Some(&current)).failed);
}
