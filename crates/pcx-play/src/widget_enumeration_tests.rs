use super::*;
use pcx_script::Value;
const EXTRA: &str = r#"
type rect extends agent
type boolexpr extends agent
type filterfunc extends boolexpr
type destructable extends widget
native Filter takes code f returns filterfunc
native DestroyFilter takes filterfunc f returns nothing
native Rect takes real a, real b, real c, real d returns rect
native RemoveRect takes rect r returns nothing
native SetRect takes rect r, real a, real b, real c, real d returns nothing
native MoveRectTo takes rect r, real x, real y returns nothing
native GetRectCenterX takes rect r returns real
native GetRectCenterY takes rect r returns real
native GetRectMinX takes rect r returns real
native GetRectMinY takes rect r returns real
native GetRectMaxX takes rect r returns real
native GetRectMaxY takes rect r returns real
native SetItemVisible takes item i, boolean show returns nothing
native IsItemVisible takes item i returns boolean
native EnumItemsInRect takes rect r, boolexpr f, code action returns nothing
constant native GetEnumItem takes nothing returns item
constant native GetFilterItem takes nothing returns item
native CreateDestructable takes integer id, real x, real y, real face, real scale, integer variant returns destructable
native EnumDestructablesInRect takes rect r, boolexpr f, code action returns nothing
constant native GetEnumDestructable takes nothing returns destructable
constant native GetFilterDestructable takes nothing returns destructable
native ShowDestructable takes destructable d, boolean show returns nothing
"#;
fn vm(script: &str) -> pcx_script::OwnedVm<ObjectHost> {
    let common = pcx_script::parse(&format!("{COMMON}{EXTRA}")).unwrap();
    let map = pcx_script::parse(script).unwrap();
    let program = Arc::new(
        pcx_script::resolve_with(&map, &[Prelude::new("synthetic common", &common)]).unwrap(),
    );
    let inner = EnumHost::checked_selected(
        program,
        "widget-enumeration",
        &["ConvertPlayerColor"],
        Limits {
            max_handles: 128,
            admission_steps: 100,
            expression_depth: 16,
            expression_work: 100,
        },
    )
    .unwrap();
    let host = ObjectHost::checked(inner)
        .unwrap()
        .with_rectangles()
        .unwrap()
        .with_filters(crate::playercolor_host::FilterBudget { max_records: 8 })
        .unwrap()
        .with_item_catalog(
            Arc::new(Catalog(Arc::new(AtomicUsize::new(0)))),
            source(),
            ItemBudget {
                max_records: 16,
                ..budget()
            },
        )
        .unwrap()
        .with_item_visibility(crate::item_visibility::Rules::fixture())
        .unwrap()
        .with_destructables(crate::playercolor_host::DestructableBudget { max_records: 8 })
        .unwrap();
    host.into_vm()
}
const SCENE: &str = r#"
globals
item A=null
item B=null
item C=null
destructable D=null
destructable E=null
rect R=null
rect SMALL=null
integer HITS=0
integer INNER=0
integer TREES=0
boolean OK=true
endglobals
function setup takes nothing returns nothing
set A=CreateItem('ITEM',0.,0.)
set B=CreateItem('ITEM',10.,10.)
set C=CreateItem('ITEM',11.,10.)
set D=CreateDestructable('TREE',0.,0.,0.,1.,0)
set E=CreateDestructable('TREE',10.,10.,0.,1.,0)
set R=Rect(0.,0.,10.,10.)
set SMALL=Rect(0.,0.,0.,0.)
endfunction
function inner takes nothing returns nothing
set INNER=INNER+1
set OK=OK and GetEnumItem()==A and GetFilterItem()==A
endfunction
function predicate takes nothing returns boolean
if GetFilterItem()==A then
call EnumItemsInRect(SMALL,null,function inner)
set OK=OK and GetFilterItem()==A
return true
endif
return false
endfunction
function outer takes nothing returns nothing
set HITS=HITS+1
set OK=OK and GetEnumItem()==A and GetFilterItem()==null
endfunction
function tree takes nothing returns nothing
set TREES=TREES+1
call ShowDestructable(GetEnumDestructable(),false)
endfunction
function enumerate takes nothing returns nothing
call EnumItemsInRect(R,Filter(function predicate),function outer)
call EnumDestructablesInRect(R,null,function tree)
endfunction
"#;
#[test]
fn real_callbacks_filter_nested_items_and_hide_the_canonical_destructables() {
    let mut v = vm(SCENE);
    v.call("setup", &[]).unwrap();
    v.call("enumerate", &[]).unwrap();
    assert_eq!(v.global("HITS"), Value::Int(1));
    assert_eq!(v.global("INNER"), Value::Int(1));
    assert_eq!(v.global("TREES"), Value::Int(2));
    assert_eq!(v.global("OK"), Value::Bool(true));
    for global in ["D", "E"] {
        let Value::Handle(id) = v.global(global) else {
            panic!()
        };
        assert!(v.host().destructable_state(id).unwrap().hidden);
    }
    for name in [
        "GetEnumItem",
        "GetFilterItem",
        "GetEnumDestructable",
        "GetFilterDestructable",
    ] {
        assert_eq!(v.host().native(name, &[]).unwrap(), Value::Null);
    }
}
#[test]
fn item_snapshot_excludes_carried_and_removed_and_defers_new_births() {
    let source = format!(
        "{SCENE}\nfunction birth takes nothing returns nothing\nset HITS=HITS+1\nif HITS==1 then\nset C=CreateItem('ITEM',5.,5.)\nendif\nendfunction\nfunction run takes nothing returns nothing\ncall EnumItemsInRect(R,null,function birth)\nendfunction\n"
    );
    let mut v = vm(&source);
    v.call("setup", &[]).unwrap();
    let Value::Handle(b) = v.global("B") else {
        panic!()
    };
    let Value::Handle(a) = v.global("A") else {
        panic!()
    };
    let Entry::Item { record } = v.host().inner.entry_mut(b).unwrap() else {
        panic!()
    };
    record.held_by = Some((a, 0));
    v.call("run", &[]).unwrap();
    assert_eq!(v.global("HITS"), Value::Int(1));
    v.call("run", &[]).unwrap();
    assert_eq!(v.global("HITS"), Value::Int(3));
    *v.host().inner.entry_mut(a).unwrap() = Entry::RemovedItem;
    v.call("run", &[]).unwrap();
    assert_eq!(v.global("HITS"), Value::Int(4));
}
#[test]
fn invalid_actions_and_callback_failures_leave_no_stale_responses() {
    let source = format!(
        "{SCENE}\nfunction wrong takes nothing returns boolean\nreturn true\nendfunction\nfunction invalid takes nothing returns nothing\ncall EnumItemsInRect(R,Filter(function predicate),function wrong)\nendfunction\nfunction bad takes nothing returns nothing\ncall DestroyTimer(GetEnumItem())\nendfunction\nfunction fail takes nothing returns nothing\ncall EnumItemsInRect(R,null,function bad)\nendfunction\n"
    );
    let mut v = vm(&source);
    v.call("setup", &[]).unwrap();
    assert!(v.call("invalid", &[]).is_err());
    assert_eq!(v.global("HITS"), Value::Int(0));
    assert!(v.call("fail", &[]).is_err());
    assert_eq!(v.host().native("GetEnumItem", &[]).unwrap(), Value::Null);
    v.call("enumerate", &[]).unwrap();
    assert_eq!(v.global("HITS"), Value::Int(1));
}

