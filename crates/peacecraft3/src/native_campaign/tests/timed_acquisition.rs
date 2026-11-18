//! Local source-profile observations; collection does not qualify native timing or admission.
use super::*;
use pcx_play::{campaign_host, campaign_inputs, map_admission};
use pcx_script::Value;
use peacecraft3::{campaign_runtime::CampaignRuntime, map_binding};
use serde::Deserialize;
use serde_json::{Value as Json, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    time::Duration,
};

const CAPTURE_BYTES: usize = 1 << 20;
const PREFIX: &str = "PCX_REACH_ATTACK_V1";

#[derive(Deserialize)]
struct Manifest {
    schema: String,
    version: u32,
    wire_prefix: String,
    client_sha256: String,
    source_stats: Json,
    arms: Vec<Arm>,
    frozen_inputs: BTreeMap<String, String>,
    canonical_inputs: BTreeMap<String, String>,
    diagnostic_seed: i32,
}
#[derive(Deserialize)]
struct Arm {
    arm: String,
    timed: bool,
    original_map: String,
    original_map_sha256: String,
    original_script: String,
    original_script_sha256: String,
    original_nonce: String,
    map: String,
    map_sha256: String,
    script: String,
    script_sha256: String,
    nonce: String,
    output_name: String,
    native_capture: String,
    native_capture_sha256: String,
    native_observations: String,
    native_observations_sha256: String,
    actor: [f32; 2],
    enemy: [f32; 2],
}

fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn file_digest(path: &str) -> String {
    let mut file = fs::File::open(path).unwrap();
    let mut hash = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        let n = file.read(&mut buffer).unwrap();
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    hash.finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn checked_file(path: &str, expected: &str, limit: usize) -> Vec<u8> {
    assert!(
        fs::metadata(path).unwrap().len() <= limit as u64,
        "{path}: input bound"
    );
    let mut bytes = Vec::new();
    fs::File::open(path)
        .unwrap()
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .unwrap();
    assert!(bytes.len() <= limit, "{path}: input grew beyond bound");
    assert_eq!(digest(&bytes), expected, "{path}: frozen input changed");
    bytes
}
fn manifest() -> Manifest {
    let path = std::env::var("PCX_TIMED_ACQUISITION_MANIFEST").unwrap();
    let expected = std::env::var("PCX_TIMED_ACQUISITION_MANIFEST_SHA256").unwrap();
    let bytes = checked_file(&path, &expected, CAPTURE_BYTES);
    let manifest: Manifest = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(manifest.schema, "pcx-reach-attacked-observations");
    assert_eq!(manifest.version, 1);
    assert_eq!(manifest.wire_prefix, PREFIX);
    assert_eq!(manifest.arms.len(), 2);
    assert_eq!(manifest.arms[0].arm, "reach-baseline");
    assert!(!manifest.arms[0].timed);
    assert_eq!(manifest.arms[1].arm, "reach-timed");
    assert!(manifest.arms[1].timed);
    for (path, hash) in &manifest.frozen_inputs {
        checked_file(path, hash, MAX_ASSET_BYTES);
    }
    manifest
}
fn record(name: &str, value: &Json) {
    let dir = PathBuf::from(std::env::var("PCX_TIMED_ACQUISITION_OUTPUT").unwrap());
    assert!(dir.is_dir());
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dir.join(name))
        .unwrap();
    serde_json::to_writer_pretty(&mut file, value).unwrap();
    writeln!(file).unwrap();
}

fn narrow_script(source: &str, old_nonce: &str, nonce: &str) -> String {
    assert_eq!(source.matches("MetaInRange").count(), 3);
    let declaration = "    integer MetaInRange = 0\n";
    let field = " + \" \" + I2S(MetaInRange)";
    let lines: Vec<_> = source.split_inclusive('\n').collect();
    let at = lines
        .iter()
        .position(|line| line.starts_with("    if IsUnitInRange(Actor, Enemy, "))
        .unwrap();
    assert_eq!(lines[at + 1], "        set MetaInRange = 1\n");
    assert_eq!(lines[at + 2], "    endif\n");
    let block = lines[at..at + 3].concat();
    for node in [declaration, field, &block] {
        assert_eq!(source.matches(node).count(), 1);
    }
    source
        .replace(declaration, "")
        .replace(field, "")
        .replace(&block, "")
        .replace(old_nonce, nonce)
        .replace("PCX_TURN2", PREFIX)
        .replace("PCX-TURN-", "PCX-REACH-ATTACK-V1-")
}

