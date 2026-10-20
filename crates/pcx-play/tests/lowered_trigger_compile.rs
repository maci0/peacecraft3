//! The JASS that `pcx_trig::lower` writes must parse and resolve through the
//! script compiler. The prelude is synthetic: it declares only the natives the
//! fixture calls, with the arities the fixture's TriggerData gives them.

use pcx_script::Prelude;

const PRELUDE: &str = "type trigger extends handle
type triggeraction extends handle
type triggercondition extends handle
type event extends handle
type boolexpr extends handle
type player extends handle
type unit extends handle
native CreateTrigger takes nothing returns trigger
native DisableTrigger takes trigger t returns nothing
native TriggerAddAction takes trigger t, code c returns triggeraction
native TriggerAddCondition takes trigger t, boolexpr c returns triggercondition
native Condition takes code c returns boolexpr
native TriggerRegisterTimerEventSingle takes trigger t, real r returns event
native DisplayTextToPlayer takes player p, string s returns nothing
native Player takes integer n returns player
native RunLater takes integer n, code c returns nothing
native Gate takes boolexpr b returns nothing
native CommentString takes string s returns nothing
native DoNothing takes nothing returns nothing
native ConditionalTriggerExecute takes trigger t returns nothing
";

const LOWERED: &str = "globals
    integer udg_Count
    boolean udg_Flag
    unit udg_Who
    trigger gg_trg_Hello_World
endglobals

function InitGlobals takes nothing returns nothing
    set udg_Count = 5
    set udg_Flag = false
endfunction

function Trig_Hello_World_Func001A takes nothing returns nothing
    call DoNothing()
endfunction

function Trig_Hello_World_Func002C takes nothing returns boolean
    return (udg_Count == 1)
endfunction

function Trig_Hello_World_Actions takes nothing returns nothing
    set udg_Count = (udg_Count + 1)
    call DisplayTextToPlayer(Player(0), \"hi there\")
    call RunLater(1, function Trig_Hello_World_Func001A)
    call Gate(Condition(function Trig_Hello_World_Func002C))
    // note
endfunction

function Trig_Hello_World_Conditions takes nothing returns boolean
    if (not (udg_Count == 1)) then
        return false
    endif
    return true
endfunction

function InitTrig_Hello_World takes nothing returns nothing
    set gg_trg_Hello_World = CreateTrigger()
    call TriggerRegisterTimerEventSingle(gg_trg_Hello_World, 0.5)
    call TriggerAddCondition(gg_trg_Hello_World, Condition(function Trig_Hello_World_Conditions))
    call TriggerAddAction(gg_trg_Hello_World, function Trig_Hello_World_Actions)
endfunction

function InitCustomTriggers takes nothing returns nothing
    call InitTrig_Hello_World()
endfunction

function RunInitializationTriggers takes nothing returns nothing
endfunction
";

fn resolve(map: &str) -> Result<(), String> {
    let prelude = pcx_script::parse_script(PRELUDE).unwrap();
    let script = pcx_script::parse_script(map).map_err(|e| e.to_string())?;
    pcx_script::resolve_with(&script, &[Prelude::new("synthetic:prelude", &prelude)])
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[test]
fn lowered_fragment_parses_and_resolves() {
    resolve(LOWERED).unwrap();
}

#[test]
fn a_call_to_a_missing_native_is_refused_so_the_check_is_not_vacuous() {
    let broken = LOWERED.replace("DisplayTextToPlayer(", "NoSuchNative(");
    assert!(resolve(&broken).is_err());
}
