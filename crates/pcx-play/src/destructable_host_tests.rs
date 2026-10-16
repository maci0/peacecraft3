use super::{COMMON, budget, context, object, slots_rule};
use crate::file_sounds::{Assets, Budget as SoundBudget, SoundInput};
use crate::playercolor_host::{
    DestructableBudget, DestructableCreation, EnumHost, Failure, Limits, ObjectHost,
};
use crate::scripts::{AssetIdentity, ScriptAsset};
use pcx_script::{Host, Prelude, Value};
use std::sync::Arc;

const DECLARATIONS: &str = "type agent extends handle
type widget extends agent
type destructable extends widget
type timer extends agent
type playercolor extends handle
constant native ConvertPlayerColor takes integer value returns playercolor
native CreateTimer takes nothing returns timer
native DestroyTimer takes timer which returns nothing
native CreateDestructable takes integer objectid, real x, real y, real face, real scale, integer variation returns destructable
globals
constant playercolor COLOR = ConvertPlayerColor(19)
endglobals
";

fn host_from(declarations: &str, script: &str, max_handles: usize) -> ObjectHost {
    let common = pcx_script::parse_script(declarations).unwrap();
    let map = pcx_script::parse_script(script).unwrap();
    let program = Arc::new(
        pcx_script::resolve_with(&map, &[Prelude::new("synthetic:destructables", &common)])
            .unwrap(),
    );
    let inner = EnumHost::checked_selected(
        program,
        "synthetic:selected-map",
        &["ConvertPlayerColor"],
        Limits {
            max_handles,
            admission_steps: 100,
            expression_depth: 16,
            expression_work: 100,
        },
    )
    .unwrap();
    ObjectHost::checked(inner).unwrap()
}

fn admitted(max_records: usize, max_handles: usize) -> ObjectHost {
    host_from(DECLARATIONS, "", max_handles)
        .with_destructables(DestructableBudget { max_records })
        .unwrap()
}

fn payload(objectid: i32, variation: i32) -> [Value; 6] {
    [
        Value::Int(objectid),
        Value::Real(1.5),
        Value::Real(-2.25),
        Value::Real(0.0),
        Value::Real(3.5),
        Value::Int(variation),
    ]
}

fn create(host: &mut ObjectHost, objectid: i32, variation: i32) -> Result<Value, String> {
    host.native("CreateDestructable", &payload(objectid, variation))
}

#[test]
fn executed_payload_and_declaration_source_stay_on_the_destructable_identity() {
    let mut host = admitted(2, 8);
    let (source, span) = host.destructable_declaration().unwrap();
    assert_eq!(source, "synthetic:destructables");
    let parsed = pcx_script::parse_script(DECLARATIONS).unwrap();
    let declared = parsed
        .natives()
        .find(|native| native.name.name == "CreateDestructable")
        .unwrap();
    assert_eq!(span, declared.span);
    assert!(DECLARATIONS[span.start as usize..span.end as usize].contains("objectid"));
    let before = host.inner().handles();
    let created = create(&mut host, -7, i32::MIN).unwrap();
    let Value::Handle(id) = created else {
        panic!("destructable handle");
    };
    assert_eq!(host.inner().kind(id).unwrap(), "destructable");
    assert_eq!(
        host.destructable_creation(id).unwrap(),
        DestructableCreation {
            objectid: -7,
            x: 1.5,
            y: -2.25,
            face: 0.0,
            scale: 3.5,
            variation: i32::MIN,
            z: None,
            dead: false,
        }
    );
    let timer = host.native("CreateTimer", &[]).unwrap();
    let Value::Handle(timer_id) = timer else {
        panic!("timer");
    };
    assert_eq!(host.inner().kind(timer_id).unwrap(), "timer");
    assert_ne!(id, timer_id);
    let family = host
        .native("DestroyTimer", &[Value::Handle(id)])
        .unwrap_err();
    assert!(family.contains("another family"), "{family}");
    assert_eq!(
        host.destructable_creation(timer_id).unwrap_err().failure,
        Failure::WrongFamily
    );
    assert_eq!(host.inner().handles(), before + 2);
    let other = host
        .native("CreateDestructableZ", &payload(-7, 1))
        .unwrap_err();
    assert!(other.contains("CreateDestructableZ"), "{other}");
    assert!(other.contains("unsupported"), "{other}");
    assert_eq!(host.inner().handles(), before + 2);
}

