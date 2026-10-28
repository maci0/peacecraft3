//! Synthetic published session with an admitted game camera, own and foreign units and
//! town halls, for the camera input tests.
use super::*;
use pcx_objects::editor::camera::GameCamera;
use pcx_objects::editor::{FieldDef, FieldKind, FieldSource};
use pcx_script::{Host, Value};
use pcx_sim::world::{Definition, FieldValue, ObjectCatalog, ObjectField, ResolutionProblem};

/// Unit type rawcodes of the synthetic catalog and their `UnitBalance/type` tokens.
pub(crate) const PEON: &str = "peon";
pub(crate) const HALL: &str = "hall";
pub(crate) const FOOTMAN: &str = "foot";
const KINDS: &[(&str, &str, &str)] = &[
    (PEON, "Peon", "0"),
    (HALL, "TownHall", "1"),
    (FOOTMAN, "", "0"),
];
/// Game camera at the first zoom level, as the `[Camera]` file lists it.
pub(crate) const GAME_CAMERA: GameCamera = GameCamera {
    angle_of_attack: 304.0,
    field_of_view: 70.0,
    rotation: 90.0,
    distance: 1650.0,
    far_z: 5000.0,
    height: 0.0,
    forward_rate: 3000.0,
    strafe_rate: 3000.0,
};

const EXTRA: &str = "native ShowUnit takes unit u, boolean shown returns nothing\nconstant native IsUnitHidden takes unit u returns boolean\nconstant native GetTriggerUnit takes nothing returns unit\nconstant native GetEnteringUnit takes nothing returns unit\nconstant native GetFilterUnit takes nothing returns unit\nconstant native GetUnitX takes unit u returns real\nconstant native GetUnitY takes unit u returns real\nnative GroupEnumUnitsInRect takes group g, rect r, boolexpr f returns nothing\nnative TriggerRegisterUnitInRange takes trigger t, unit u, real r, boolexpr f returns event\nconstant native GetLocalPlayer takes nothing returns player\nglobals\nconstant unitevent TEST_RANGE=ConvertUnitEvent(-47)\nconstant camerafield CAMERA_FIELD_TARGET_DISTANCE = ConvertCameraField(11)\nconstant camerafield CAMERA_FIELD_FARZ = ConvertCameraField(14)\nconstant camerafield CAMERA_FIELD_ANGLE_OF_ATTACK = ConvertCameraField(17)\nconstant camerafield CAMERA_FIELD_FIELD_OF_VIEW = ConvertCameraField(20)\nconstant camerafield CAMERA_FIELD_ROLL = ConvertCameraField(23)\nconstant camerafield CAMERA_FIELD_ROTATION = ConvertCameraField(26)\nconstant camerafield CAMERA_FIELD_ZOFFSET = ConvertCameraField(29)\nendglobals\n";

struct Units;
impl ObjectCatalog for Units {
    fn resolve(&self, rawcode: [u8; 4]) -> Result<Definition, ResolutionProblem> {
        let name = String::from_utf8_lossy(&rawcode).into_owned();
        let Some((_, kind, structure)) = KINDS.iter().find(|(raw, ..)| *raw == name) else {
            return Err(ResolutionProblem::MissingRawcode(name));
        };
        let mut definition = super::unit_range_cases::Geometry.resolve(*b"UNIT")?;
        definition.rawcode = rawcode;
        definition.base_rawcode = name.clone();
        for (index, (slk, column, kind, value)) in [
            (SlkId::UnitBalance, "isbldg", FieldKind::Bool, *structure),
            (SlkId::UnitUi, "scale", FieldKind::Real, "0.2"),
            (SlkId::UnitBalance, "type", FieldKind::Token, *kind),
        ]
        .into_iter()
        .enumerate()
        {
            let id = [b'B', b'0', b'A' + index as u8, b'0'];
            definition.fields.insert(
                id,
                ObjectField {
                    metadata: Some(FieldDef {
                        fourcc: id,
                        from: SlkId::UnitMetaData,
                        field: column.into(),
                        slk: slk.name().into(),
                        index: -1,
                        declared: match kind {
                            FieldKind::Bool => "bool",
                            FieldKind::Real => "real",
                            _ => "string",
                        }
                        .into(),
                        kind,
                        category: "synthetic camera input".into(),
                        source: FieldSource::Column {
                            slk,
                            column: column.into(),
                        },
                        measured: true,
                    }),
                    value: Ok(FieldValue::Cell(value.into())),
                },
            );
        }
        Ok(definition)
    }
}