struct Source {
    recipe: Arc<CampaignRecipe>,
    bindings: MapBindings,
    selection: pcx_play::campaign_data_set::CampaignDataSet,
}
impl Source {
    fn load(manifest: &Manifest) -> Self {
        let (install, map, expansion) = selected_chapter_env();
        assert!(!expansion);
        assert!(
            map.ends_with("Undead02.w3m"),
            "select the original catalog donor"
        );
        for (path, hash) in &manifest.canonical_inputs {
            assert_eq!(Path::new(path).parent(), Some(Path::new(&install)));
            assert_eq!(
                &file_digest(path),
                hash,
                "native reference source changed: {path}"
            );
        }
        let mode = crate::launch_gui::Launcher::test_source(install.clone(), false).mode();
        let recipe = campaign_recipe(Path::new(&install), mode).unwrap();
        assert_eq!(digest(recipe.image()), manifest.client_sha256);
        let config = configuration(
            Arc::clone(&recipe),
            mode,
            &Chapter {
                campaign: "timed acquisition diagnostic".into(),
                header: String::new(),
                name: map.clone(),
                map,
            },
            1,
        )
        .unwrap();
        fn member(m: &Member) -> map_binding::IdentifiedMember<'_> {
            map_binding::IdentifiedMember {
                path: &m.path,
                label: &m.label,
                identity: &m.identity,
                source: &m.source,
            }
        }
        let bindings = map_binding::bind_map_bounded(
            Arc::new(config.resolver),
            map_binding::BindingInputs {
                map: member(&config.map),
                common: member(&config.common),
                blizzard: member(&config.blizzard),
                catalog_identity: &config.catalog_identity,
                catalog_label: &config.catalog_label,
                catalog_sources: &config.catalog_sources,
                required_catalog_tables: &config.required_catalog_tables,
                required_fields: &config.required_fields,
                revision: config.revision,
            },
            config.limits,
        )
        .unwrap();
        let selection = recipe
            .open_map(
                &bindings.catalog_map_bytes,
                &bindings.map_label,
                map_read_budget(),
            )
            .unwrap()
            .selection;
        Self {
            recipe,
            bindings,
            selection,
        }
    }

    fn start(
        &self,
        m: &Manifest,
        arm: &Arm,
        narrow: bool,
    ) -> (MapScriptSession<MapStartupHost>, Json, Result<(), String>) {
        let b = &self.bindings;
        let (path, hash) = if narrow {
            (&arm.map, &arm.map_sha256)
        } else {
            (&arm.original_map, &arm.original_map_sha256)
        };
        let bytes: Arc<[u8]> = checked_file(path, hash, MAX_ASSET_BYTES).into();
        let map = pcx_map::Map::parse(&bytes).unwrap();
        let donor = pcx_map::Map::parse(&b.catalog_map_bytes).unwrap();
        let mut overlays = Vec::new();
        for member in [
            "war3map.w3i",
            "war3map.w3u",
            "war3map.w3t",
            "war3map.w3a",
            "war3map.w3b",
            "war3map.w3d",
            "war3map.w3h",
            "war3map.w3q",
            "war3mapMisc.txt",
        ] {
            let actual = map
                .extract_checked(path, member, map_read_budget())
                .unwrap();
            let original = donor
                .extract_checked(&b.map_label, member, map_read_budget())
                .unwrap();
            assert_eq!(actual, original, "catalog donor differs: {member}");
            overlays.push(json!({"member":member,"sha256":actual.as_deref().map(digest)}));
        }
        let selected = self
            .recipe
            .open_map(&bytes, path, map_read_budget())
            .unwrap();
        assert_eq!(
            selected.selection, self.selection,
            "catalog data-set mismatch"
        );
        let original = checked_file(
            &arm.original_script,
            &arm.original_script_sha256,
            CAPTURE_BYTES,
        );
        let expected = if narrow {
            let derived = checked_file(&arm.script, &arm.script_sha256, CAPTURE_BYTES);
            assert_eq!(
                derived,
                narrow_script(
                    std::str::from_utf8(&original).unwrap(),
                    &arm.original_nonce,
                    &arm.nonce
                )
                .as_bytes()
            );
            derived
        } else {
            original
        };
        assert_eq!(
            map.extract_checked(path, "war3map.j", map_read_budget())
                .unwrap()
                .unwrap(),
            expected
        );
        let identity = AssetIdentity {
            edition: b.map_identity.edition.clone(),
            version: format!("private diagnostic sha256 {hash}"),
            source: path.clone(),
        };
        fn script(
            s: &peacecraft3::map_requests::BoundScript,
        ) -> map_admission::IdentifiedScript<'_> {
            map_admission::IdentifiedScript {
                identity: &s.identity,
                asset: pcx_play::scripts::ScriptAsset {
                    label: &s.label,
                    bytes: &s.bytes,
                },
            }
        }
        let context = Arc::new(
            map_admission::prepare_map_context(
                map_admission::MapAdmissionInputs {
                    map: &map,
                    map_label: path,
                    map_identity: &identity,
                    common: script(&b.common),
                    blizzard: script(&b.blizzard),
                    catalog_identity: &b.catalog_identity,
                    catalog: &b.catalog,
                    required_fields: &b.required_fields,
                    trigger_catalog: b.trigger_catalog.clone(),
                },
                b.budget,
            )
            .unwrap(),
        );
        assert_eq!(context.map_sha256.as_slice(), &Sha256::digest(&bytes)[..]);
        let sources = campaign_inputs::MapSources {
            map_label: path.clone(),
            map_bytes: Arc::clone(&bytes),
            catalog_identity: b.catalog_identity.clone(),
            catalog: Arc::clone(&b.catalog.0),
        };
        let reader = Arc::new(selected.resolver);
        let inputs = campaign_inputs::prepare_new_game(
            &context,
            &sources,
            &reader,
            self.recipe.image(),
            campaign_inputs::client_identity(
                self.recipe.image(),
                &self.recipe.rules().origin.source,
            ),
            m.diagnostic_seed,
            campaign_inputs::Launch {
                expansion: false,
                route: Some(LaunchRoute::SinglePlayerStart),
            },
        )
        .unwrap();
        let heading = inputs
            .post_config
            .new_game
            .as_ref()
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .heading_rules()
            .unwrap();
        let profile = json!({"period":heading.period,"window":heading.window,"tolerance":heading.tolerance,
            "half_turn":heading.half_turn,"full_turn":heading.full_turn,"source_clock":format!("{:?}",inputs.clock.clock),"hero_attack_speed_per_agility":inputs.post_config.new_game.as_ref().unwrap().combat.as_ref().unwrap().attack_speed_per_agility});
        let metadata = json!({"map_sha256":hash,"map_identity":format!("{:?}",context.sources.map),"catalog_identity":format!("{:?}",b.catalog_identity),
            "dataset":format!("{:?}",selected.selection),"overlay_byte_proof":overlays,"profile":profile,
            "client_sha256":digest(self.recipe.image()),"common_sha256":digest(&b.common.bytes),"blizzard_sha256":digest(&b.blizzard.bytes),
            "catalog_receipts":format!("{:?}",b.receipts.catalog),"canonical_inputs":m.canonical_inputs,"seed":m.diagnostic_seed,"native_rng_phase_matched":false});
        let post = inputs.post_config.clone();
        let config = campaign_host::entry_id(&context, "config").unwrap();
        let main = campaign_host::entry_id(&context, "main").unwrap();
        let admitted = Arc::clone(&context);
        let mut session = MapScriptSession::default();
        let result = session
            .replace_sequenced(
                Arc::clone(&context),
                move |ctx| {
                    let mut host = campaign_host::create_host(
                        ctx,
                        inputs,
                        Some(reader),
                        bytes,
                        true,
                        pcx_play::campaign_cache::local_storage(),
                    )?;
                    assert!(host.objects_mut().select_native_counter_platform()?);
                    Ok::<_, pcx_play::map_startup_host::MapStartupError>(host)
                },
                &[config, main],
                move |vm, completed| {
                    campaign_host::apply_after_config(vm, &admitted, completed, &post)
                },
            )
            .map_err(|e| e.to_string());
        let result =
            result.and_then(|()| session.sample_admitted_vision().map_err(|e| e.to_string()));
        (session, metadata, result)
    }
}