#[test]
fn zero_budget_malformed_calls_and_arena_capacity_keep_the_prior_record() {
    let mut empty = admitted(0, 4);
    let handles = empty.inner().handles();
    let refused = create(&mut empty, 1, 0).unwrap_err();
    assert!(refused.contains("destructable creation"), "{refused}");
    assert_eq!(empty.inner().handles(), handles);
    let mut host = admitted(2, 1);
    let created = create(&mut host, 4, 0).unwrap();
    let Value::Handle(id) = created else {
        panic!("destructable handle");
    };
    let kept = host.destructable_creation(id).unwrap();
    let handles = host.inner().handles();
    let capacity = create(&mut host, 5, 1).unwrap_err();
    assert!(capacity.contains("capacity"), "{capacity}");
    assert_eq!(host.inner().handles(), handles);
    assert_eq!(host.destructable_creation(id).unwrap(), kept);
    let malformed = host
        .native(
            "CreateDestructable",
            &[
                Value::Int(9),
                Value::Int(1),
                Value::Real(0.0),
                Value::Real(0.0),
                Value::Real(1.0),
                Value::Int(0),
            ],
        )
        .unwrap_err();
    assert!(
        malformed.contains("arguments do not match the destructable native declaration"),
        "{malformed}"
    );
    assert!(!malformed.contains("exception"), "{malformed}");
    let infinite = host
        .native(
            "CreateDestructable",
            &[
                Value::Int(9),
                Value::Real(f32::INFINITY),
                Value::Real(0.0),
                Value::Real(0.0),
                Value::Real(1.0),
                Value::Int(0),
            ],
        )
        .unwrap_err();
    assert!(
        infinite.contains("recreation policy: finite real required"),
        "{infinite}"
    );
    assert!(!infinite.contains("exception"), "{infinite}");
    assert_eq!(host.inner().handles(), handles);
    assert_eq!(host.destructable_creation(id).unwrap(), kept);
}

#[test]
fn admission_requires_widget_ancestry_one_source_and_the_declared_signature() {
    let again = match admitted(1, 4).with_destructables(DestructableBudget { max_records: 1 }) {
        Err(error) => error,
        Ok(_) => panic!("destructables were admitted twice"),
    };
    assert_eq!(
        again.failure,
        Failure::DestructableState("already admitted")
    );
    let direct = DECLARATIONS.replace(
        "type destructable extends widget\n",
        "type destructable extends agent\n",
    );
    let ancestry =
        match host_from(&direct, "", 4).with_destructables(DestructableBudget { max_records: 1 }) {
            Err(error) => error,
            Ok(_) => panic!("destructable ancestry was admitted"),
        };
    assert_eq!(ancestry.failure, Failure::Signature);
    let wrong = DECLARATIONS.replace(
        "integer variation returns destructable",
        "integer variation returns widget",
    );
    let signature =
        match host_from(&wrong, "", 4).with_destructables(DestructableBudget { max_records: 1 }) {
            Err(error) => error,
            Ok(_) => panic!("wrong return type was admitted"),
        };
    assert_eq!(signature.failure, Failure::Signature);
    let duplicate = "native CreateDestructable takes integer objectid, real x, real y, real face, real scale, integer variation returns destructable\n";
    let split = match host_from(DECLARATIONS, duplicate, 4)
        .with_destructables(DestructableBudget { max_records: 1 })
    {
        Err(error) => error,
        Ok(_) => panic!("duplicate declaration was admitted"),
    };
    assert_eq!(split.failure, Failure::Source);
    let mut bare = host_from(DECLARATIONS, "", 4);
    let unsupported = bare
        .native("CreateDestructable", &payload(1, 0))
        .unwrap_err();
    assert!(unsupported.contains("unsupported"), "{unsupported}");
}