/// A session whose `main` runs `script`; Player(0) is the local player. The game camera
/// is admitted and aimed at (64, 64).
pub(crate) fn session(script: &str) -> MapScriptSession<MapStartupHost> {
    let mut map = fixture(script);
    map.common
        .extend_from_slice(super::script_timer_cases::TIMER_COMMON.as_bytes());
    map.common
        .extend_from_slice(super::timed_trigger_cases::COMMON.as_bytes());
    map.common.extend_from_slice(EXTRA.as_bytes());
    map.rebuild();
    let reader = map.resolver();
    let bindings = bind_map(&reader, map.input(1), map.limits()).unwrap();
    let mut inputs = host_inputs(Arc::new(Units));
    inputs.timer_time = Some(10.);
    inputs.enumeration.names.push("ConvertGameEvent".into());
    inputs.enumeration.limits.max_handles = 256;
    inputs.budgets.camera_setup_fields = 7;
    inputs.budgets.conditions =
        Some(pcx_play::playercolor_host::ConditionBudget { max_records: 8 });
    inputs.budgets.trigger_conditions = Some(8);
    inputs.budgets.unit_in_range = Some(8);
    inputs.budgets.units.max_retained_fields = 64;
    inputs.budgets.units.max_records = 64;
    inputs.trigger_runtime = Some(pcx_play::playercolor_host::TriggerRuntimeInput {
        initial_enabled: true,
        timer_event_global: "TEST_TIMER_EVENT".into(),
        max_timed_events: 8,
        max_parked_threads: 8,
    });
    let payload = Arc::clone(&bindings.catalog_map_bytes);
    let mut app = App::new();
    peacecraft3::map_requests::install(&mut app);
    app.insert_resource(MapRequests {
        bindings: Some(bindings),
        ..Default::default()
    });
    app.world_mut().write_message(MapRequest {
        label: "synthetic camera input".into(),
        map_member: pcx_assets::AssetPath::new("selected map.w3m").unwrap(),
        identity: map.map_id,
        binding_revision: 1,
        payload: MapPayload::Shared(payload),
    });
    app.update();
    let context = app
        .world()
        .resource::<MapRequests>()
        .admission
        .shared_context()
        .unwrap();
    let entries =
        ["config", "main"].map(|name| context.scripts.program().function_id(name).unwrap());
    let mut session = MapScriptSession::default();
    session
        .replace(
            Arc::clone(&context),
            move |ctx| {
                let source = ctx.world.source.clone();
                let mut host = pcx_play::map_startup_host::assemble(ctx.clone(), inputs)?;
                host.objects_mut()
                    .admit_unit_placement(super::unit_range_cases::placement(source))?;
                host.objects_mut()
                    .admit_unit_range_runtime("TEST_RANGE", 8)?;
                host.objects_mut().admit_local_player(&ctx, 0)?;
                host.objects_mut().admit_camera_view(&GAME_CAMERA)?;
                host.objects_mut().scroll_camera([64.0, 64.0])?;
                // The player rows exist once queried; a scene may hold no unit of the local one.
                for number in [0, 1] {
                    host.objects_mut()
                        .native("Player", &[Value::Int(number)])
                        .map_err(pcx_play::map_startup_host::MapStartupError::Observation)?;
                }
                Ok::<_, pcx_play::map_startup_host::MapStartupError>(host)
            },
            &entries,
        )
        .unwrap();
    session
}

#[test]
fn synthetic_camera_session_has_a_game_camera_and_typed_town_halls() {
    let mut session = session(
        "function config takes nothing returns nothing\nendfunction\nfunction main takes nothing returns nothing\ncall CreateUnit(Player(0),'peon',16.0,16.0,0.0)\ncall CreateUnit(Player(0),'hall',48.0,20.0,0.0)\ncall CreateUnit(Player(1),'hall',90.0,90.0,0.0)\nendfunction\n",
    );
    let objects = session.host().unwrap().objects();
    assert_eq!(objects.camera_target(), Some([64.0, 64.0]));
    assert_eq!(objects.camera_scroll_rates(), Some([3000.0, 3000.0]));
    let halls = objects.town_halls(0).unwrap();
    assert_eq!(halls.len(), 1);
    assert_eq!(halls[0].1, [48.0, 20.0]);
    assert_eq!(objects.town_halls(1).unwrap().len(), 1);
    assert!(objects.town_halls(2).unwrap().is_empty());
    assert_eq!(objects.unit_camera_point(halls[0].0).unwrap(), [48.0, 20.0]);
}