#[derive(Debug, serde::Serialize)]
struct Capture {
    metadata: [i32; 11],
    placement: [i32; 13],
    samples: Vec<[i32; 10]>,
    events: Vec<[i32; 6]>,
}
fn real(word: i32) -> Result<f32, String> {
    let value = f32::from_bits(word as u32);
    if value.is_finite() {
        Ok(value)
    } else {
        Err("nonfinite observation".into())
    }
}
fn capture(raw: &str, nonce: &str, arm: &str) -> Result<Capture, String> {
    let fail = |s: &str| Err(s.to_owned());
    if raw.len() > CAPTURE_BYTES {
        return fail("capture byte bound");
    }
    let marker = format!("{PREFIX} ");
    let mut rows = Vec::new();
    for line in raw.lines() {
        if let Some((_, tail)) = line.split_once(&marker) {
            let tokens: Vec<_> = tail.split('"').next().unwrap().split_whitespace().collect();
            if tokens.len() < 3 || tokens[..2] != [nonce, arm] {
                return fail("capture identity");
            }
            rows.push(tokens[2..].to_vec());
        }
    }
    if raw.matches(&marker).count() != rows.len() || rows.len() < 4 {
        return fail("capture rows");
    }
    if rows.iter().any(|r| r.first() == Some(&"invalid")) {
        return fail("producer marked invalid");
    }
    if rows[0] != ["begin", "257"] {
        return fail("begin");
    }
    let ints = |tokens: &[&str]| -> Result<Vec<i32>, String> {
        tokens
            .iter()
            .map(|s| {
                let n = s.parse::<i32>().map_err(|_| "integer".to_owned())?;
                if n.to_string() != *s {
                    return Err("noncanonical integer".into());
                }
                Ok(n)
            })
            .collect()
    };
    let last = rows.last().unwrap();
    if last.len() != 3 || last[..2] != ["end", "257"] {
        return fail("footer");
    }
    let count = ints(&last[2..])?[0];
    if !(0..=128).contains(&count) || rows.len() != 4 + 257 + count as usize {
        return fail("row/event count");
    }
    let fixed = |index: usize, tag: &str, len: usize| -> Result<Vec<i32>, String> {
        let row = &rows[index];
        if row.len() != len + 1 || row[0] != tag {
            return Err(format!("{tag} width"));
        }
        ints(&row[1..])
    };
    let metadata: [i32; 11] = fixed(1, "metadata", 11)?.try_into().unwrap();
    let placement: [i32; 13] = fixed(2, "placement", 13)?.try_into().unwrap();
    let mut samples: Vec<[i32; 10]> = Vec::new();
    for i in 0..257 {
        let row = fixed(3 + i, "sample", 11)?;
        if row[0] != i as i32 {
            return fail("sample index");
        }
        let s: [i32; 10] = row[1..].try_into().unwrap();
        for at in [0, 1, 2, 3, 5, 6] {
            real(s[at])?;
        }
        if real(s[5])? <= 0.0
            || s[7] != i32::from(i < 96)
            || (i < 96 && real(s[6])? <= 0.0)
            || ![-1, 0, 1].contains(&s[8])
            || !(0..=128).contains(&s[9])
        {
            return fail("sample state");
        }
        if samples
            .last()
            .is_some_and(|previous| real(previous[0]).unwrap() > real(s[0]).unwrap())
        {
            return fail("sample clock reversed");
        }
        samples.push(s);
    }
    if real(samples[256][0])? <= real(samples[0][0])? {
        return fail("clock did not advance");
    }
    let mut events: Vec<[i32; 6]> = Vec::new();
    for i in 0..count as usize {
        let row = fixed(260 + i, "event", 7)?;
        if row[0] != i as i32 {
            return fail("event index");
        }
        let e: [i32; 6] = row[1..].try_into().unwrap();
        real(e[1])?;
        if ![1, 2, 3, 4, 10, 11, 12, 13].contains(&e[0])
            || !(-1..=2).contains(&e[3])
            || !(0..=256).contains(&e[2])
        {
            return fail("event state");
        }
        if events
            .last()
            .is_some_and(|previous| real(previous[1]).unwrap() > real(e[1]).unwrap())
        {
            return fail("event clock reversed");
        }
        events.push(e);
    }
    for (kind, cursor) in [(10, 32), (11, 96), (12, 128), (13, 160)] {
        let found: Vec<_> = events.iter().filter(|e| e[0] == kind).collect();
        if found.len() != 1 || found[0][2] != cursor {
            return fail("stimulus checkpoints");
        }
    }
    Ok(Capture {
        metadata,
        placement,
        samples,
        events,
    })
}