fn variant_declarations() -> String {
    let globals = DECLARATIONS.find("\nglobals\n").expect("globals");
    let mut text = DECLARATIONS.to_string();
    text.insert_str(
        globals + 1,
        "native CreateDestructableZ takes integer objectid, real x, real y, real z, real face, real scale, integer variation returns destructable\n\
native CreateDeadDestructable takes integer objectid, real x, real y, real face, real scale, integer variation returns destructable\n\
native CreateDeadDestructableZ takes integer objectid, real x, real y, real z, real face, real scale, integer variation returns destructable\n\
native RemoveDestructable takes destructable d returns nothing\n\
native KillDestructable takes destructable d returns nothing\n",
    );
    text
}

#[test]
fn variants_share_one_budget_and_absent_or_bad_declarations_stay_isolated() {
    let declarations = variant_declarations();
    let mut host = host_from(&declarations, "", 4)
        .with_destructables(DestructableBudget { max_records: 1 })
        .unwrap();
    let parsed = pcx_script::parse_script(&declarations).unwrap();
    for name in [
        "CreateDestructable",
        "CreateDestructableZ",
        "CreateDeadDestructable",
        "CreateDeadDestructableZ",
    ] {
        let (source, span) = host
            .destructable_variant_declaration(name)
            .unwrap()
            .expect(name);
        assert_eq!(source, "synthetic:destructables");
        let declared = parsed
            .natives()
            .find(|native| native.name.name == name)
            .unwrap();
        assert_eq!(span, declared.span);
    }
    assert!(
        admitted(1, 4)
            .destructable_variant_declaration("CreateDeadDestructableZ")
            .unwrap()
            .is_none()
    );
    let handles = host.inner().handles();
    let infinite = host
        .native(
            "CreateDestructableZ",
            &[
                Value::Int(1),
                Value::Real(0.0),
                Value::Real(0.0),
                Value::Real(f32::NAN),
                Value::Real(0.0),
                Value::Real(1.0),
                Value::Int(0),
            ],
        )
        .unwrap_err();
    assert!(
        infinite.contains("recreation policy: finite real required"),
        "{infinite}"
    );
    let wrong_kind = host
        .native(
            "CreateDeadDestructableZ",
            &[
                Value::Int(1),
                Value::Real(0.0),
                Value::Real(0.0),
                Value::Int(4),
                Value::Real(0.0),
                Value::Real(1.0),
                Value::Int(0),
            ],
        )
        .unwrap_err();
    assert!(
        wrong_kind.contains("arguments do not match the destructable native declaration"),
        "{wrong_kind}"
    );
    assert!(!wrong_kind.contains("exception"), "{wrong_kind}");
    assert_eq!(host.inner().handles(), handles);
    let created = host
        .native(
            "CreateDeadDestructable",
            &[
                Value::Int(-2),
                Value::Real(0.0),
                Value::Real(1.0),
                Value::Real(-8.0),
                Value::Real(0.5),
                Value::Int(3),
            ],
        )
        .unwrap();
    let Value::Handle(id) = created else {
        panic!("dead destructable");
    };
    assert_eq!(
        host.destructable_creation(id).unwrap(),
        DestructableCreation {
            objectid: -2,
            x: 0.0,
            y: 1.0,
            z: None,
            face: -8.0,
            scale: 0.5,
            variation: 3,
            dead: true,
        }
    );
    let budget = host
        .native("CreateDestructableZ", &{
            let mut args = payload(1, 0).to_vec();
            args.insert(3, Value::Real(9.0));
            args
        })
        .unwrap_err();
    assert!(budget.contains("destructable creation"), "{budget}");
    assert!(host.destructable_creation(id).unwrap().dead);
    assert_eq!(host.inner().handles(), handles + 1);
    for name in ["RemoveDestructable", "KillDestructable"] {
        let refused = host.native(name, &[Value::Handle(id)]).unwrap_err();
        assert!(refused.contains("unsupported"), "{refused}");
        assert_eq!(host.destructable_creation(id).unwrap().variation, 3);
    }
    let broken = declarations.replace(
        "native CreateDestructableZ takes integer objectid, real x, real y, real z, real face, real scale, integer variation returns destructable",
        "native CreateDestructableZ takes integer objectid, real x, real y, real z, real face, real scale, integer variation returns widget",
    );
    let signature =
        match host_from(&broken, "", 4).with_destructables(DestructableBudget { max_records: 1 }) {
            Err(error) => error,
            Ok(_) => panic!("wrong Z declaration was admitted"),
        };
    assert_eq!(signature.failure, Failure::Signature);
    let only_in_map = "native CreateDestructableZ takes integer objectid, real x, real y, real z, real face, real scale, integer variation returns destructable\n";
    let foreign = match host_from(DECLARATIONS, only_in_map, 4)
        .with_destructables(DestructableBudget { max_records: 1 })
    {
        Err(error) => error,
        Ok(_) => panic!("cross-source Z declaration was admitted"),
    };
    assert_eq!(foreign.failure, Failure::Source);
}

