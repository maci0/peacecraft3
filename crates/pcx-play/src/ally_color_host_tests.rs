use super::*;
const COLORS:&[u8]=b"[TeamColorFilter]\nColorIndexPlayer=3\nColorIndexAlly=8\nColorIndexCreep=9\nColorIndexEnemy=10\n[FogOfWar]\nFogColorPlayer=17,7,11,13\nFogColorAlly=19,23,29,31\nFogColorCreepNormal=37,41,43,47\nFogColorEnemy=53,59,61,67\n";
#[test]
fn filter_roundtrip_recolors_the_same_world_and_tracks_directional_alliances() {
    let (mut vm, a, b) = open(&armed("100", "normal"));
    let alliance = vm.global("ALLIANCE_PASSIVE");
    let h = &mut vm.host().inner;
    let context = Arc::clone(h.player_context().unwrap());
    h.admit_local_player(&context, 4).unwrap();
    let mut rules = crate::ally_color::Rules::fixture();
    rules.image_sha256 = h.player_client_sha256().unwrap();
    let input = crate::ally_color::Input {
        rules,
        palette: crate::ally_color::Palette::read(&[("synthetic", COLORS)], 4096).unwrap(),
    };
    h.admit_ally_colors(input).unwrap();
    assert_eq!(
        h.native("GetAllyColorFilterState", &[]).unwrap(),
        Value::Int(7)
    );
    assert_eq!(
        h.display_team_color(h.executed_unit(a).unwrap(), 4)
            .unwrap(),
        4
    );
    assert_eq!(
        h.minimap_role_color(h.executed_unit(a).unwrap()).unwrap(),
        Some([7, 11, 13, 17])
    );
    // The fixture's start option is 7, which the mode handler treats as minimap ally colours,
    // but the client constructor reads the raw option and starts on player colours.
    assert_eq!(
        h.minimap_role_color(h.executed_unit(b).unwrap()).unwrap(),
        None
    );
    h.native("SetAllyColorFilterState", &[Value::Int(7)])
        .unwrap();
    assert_eq!(
        h.minimap_role_color(h.executed_unit(b).unwrap()).unwrap(),
        Some([59, 61, 67, 53])
    );
    h.native("SetAllyColorFilterState", &[Value::Int(0)])
        .unwrap();
    assert_eq!(
        h.minimap_role_color(h.executed_unit(b).unwrap()).unwrap(),
        None
    );
    assert_eq!(
        h.minimap_role_color(h.executed_unit(a).unwrap()).unwrap(),
        Some([7, 11, 13, 17])
    );
    let handles = h.inner().handles();
    h.native("SetAllyColorFilterState", &[Value::Int(11)])
        .unwrap();
    assert_eq!(
        h.display_team_color(h.executed_unit(a).unwrap(), 4)
            .unwrap(),
        3
    );
    assert_eq!(
        h.display_team_color(h.executed_unit(b).unwrap(), 5)
            .unwrap(),
        10
    );
    let local = h.native("Player", &[Value::Int(4)]).unwrap();
    let other = h.native("Player", &[Value::Int(5)]).unwrap();
    h.native(
        "SetPlayerAlliance",
        &[
            other.clone(),
            local.clone(),
            alliance.clone(),
            Value::Bool(true),
        ],
    )
    .unwrap();
    assert_eq!(
        h.display_team_color(h.executed_unit(b).unwrap(), 5)
            .unwrap(),
        10
    );
    h.native(
        "SetPlayerAlliance",
        &[local, other, alliance, Value::Bool(true)],
    )
    .unwrap();
    assert_eq!(
        h.display_team_color(h.executed_unit(b).unwrap(), 5)
            .unwrap(),
        8
    );
    for args in [
        vec![],
        vec![Value::Null],
        vec![Value::Real(11.)],
        vec![Value::Int(7), Value::Int(11)],
    ] {
        assert!(h.native("SetAllyColorFilterState", &args).is_err());
    }
    assert_eq!(
        h.native("GetAllyColorFilterState", &[]).unwrap(),
        Value::Int(11)
    );
    h.native("SetAllyColorFilterState", &[Value::Int(-19)])
        .unwrap();
    assert_eq!(
        h.native("GetAllyColorFilterState", &[]).unwrap(),
        Value::Int(-19)
    );
    assert_eq!(
        h.display_team_color(h.executed_unit(b).unwrap(), 5)
            .unwrap(),
        5
    );
    assert_eq!(h.inner().handles(), handles);
}

#[test]
fn admission_is_atomic_and_neutral_display_roles_stay_distinct() {
    let (mut vm, _, b) = open(&armed("100", "normal"));
    let h = &mut vm.host().inner;
    let context = Arc::clone(h.player_context().unwrap());
    h.admit_local_player(&context, 4).unwrap();
    let mut rules = crate::ally_color::Rules::fixture();
    rules.creep_first = 5;
    rules.unfiltered_owner = 5;
    let mut input = crate::ally_color::Input {
        rules,
        palette: crate::ally_color::Palette::read(&[("synthetic", COLORS)], 4096).unwrap(),
    };
    input.rules.image_sha256 = [0xff; 32];
    assert!(h.admit_ally_colors(input.clone()).is_err());
    assert_eq!(h.ally_color_filter_state(), None);
    input.rules.image_sha256 = h.player_client_sha256().unwrap();
    h.admit_ally_colors(input.clone()).unwrap();
    h.native("SetAllyColorFilterState", &[Value::Int(11)])
        .unwrap();
    assert!(h.admit_ally_colors(input).is_err());
    assert_eq!(h.ally_color_filter_state(), Some(11));
    assert_eq!(
        h.display_team_color(h.executed_unit(b).unwrap(), 5)
            .unwrap(),
        5
    );
    assert_eq!(
        h.minimap_role_color(h.executed_unit(b).unwrap()).unwrap(),
        Some([41, 43, 47, 37])
    );
}