fn validate_geometry(c: &Capture, m: &Manifest, arm: &Arm) {
    let stat = |table: &str, unit: &str, field: &str| -> f32 {
        m.source_stats[table][unit][field]
            .as_str()
            .unwrap()
            .parse()
            .unwrap()
    };
    let word = |v: f32| v.to_bits() as i32;
    let expected = [
        i32::from_be_bytes(*b"Uear"),
        i32::from_be_bytes(*b"hfoo"),
        word(stat("UnitWeapons.slk", "Uear", "acquire")),
        word(stat("UnitWeapons.slk", "Uear", "acquire")),
        word(stat("UnitBalance.slk", "Uear", "collision")),
        word(stat("UnitBalance.slk", "hfoo", "collision")),
        word(arm.actor[0]),
        word(arm.actor[1]),
        word(arm.enemy[0]),
        word(arm.enemy[1]),
    ];
    assert_eq!(c.metadata[..10], expected);
    assert!([0, 1].contains(&c.metadata[10]));
    assert_eq!(
        c.placement[..4],
        [expected[6], expected[7], expected[8], expected[9]]
    );
    assert_eq!(c.placement[4..8], [0; 4]);
    assert_eq!(c.placement[8], word(arm.enemy[0] - arm.actor[0]));
    assert_eq!(c.placement[9], word(0.0));
    let low = real(expected[4]).unwrap() + real(expected[5]).unwrap();
    let high = stat("UnitWeapons.slk", "Uear", "rangeN1");
    for at in [8, 10] {
        let value = real(c.placement[at]).unwrap();
        assert!(low < value && value < high, "conservative reach class");
    }
    let enemy_acquire = word(stat("UnitWeapons.slk", "hfoo", "acquire"));
    assert_eq!(c.placement[11..], [enemy_acquire; 2]);
    // Actual sampled positions, rather than an assumption about the constructor or target owner.
    for s in &c.samples[..96] {
        assert_eq!(
            &s[1..3],
            &expected[6..8],
            "actor moved before target removal"
        );
    }
}
fn attacked(c: &Capture, baseline: bool) -> Option<f32> {
    let events: Vec<_> = c.events.iter().filter(|e| e[0] == 3).collect();
    let eligible = |e: &&[i32; 6]| e[3] == 2 && (32..96).contains(&e[2]);
    if baseline {
        assert!(
            !events.is_empty() && events.iter().all(eligible),
            "baseline actual role2 attacked exposure absent or foreign"
        );
    }
    let time = events
        .into_iter()
        .find(eligible)
        .map(|e| real(e[1]).unwrap());
    if let Some(time) = time {
        let visible = c.events.iter().find(|e| e[0] == 10).unwrap();
        let removed = c.events.iter().find(|e| e[0] == 11).unwrap();
        assert!(time >= real(visible[1]).unwrap() && time < real(removed[1]).unwrap());
    }
    time
}