fn wave() -> Vec<u8> {
    let mut bytes = b"RIFF\0\0\0\0WAVEfmt ".to_vec();
    bytes.extend(16u32.to_le_bytes());
    for value in [1u16, 1] {
        bytes.extend(value.to_le_bytes());
    }
    bytes.extend(8000u32.to_le_bytes());
    bytes.extend(16000u32.to_le_bytes());
    bytes.extend(2u16.to_le_bytes());
    bytes.extend(16u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend(4u32.to_le_bytes());
    bytes.extend([0u8, 0, 0, 0]);
    let size = (bytes.len() - 8) as u32;
    bytes[4..8].copy_from_slice(&size.to_le_bytes());
    bytes
}

#[test]
fn owned_vm_keeps_destructable_records_beside_sound_timer_and_player_identities() {
    let common = COMMON.replace(
        "type agent extends handle\n",
        "type agent extends handle\ntype widget extends agent\ntype destructable extends widget\ntype sound extends agent\n",
    );
    let globals = common.find("\nglobals\n").expect("globals block");
    let mut common = common;
    common.insert_str(
        globals + 1,
        "native CreateSound takes string file, boolean repeating, boolean spatial, boolean stop, integer fadeIn, integer fadeOut, string eax returns sound\nnative CreateDestructable takes integer objectid, real x, real y, real face, real scale, integer variation returns destructable\nnative CreateDestructableZ takes integer objectid, real x, real y, real z, real face, real scale, integer variation returns destructable\nnative CreateDeadDestructable takes integer objectid, real x, real y, real face, real scale, integer variation returns destructable\nnative CreateDeadDestructableZ takes integer objectid, real x, real y, real z, real face, real scale, integer variation returns destructable\n",
    );
    let map = "globals\nplayer P = null\ntimer T = null\nsound S = null\ndestructable A = null\ndestructable B = null\ndestructable C = null\ndestructable D = null\nendglobals\nfunction main takes nothing returns nothing\nset P = Player(0)\nset T = CreateTimer()\nset S = CreateSound(\"Media\\\\a.wav\", false, false, false, 0, 0, \"\")\nset A = CreateDestructable(-7, 1.5, -2.25, 0.0, 3.5, -1)\nset B = CreateDestructableZ(-3, 0.5, 2.0, -4.25, 90.0, 1.0, 2)\nset C = CreateDeadDestructable(8, -1.0, 0.0, 15.0, 0.25, 0)\nset D = CreateDeadDestructableZ(1, 3.0, -3.0, 12.5, 180.0, 2.0, -5)\nendfunction\n";
    let ctx = context(301, &common, map, "destructable-creation");
    let pcm = wave();
    let assets = Assets::read(
        &[SoundInput {
            member: r"Media\a.wav",
            asset: ScriptAsset {
                label: "synthetic:archive::Media\\a.wav",
                bytes: &pcm,
            },
            origin: AssetIdentity {
                edition: "synthetic".into(),
                version: "test".into(),
                source: "synthetic:PCM".into(),
            },
        }],
        SoundBudget {
            resources: 1,
            resource_bytes: pcm.len(),
            total_bytes: pcm.len(),
            metadata_bytes: 1024,
        },
    )
    .unwrap();
    let host = object(&ctx, 16)
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_file_sounds(assets, 8)
        .unwrap()
        .with_destructables(DestructableBudget { max_records: 4 })
        .unwrap();
    let (source, span) = host.destructable_declaration().unwrap();
    assert_eq!(source, "synthetic/common.j");
    assert!(common[span.start as usize..span.end as usize].contains("CreateDestructable"));
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(host);
    vm.init_globals().unwrap();
    vm.call("main", &[]).unwrap();
    let Value::Handle(player) = vm.global("P") else {
        panic!("player");
    };
    let Value::Handle(timer) = vm.global("T") else {
        panic!("timer");
    };
    let Value::Handle(sound) = vm.global("S") else {
        panic!("sound");
    };
    let Value::Handle(base) = vm.global("A") else {
        panic!("base destructable");
    };
    let Value::Handle(elevated) = vm.global("B") else {
        panic!("z destructable");
    };
    let Value::Handle(dead) = vm.global("C") else {
        panic!("dead destructable");
    };
    let Value::Handle(dead_z) = vm.global("D") else {
        panic!("dead z destructable");
    };
    let host = vm.host();
    assert_eq!(host.inner.inner().kind(player).unwrap(), "player");
    assert_eq!(host.inner.inner().kind(timer).unwrap(), "timer");
    assert_eq!(host.inner.inner().kind(sound).unwrap(), "sound");
    for id in [base, elevated, dead, dead_z] {
        assert_eq!(host.inner.inner().kind(id).unwrap(), "destructable");
    }
    assert_eq!(
        host.inner
            .native("GetPlayerId", &[Value::Handle(player)])
            .unwrap(),
        Value::Int(0)
    );
    assert_eq!(
        host.inner
            .file_sound_state(sound)
            .unwrap()
            .resource
            .member(),
        r"Media\a.wav"
    );
    assert_eq!(
        host.inner.destructable_creation(base).unwrap(),
        DestructableCreation {
            objectid: -7,
            x: 1.5,
            y: -2.25,
            z: None,
            face: 0.0,
            scale: 3.5,
            variation: -1,
            dead: false,
        }
    );
    assert_eq!(
        host.inner.destructable_creation(elevated).unwrap().z,
        Some(-4.25)
    );
    assert!(!host.inner.destructable_creation(elevated).unwrap().dead);
    assert_eq!(host.inner.destructable_creation(dead).unwrap().z, None);
    assert!(host.inner.destructable_creation(dead).unwrap().dead);
    assert_eq!(
        host.inner.destructable_creation(dead_z).unwrap().z,
        Some(12.5)
    );
    assert!(host.inner.destructable_creation(dead_z).unwrap().dead);
    assert_eq!(
        host.inner.destructable_creation(sound).unwrap_err().failure,
        Failure::WrongFamily
    );
    let cross = host
        .inner
        .native("GetPlayerId", &[Value::Handle(base)])
        .unwrap_err();
    assert!(cross.contains("another family"), "{cross}");
}
