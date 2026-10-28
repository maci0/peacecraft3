//! Synthetic published session with owned and foreign units for drag-box selection tests.
use super::*;
use pcx_objects::editor::{FieldDef, FieldKind, FieldSource};
use pcx_script::{Host, Value};
use pcx_sim::world::{Definition, FieldValue, ObjectCatalog, ObjectField, ResolutionProblem};

/// Unit type rawcodes of the synthetic catalog.
pub(crate) const PEON: &str = "peon";
pub(crate) const FOOTMAN: &str = "foot";
pub(crate) const HALL: &str = "hall";

const EXTRA: &str = "native ShowUnit takes unit u, boolean shown returns nothing\nconstant native IsUnitHidden takes unit u returns boolean\nconstant native GetTriggerUnit takes nothing returns unit\nconstant native GetEnteringUnit takes nothing returns unit\nconstant native GetFilterUnit takes nothing returns unit\nconstant native GetUnitX takes unit u returns real\nconstant native GetUnitY takes unit u returns real\nnative GroupEnumUnitsInRect takes group g, rect r, boolexpr f returns nothing\nnative TriggerRegisterUnitInRange takes trigger t, unit u, real r, boolexpr f returns event\nconstant native GetLocalPlayer takes nothing returns player\nglobals\nconstant unitevent TEST_RANGE=ConvertUnitEvent(-47)\nendglobals\n";
const UNITS: &[&str] = &[PEON, FOOTMAN, HALL];

struct Units;
impl ObjectCatalog for Units {
    fn resolve(&self, rawcode: [u8; 4]) -> Result<Definition, ResolutionProblem> {
        let name = String::from_utf8_lossy(&rawcode).into_owned();
        if !UNITS.contains(&name.as_str()) {
            return Err(ResolutionProblem::MissingRawcode(name));
        }
        let mut definition = super::unit_range_cases::Geometry.resolve(*b"UNIT")?;
        definition.rawcode = rawcode;
        definition.base_rawcode = name.clone();
        for (index, (slk, column, kind, value)) in [
            (
                SlkId::UnitBalance,
                "isbldg",
                FieldKind::Bool,
                if name == HALL { "1" } else { "0" },
            ),
            (SlkId::UnitUi, "scale", FieldKind::Real, "0.2"),
            (
                SlkId::UnitData,
                "prio",
                FieldKind::Int,
                match name.as_str() {
                    PEON => "1",
                    FOOTMAN => "6",
                    _ => "3",
                },
            ),
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
                            FieldKind::Int => "int",
                            _ => "real",
                        }
                        .into(),
                        kind,
                        category: "synthetic box selection".into(),
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

/// A session whose `main` runs `script`; Player(0) is the local player.
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
        label: "synthetic box selection".into(),
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
fn synthetic_box_session_creates_owned_and_foreign_units_for_the_local_player() {
    let mut session = session(
        "function config takes nothing returns nothing\nendfunction\nfunction main takes nothing returns nothing\ncall CreateUnit(Player(0),'peon',16.0,16.0,0.0)\ncall CreateUnit(Player(1),'hall',48.0,16.0,0.0)\nendfunction\n",
    );
    let objects = session.host().unwrap().objects();
    assert_eq!(objects.executed_units().count(), 2);
    let candidates: Vec<_> = objects
        .executed_units()
        .map(|(id, _)| objects.box_candidate(id).unwrap().unwrap())
        .collect();
    assert_eq!(
        candidates
            .iter()
            .map(|c| (c.own, c.structure))
            .collect::<Vec<_>>(),
        [(true, false), (false, true)]
    );
}