fn native_reference(arm: &Arm) -> Capture {
    checked_file(
        &arm.native_capture,
        &arm.native_capture_sha256,
        CAPTURE_BYTES,
    );
    let raw = checked_file(
        &arm.native_observations,
        &arm.native_observations_sha256,
        CAPTURE_BYTES,
    );
    let v: Json = serde_json::from_slice(&raw).unwrap();
    assert_eq!(v["arm"], arm.arm);
    assert_eq!(v["nonce"], arm.original_nonce);
    // A named view of the original strict parser's frozen result; its schema and raw capture stay intact.
    let ints = |value: &Json| -> Vec<i32> {
        value
            .as_array()
            .unwrap()
            .iter()
            .map(|v| i32::try_from(v.as_i64().unwrap()).unwrap())
            .collect()
    };
    let metadata = ints(&v["metadata"]).try_into().unwrap();
    let mut placement = ints(&v["initial_placement_observations"]);
    assert_eq!(placement.len(), 14);
    placement.remove(11); // The separate view omits only the explicitly unqueried native field.
    let project = |array: &Json, names: &[&str]| -> Vec<Vec<i32>> {
        array
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                names
                    .iter()
                    .map(|name| i32::try_from(row[name].as_i64().unwrap()).unwrap())
                    .collect()
            })
            .collect()
    };
    let samples = project(
        &v["samples"],
        &[
            "time",
            "x",
            "y",
            "facing",
            "current_order",
            "actor_life",
            "enemy_life",
            "enemy_exists",
            "last_acquired_role",
            "acquisition_count",
        ],
    )
    .into_iter()
    .map(|row| row.try_into().unwrap())
    .collect::<Vec<_>>();
    let events = project(
        &v["events"],
        &[
            "kind",
            "time",
            "sample_cursor",
            "target_role",
            "current_order",
            "issued_order",
        ],
    )
    .into_iter()
    .map(|row| row.try_into().unwrap())
    .collect::<Vec<_>>();
    assert_eq!(samples.len(), 257);
    assert!(events.len() <= 128);
    Capture {
        metadata,
        placement: placement.try_into().unwrap(),
        samples,
        events,
    }
}