#[test]
fn rectangle_callbacks_hide_and_restore_the_same_canonical_items() {
    let mut v = vm(&format!(
        "{SCENE}\nfunction hide takes nothing returns nothing\ncall SetItemVisible(GetEnumItem(),false)\nendfunction\nfunction show takes nothing returns nothing\ncall SetItemVisible(GetEnumItem(),true)\nendfunction\nfunction hideAll takes nothing returns nothing\ncall EnumItemsInRect(R,null,function hide)\nendfunction\nfunction showAll takes nothing returns nothing\ncall EnumItemsInRect(R,null,function show)\nendfunction"
    ));
    v.call("setup", &[]).unwrap();
    v.call("hideAll", &[]).unwrap();
    for (name, shown) in [("A", false), ("B", false), ("C", true)] {
        let item = v.global(name);
        assert_eq!(
            v.host().native("IsItemVisible", &[item]).unwrap(),
            Value::Bool(shown)
        );
    }
    v.call("showAll", &[]).unwrap();
    assert!(
        v.host()
            .executed_items()
            .all(|(_, i)| i.visible() == Some(true))
    );
    assert_eq!(
        v.host().native("IsItemVisible", &[Value::Null]).unwrap(),
        Value::Bool(false)
    );
    assert!(
        v.host()
            .native("SetItemVisible", &[Value::Null, Value::Int(0)])
            .is_err()
    );
}
