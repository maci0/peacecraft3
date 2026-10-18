use super::*;
use crate::terrain_pathing::{TerrainPathing, tests::input};

fn common() -> String {
    format!(
        "type pathingtype extends handle\nconstant native ConvertPathingType takes integer i returns pathingtype\nnative IsTerrainPathable takes real x, real y, pathingtype t returns boolean\n{COMMON}\nglobals\nconstant pathingtype WALK = ConvertPathingType(1)\nconstant pathingtype OTHER = ConvertPathingType(2)\nconstant pathingtype UNKNOWN = ConvertPathingType(-5)\nendglobals\n"
    )
}

fn owned(common: &str, map: &str) -> (Arc<ParsedMapContext>, ObjectHost) {
    let map = if map.is_empty() {
        "// no map globals"
    } else {
        map
    };
    let ctx = context(300, common, map, "synthetic:terrain-query");
    let e = EnumHost::checked_selected(
        ctx.scripts.shared_program(),
        "synthetic:terrain-query",
        &[
            "ConvertPlacement",
            "ConvertPlayerColor",
            "ConvertRacePref",
            "ConvertMapControl",
            "ConvertPathingType",
        ],
        Limits {
            max_handles: 32,
            admission_steps: 1000,
            expression_depth: 16,
            expression_work: 1000,
        },
    )
    .unwrap();
    let host = ObjectHost::checked(e)
        .unwrap()
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap();
    (ctx, host)
}

fn query(host: &mut ObjectHost, x: f32, y: f32, kind: Value) -> Result<Value, String> {
    host.native("IsTerrainPathable", &[Value::Real(x), Value::Real(y), kind])
}

fn selected() -> TerrainPathing {
    input(4, &[0xff, 0x10, 0x80])
}

#[test]
fn selected_masks_and_scale_query_the_owned_raster_without_allocation() {
    let (ctx, host) = owned(&common(), "");
    let mut host = host.with_terrain_pathing(selected()).unwrap();
    let walk = host.native("ConvertPathingType", &[Value::Int(1)]).unwrap();
    let other = host.native("ConvertPathingType", &[Value::Int(2)]).unwrap();
    let unknown = host
        .native("ConvertPathingType", &[Value::Int(-5)])
        .unwrap();
    let before = host.native("CreateTimer", &[]).unwrap();
    for y in 0..ctx.world.pathing.height {
        for x in 0..ctx.world.pathing.width {
            let flags = ctx.world.pathing.cell(x, y).unwrap();
            for (kind, mask) in [(&walk, 0x10), (&other, 0x80), (&unknown, 0)] {
                for fraction in [0.0, 0.5, 15.5] {
                    assert_eq!(
                        query(
                            &mut host,
                            x as f32 * 16.0 + fraction,
                            y as f32 * 16.0,
                            kind.clone()
                        )
                        .unwrap(),
                        Value::Bool(flags & mask != 0)
                    );
                }
            }
        }
    }
    assert!(std::sync::Arc::ptr_eq(host.player_context().unwrap(), &ctx));
    let after = host.native("CreateTimer", &[]).unwrap();
    assert_eq!(id(&after).token(), id(&before).token() + 1);
    assert!(host.with_terrain_pathing(selected()).is_err());
}

#[test]
fn invalid_coordinates_families_identities_and_omitted_admission_refuse() {
    let (_, mut bare) = owned(&common(), "");
    let foreign = bare.native("ConvertPathingType", &[Value::Int(1)]).unwrap();
    assert!(query(&mut bare, 0.0, 0.0, foreign.clone()).is_err());
    let (_, host) = owned(&common(), "");
    let mut host = host.with_terrain_pathing(selected()).unwrap();
    let kind = host.native("ConvertPathingType", &[Value::Int(1)]).unwrap();
    let color = host
        .native("ConvertPlayerColor", &[Value::Int(37)])
        .unwrap();
    let missing = Value::Handle(HandleId::new(std::num::NonZeroU64::new(u64::MAX).unwrap()));
    for value in [missing, color, Value::Null, Value::Int(1)] {
        assert!(query(&mut host, 0.0, 0.0, value).is_err());
    }
    for (x, y) in [
        (f32::NAN, 0.0),
        (0.0, f32::INFINITY),
        (-0.5, 0.0),
        (64.0, 0.0),
        (0.0, 32.0),
        (f32::from_bits(1), 0.0),
    ] {
        assert!(query(&mut host, x, y, kind.clone()).is_err());
    }
    assert!(host.native("IsTerrainPathable", &[]).is_err());
    assert!(host.native("SetTerrainPathable", &[]).is_err());
    assert!(query(&mut host, 0.0, 0.0, kind).is_ok());
}

#[test]
fn vm_native_uses_the_same_admitted_map_context() {
    let map = "function check takes nothing returns boolean\nreturn IsTerrainPathable(16.5, 0.0, WALK)\nendfunction\n";
    let (ctx, host) = owned(&common(), map);
    let host = host.with_terrain_pathing(selected()).unwrap();
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(host);
    drop(ctx);
    vm.init_globals().unwrap();
    assert_eq!(vm.call("check", &[]).unwrap(), Value::Bool(false));
}

#[test]
fn declaration_contract_source_and_admitted_map_are_required() {
    for bad in [
        common().replace(
            "native IsTerrainPathable",
            "constant native IsTerrainPathable",
        ),
        common().replace("real y, pathingtype t", "integer y, pathingtype t"),
        common().replace(
            "native IsTerrainPathable takes real x, real y, pathingtype t returns boolean\n",
            "",
        ),
    ] {
        let (_, host) = owned(&bad, "");
        assert!(host.with_terrain_pathing(selected()).is_err());
    }
    let common = common().replace(
        "native IsTerrainPathable takes real x, real y, pathingtype t returns boolean\n",
        "",
    );
    let (_, host) = owned(
        &common,
        "native IsTerrainPathable takes real x, real y, pathingtype t returns boolean\n",
    );
    assert!(host.with_terrain_pathing(selected()).is_err());
}