fn initial_profile(session: &mut MapScriptSession<MapStartupHost>) -> Json {
    let Some(Value::Handle(actor)) = session.global("Actor") else {
        panic!("Actor missing");
    };
    let objects = session.host().unwrap().objects();
    let u = objects.executed_unit(actor).unwrap();
    assert!(u.shown && u.paused_since.is_none() && u.acquire_range.is_none());
    assert!(u.morph.is_none() && u.construction.is_none());
    let activity = u.activity.as_ref().unwrap();
    assert!(activity.movable() && !activity.moving() && !activity.held());
    assert_eq!(objects.current_unit_order(actor).unwrap(), 0);
    let combat = u.combat.as_ref().unwrap();
    assert!(
        !combat.dead
            && combat.target.is_none()
            && combat.attack_destination.is_none()
            && combat.ground_target.is_none()
    );
    let weapon = combat.weapon.unwrap();
    assert!(weapon.damage_point.is_some());
    let stats = objects.unit_stat_snapshot(actor).unwrap();
    assert!(stats.attributes.is_some(), "source hero attributes missing");
    let clock = objects.test_native_clock_state().unwrap();
    json!({"weapon":format!("{weapon:?}"),"combat_stat_snapshot":format!("{stats:?}"),
        "clock_rules":format!("{:?}",clock.rules()),"rawcode":u.definition.rawcode,
        "requested_position":[u.x,u.y],"actual_position":u.position,"owner":u.executed_owner,
        "collision":u.collision_radius.unwrap(),"face":u.face,"preconditions":"shown mobile idle unpaused unmodified source profile"})
}

fn run_narrow(source: &Source, m: &Manifest, arm: &Arm) -> (Capture, Json) {
    let (mut session, mut metadata, started) = source.start(m, arm, true);
    record(
        &format!("{}-startup.json", arm.arm),
        &json!({"source":metadata,"result":started}),
    );
    started.expect("normal source startup");
    let profile = initial_profile(&mut session);
    metadata["live_profile"] = profile;
    let input = Duration::from_millis(25); // Diagnostic transport cadence; game timing stays in unmodified Rules.
    let mut runtime = CampaignRuntime::new(input, 8, 65536);
    let initial = session
        .host()
        .unwrap()
        .objects()
        .native_counter_progress_ns()
        .unwrap();
    for frame in 0..12000_u32 {
        runtime.advance(&mut session, input);
        if let Some(error) = &runtime.last_error {
            record(
                &format!("{}-failure.json", arm.arm),
                &json!({"frame":frame,"error":error.to_string(),"clock":format!("{:?}",session.host().map(|h|h.objects().native_game_point())),"source":metadata}),
            );
            panic!("{} frame={frame}: {error}", arm.arm);
        }
        let host = session.host().unwrap();
        let objects = host.objects();
        assert!(objects.native_counter_platform());
        assert_eq!(
            objects.native_counter_progress_ns().unwrap() - initial
                + objects.native_counter_pending_ns().unwrap(),
            u128::from(frame) * input.as_nanos()
        );
        if let Some(raw) = host.preload_generated(&arm.output_name) {
            assert!(raw.len() <= CAPTURE_BYTES);
            record(
                &format!("{}-capture.json", arm.arm),
                &json!({"source":metadata,"frame":frame,"generated":raw,"sha256":digest(raw.as_bytes()),"pending_ns":objects.native_counter_pending_ns()}),
            );
            assert_eq!(
                objects.native_counter_pending_ns(),
                Some(0),
                "completed capture still has pending input"
            );
            let capture = capture(raw, &arm.nonce, &arm.arm).expect("complete narrow protocol");
            validate_geometry(&capture, m, arm);
            return (capture, metadata);
        }
    }
    panic!(
        "{}: no complete capture within bounded Counter inputs",
        arm.arm
    );
}

