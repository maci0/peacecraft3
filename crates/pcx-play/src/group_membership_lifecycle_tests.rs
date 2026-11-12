use super::*;

#[test]
fn membership_null_and_removed_units_share_canonical_group_cleanup() {
    let map = r#"
globals
unit U = null
group G = CreateGroup()
group OTHER = CreateGroup()
endglobals
function main takes nothing returns nothing
set U = CreateUnit(Player(4), 'q007', 16.0, 16.0, 0.0)
call GroupAddUnit(G, U)
call GroupAddUnit(OTHER, U)
endfunction
function contains takes nothing returns boolean
return IsUnitInGroup(U, G)
endfunction
function missing takes nothing returns boolean
return IsUnitInGroup(null, G)
endfunction
function remove takes nothing returns nothing
call RemoveUnit(U)
endfunction
"#;
    let h = position_host_program(
        100,
        8,
        map,
        "native RemoveUnit takes unit u returns nothing\n",
    );
    let context = Arc::clone(h.player_context().unwrap());
    let mut vm = context.scripts.clone().owned_vm(h);
    vm.init_globals().unwrap();
    vm.call("main", &[]).unwrap();
    assert_eq!(vm.call("contains", &[]).unwrap(), Value::Bool(true));
    assert_eq!(vm.call("missing", &[]).unwrap(), Value::Bool(false));
    let handles = vm.host().inner.inner().handles();
    vm.call("remove", &[]).unwrap();
    assert_eq!(vm.call("contains", &[]).unwrap(), Value::Bool(false));
    assert_eq!(vm.host().inner.memberships(), 0);
    let unit = vm.global("U");
    let other = vm.global("OTHER");
    assert_eq!(
        vm.host()
            .inner
            .native("IsUnitInGroup", &[unit.clone(), other])
            .unwrap(),
        Value::Bool(false)
    );
    assert_eq!(
        vm.host()
            .inner
            .native("IsUnitInGroup", &[unit.clone(), Value::Null])
            .unwrap(),
        Value::Bool(false)
    );
    let Value::Handle(unit) = unit else {
        panic!("unit");
    };
    assert_eq!(vm.host().inner.inner().kind(unit).unwrap(), "unit");
    assert_eq!(vm.host().inner.inner().handles(), handles);
    vm.call("remove", &[]).unwrap();
    assert_eq!(vm.call("contains", &[]).unwrap(), Value::Bool(false));
}