#[test]
#[ignore = "requires selected canonical Undead02 install and pinned private PCX_TIMED_ACQUISITION_MANIFEST/SHA256/OUTPUT; private display/audio/profile"]
fn unchanged_full_v9_reach_refuses_unimplemented_query() {
    let m = manifest();
    let source = Source::load(&m);
    for arm in &m.arms {
        let (_, metadata, result) = source.start(&m, arm, false);
        let error = result.expect_err("unchanged full V9 must retain its actual query frontier");
        record(
            &format!("{}-full-v9-refusal.json", arm.arm),
            &json!({"full_protocol_outcome":"RED","source":metadata,"error":error}),
        );
        assert!(
            error.contains("IsUnitInRange"),
            "unexpected earlier frontier: {error}"
        );
    }
}

#[test]
#[ignore = "requires selected canonical Undead02 install and pinned private PCX_TIMED_ACQUISITION_MANIFEST/SHA256/OUTPUT; private display/audio/profile"]
fn matched_source_reach_attacked_observations() {
    let m = manifest();
    let source = Source::load(&m);
    let mut own = Vec::new();
    let mut native = Vec::new();
    let mut profiles = Vec::new();
    for arm in &m.arms {
        let reference = native_reference(arm);
        validate_geometry(&reference, &m, arm);
        let (observations, profile) = run_narrow(&source, &m, arm);
        record(
            &format!("{}-observations.json", arm.arm),
            &json!({"own":observations,"native_reference_view":reference,"original_parser_unchanged":true}),
        );
        own.push(attacked(&observations, !arm.timed));
        native.push(attacked(&reference, !arm.timed));
        profiles.push(profile);
    }
    assert_eq!(profiles[0]["profile"], profiles[1]["profile"]);
    assert_eq!(profiles[0]["live_profile"], profiles[1]["live_profile"]);
    record(
        "paired-comparison.json",
        &json!({"outcome":"bounded_observations_collected","own_first_attacked_seconds":own,
        "native_first_attacked_seconds":native,"own_timed_minus_baseline":own[1].map(|t|t-own[0].unwrap()),
        "native_timed_minus_baseline":native[1].map(|t|t-native[0].unwrap()),
        "full_v9_outcome":"RED","native_callback_phase_or_rng_match":false,"production_admission_qualified":false,"unexpected_timed_callbacks":"All raw events retained; only role2 attacked events in declared exposure enter latency comparison; other responses remain unqualified observations"}),
    );
}

#[test]
fn narrow_capture_rejects_invalid_and_preserves_zero_acquisition() {
    let prefix = format!("{PREFIX} nonce reach-baseline ");
    let mut rows = vec![
        "begin 257".to_owned(),
        format!("metadata {}", vec!["0"; 11].join(" ")),
        format!("placement {}", vec!["0"; 13].join(" ")),
    ];
    for i in 0..257 {
        let values = [
            ((i as f32) / 64.0).to_bits() as i32,
            0,
            0,
            0,
            0,
            100.0_f32.to_bits() as i32,
            100.0_f32.to_bits() as i32,
            i32::from(i < 96),
            0,
            0,
        ];
        rows.push(format!(
            "sample {i} {}",
            values
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" ")
        ));
    }
    for (i, (kind, step)) in [(10, 32), (11, 96), (12, 128), (13, 160)]
        .into_iter()
        .enumerate()
    {
        rows.push(format!(
            "event {i} {kind} {} {step} 0 0 0",
            ((step as f32) / 64.0).to_bits() as i32
        ));
    }
    rows.push("end 257 4".into());
    let raw = rows
        .iter()
        .map(|row| format!("{prefix}{row}\n"))
        .collect::<String>();
    let parsed = capture(&raw, "nonce", "reach-baseline").unwrap();
    assert_eq!(parsed.events.len(), 4);
    assert!(parsed.samples.iter().all(|s| s[9] == 0));
    for invalid in [
        raw.replace(
            "begin 257",
            "begin 257\nPCX_REACH_ATTACK_V1 nonce reach-baseline invalid",
        ),
        raw.replace("sample 1 ", "sample 0 "),
        raw.replace("end 257 4", "end 257 3"),
        raw.replace("nonce reach-baseline", "foreign reach-baseline"),
    ] {
        assert!(capture(&invalid, "nonce", "reach-baseline").is_err());
    }
}
