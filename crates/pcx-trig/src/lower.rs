//! Editor trigger graph (`war3map.wtg` version 4) to JASS source.
//!
//! The output follows the World Editor's generated layout (differential against the
//! `war3map.j` of the shipped maps, see `docs/re-notes/wtg-lowering.md`): a `globals` block,
//! `InitGlobals`, `InitCustomTriggers`, `RunInitializationTriggers`, and per
//! trigger an actions function, a conditions function when conditions exist,
//! an `InitTrig_` function and one wrapper per code or boolexpr parameter.
//! Function names come from `UI\TriggerData.txt` (`ScriptName` alias when
//! authored), preset code text from `[TriggerParams]`, and variable types from
//! `[TriggerTypes]`.
//!
//! Names starting `gg_` (trigger handles, map object globals) are passed
//! through. The map's own script declares object globals, so a lowered trigger
//! links only with them. User variables are `udg_` globals.
//!
//! Disabled triggers keep their `gg_trg_` declaration but emit no functions.
//! Refused, failing the whole map with [`LowerError::Unsupported`]: constructs the
//! editor expands into multi-action forms not yet lowered, custom-text triggers,
//! run-on-initialization triggers, and trigger names that collide after sanitizing.

use crate::catalog::{Catalog, Definition, Kind};
use crate::{Eca, EcaKind, Param, ParamKind, Sub, SubKind, Trigger, Wtg};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

const VARIABLE_PREFIX: &str = "udg_";
const TRIGGER_PREFIX: &str = "gg_trg_";
const INDENT: &str = "    ";
/// The event the editor realizes by running the trigger from `RunInitializationTriggers`.
const MAP_INITIALIZATION_EVENT: &str = "MapInitializationEvent";

/// Functions the World Editor expands into control flow or multi-action forms that
/// are not lowered yet. A map using one is refused. `ForLoopA`, `ForLoopB`,
/// `IfThenElse` and `ReturnAction` are lowered as statements in `statement`.
const EDITOR_EXPANDED: [&str; 10] = [
    "AndMultiple",
    "OrMultiple",
    "IfThenElseMultiple",
    "ForLoopAMultiple",
    "ForLoopBMultiple",
    "ForLoopVarMultiple",
    "ForLoopVar",
    "ForForceMultiple",
    "ForGroupMultiple",
    "CustomScriptCode",
];

/// Lowers `wtg` to JASS, using `catalog` for names, preset code text and variable types.
pub fn lower(wtg: &Wtg<'_>, catalog: &Catalog) -> Result<String, LowerError> {
    let mut globals = String::new();
    let mut init_globals = String::new();
    let mut variables = BTreeMap::new();
    for variable in wtg.variables() {
        let name = identifier(&variable.name, "variable", "variable name")?;
        let context = format!("variable {name}");
        let type_name = text(&variable.type_name, &context, "variable type")?;
        let jass_type = catalog.jass_type(type_name).ok_or_else(|| {
            malformed(
                &context,
                format!("type {type_name:?} has no [TriggerTypes] entry"),
            )
        })?;
        let array = if variable.is_array != 0 { "array " } else { "" };
        let _ = writeln!(
            globals,
            "{INDENT}{jass_type} {array}{VARIABLE_PREFIX}{name}"
        );
        let initial = match (variable.is_initialized != 0, variable.is_array != 0) {
            (true, true) => return Err(unsupported(&context, "initialized array variable")),
            (true, false) => {
                let value = text(&variable.initial_value, &context, "initial value")?;
                if value.is_empty() {
                    return Err(malformed(
                        &context,
                        "initialized variable without a value".into(),
                    ));
                }
                Some(initial_value(catalog, jass_type, value, &context)?)
            }
            (false, false) => match catalog.type_default(type_name) {
                Some(text) => Some(text.to_string()),
                // The editor writes an empty string into an uninitialized string global;
                // TriggerTypeDefaults has no string entry (measured on CreditMusic).
                None if jass_type == "string" => Some("\"\"".into()),
                None => None,
            },
            (false, true) => None,
        };
        if let Some(value) = initial {
            let _ = writeln!(
                init_globals,
                "{INDENT}set {VARIABLE_PREFIX}{name} = {value}"
            );
        }
        variables.insert(name.to_string(), type_name.to_string());
    }

    // Every trigger is declared, disabled ones included, as the editor does.
    let mut identifiers = BTreeSet::new();
    let mut trigger_globals = String::new();
    for trigger in wtg.triggers() {
        let id = trigger_id(text(&trigger.name, "trigger", "trigger name")?);
        if !identifiers.insert(id.clone()) {
            return Err(unsupported(
                &id,
                "trigger names that collide after sanitizing",
            ));
        }
        let _ = writeln!(trigger_globals, "{INDENT}trigger {TRIGGER_PREFIX}{id}");
    }
    globals.push_str(&trigger_globals);

    let mut functions = String::new();
    let mut init_calls = String::new();
    let mut run_on_init = String::new();
    for trigger in wtg.triggers().iter().filter(|t| t.is_enabled != 0) {
        let mut lowerer = Lowerer {
            catalog,
            variables: &variables,
            trigger: String::new(),
            id: String::new(),
            wrappers: String::new(),
            wrapper_count: 0,
        };
        let lowered = lowerer.trigger(trigger)?;
        functions.push_str(&lowerer.wrappers);
        functions.push_str(&lowered.functions);
        let _ = writeln!(init_calls, "{INDENT}call InitTrig_{}()", lowered.id);
        // An initially-off map-initialization trigger is not run from the list (measured on
        // NightElf05 The Dragons, the only shipped case).
        if lowered.map_initialization && trigger.is_initially_off == 0 {
            let _ = writeln!(
                run_on_init,
                "{INDENT}call ConditionalTriggerExecute({TRIGGER_PREFIX}{})",
                lowered.id
            );
        }
    }

    let mut out = String::new();
    if !globals.is_empty() {
        let _ = write!(out, "globals\n{globals}endglobals\n\n");
    }
    let _ = write!(
        out,
        "function InitGlobals takes nothing returns nothing\n{init_globals}endfunction\n\n"
    );
    out.push_str(&functions);
    let _ = write!(
        out,
        "function InitCustomTriggers takes nothing returns nothing\n{init_calls}endfunction\n\n"
    );
    let _ = write!(
        out,
        "function RunInitializationTriggers takes nothing returns nothing\n{run_on_init}endfunction\n"
    );
    Ok(out)
}

/// A refusal naming the trigger or variable and the construct not lowered.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LowerError {
    #[error("{context}: {construct} is not lowered")]
    Unsupported { context: String, construct: String },
    #[error("{context}: {kind:?} {name:?} has no TriggerData definition")]
    UnknownDefinition {
        context: String,
        kind: Kind,
        name: String,
    },
    #[error("{context}: preset {key:?} has no [TriggerParams] code text")]
    UnknownPreset { context: String, key: String },
    #[error("{context}: {reason}")]
    Malformed { context: String, reason: String },
}

fn unsupported(context: &str, construct: &str) -> LowerError {
    LowerError::Unsupported {
        context: context.into(),
        construct: construct.into(),
    }
}

fn malformed(context: &str, reason: String) -> LowerError {
    LowerError::Malformed {
        context: context.into(),
        reason,
    }
}

fn text<'a>(bytes: &'a [u8], context: &str, what: &str) -> Result<&'a str, LowerError> {
    std::str::from_utf8(bytes).map_err(|_| malformed(context, format!("{what} is not UTF-8")))
}

/// A name the generated JASS can use as an identifier. GUI names are not
/// sanitized, so an unusable one is refused rather than rewritten.
fn identifier<'a>(bytes: &'a [u8], context: &str, what: &str) -> Result<&'a str, LowerError> {
    let name = text(bytes, context, what)?;
    let valid = name
        .bytes()
        .enumerate()
        .all(|(i, b)| b == b'_' || b.is_ascii_alphabetic() || (i > 0 && b.is_ascii_digit()));
    if valid && !name.is_empty() {
        Ok(name)
    } else {
        Err(malformed(
            context,
            format!("{what} {name:?} is not a JASS identifier"),
        ))
    }
}

/// The editor's generated-name stem: every byte outside `[A-Za-z0-9]` becomes `_`.
fn trigger_id(name: &str) -> String {
    // Trailing whitespace is dropped; leading whitespace becomes `_`.
    name.trim_end()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

/// An initial value is a `[TriggerParams]` preset key (`UnitNull`, `Player03`), or a
/// literal of the variable's base type.
fn initial_value(
    catalog: &Catalog,
    jass_type: &str,
    value: &str,
    context: &str,
) -> Result<String, LowerError> {
    if let Some(code) = catalog.preset_code(value) {
        return Ok(
            match code.strip_prefix('`').and_then(|c| c.strip_suffix('`')) {
                Some(literal) => format!("\"{literal}\""),
                None => code.trim_matches('"').to_string(),
            },
        );
    }
    match jass_type {
        "integer" | "real" => number_text(value)
            .ok_or_else(|| malformed(context, format!("numeric initial value {value:?}"))),
        "string" => string_text(value, context),
        _ if value.is_empty() => Err(malformed(
            context,
            "initialized variable without a value".into(),
        )),
        _ => Ok(value.to_string()),
    }
}

/// A numeric literal: a decimal, or an object code written bare or quoted.
fn number_text(literal: &str) -> Option<String> {
    if literal.parse::<f64>().is_ok() {
        return Some(literal.to_string());
    }
    let quoted_code = literal.len() == 6 && literal.starts_with('\'') && literal.ends_with('\'');
    if quoted_code {
        return Some(literal.to_string());
    }
    if literal.len() == 4 && literal.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return Some(format!("'{literal}'"));
    }
    None
}

/// A string literal. A quote or line break cannot be written in the file's JASS form.
fn string_text(value: &str, context: &str) -> Result<String, LowerError> {
    if value.contains(['"', '\n', '\r']) {
        return Err(unsupported(
            context,
            "string literal with a quote or line break",
        ));
    }
    Ok(format!("\"{value}\""))
}

/// The functions one trigger lowers to, minus wrappers, which the caller emits first.
struct LoweredTrigger {
    id: String,
    functions: String,
    map_initialization: bool,
}

struct Lowerer<'c> {
    catalog: &'c Catalog,
    variables: &'c BTreeMap<String, String>,
    /// Context for error messages: the trigger name.
    trigger: String,
    /// Generated-name stem of the current trigger.
    id: String,
    /// Wrapper functions for code and boolexpr parameters of the current trigger.
    wrappers: String,
    wrapper_count: usize,
}

impl<'c> Lowerer<'c> {
    fn trigger(&mut self, trigger: &Trigger) -> Result<LoweredTrigger, LowerError> {
        self.trigger = text(&trigger.name, "trigger", "trigger name")?.to_string();
        self.id = trigger_id(&self.trigger);
        let id = self.id.clone();
        if trigger.is_custom != 0 {
            return Err(unsupported(&self.trigger, "custom text trigger"));
        }
        if trigger.run_on_init != 0 {
            return Err(unsupported(&self.trigger, "run-on-initialization trigger"));
        }
        let mut events = Vec::new();
        let mut conditions = Vec::new();
        let mut actions = Vec::new();
        // Disabled ECAs are omitted from the editor output (measured on actions).
        for eca in trigger.ecas.iter().filter(|e| e.is_enabled != 0) {
            match eca.kind {
                EcaKind::Event => events.push(eca),
                EcaKind::Condition => conditions.push(eca),
                EcaKind::Action => actions.push(eca),
            }
        }
        let map_initialization = events
            .iter()
            .any(|eca| eca.name.as_slice() == MAP_INITIALIZATION_EVENT.as_bytes());
        if map_initialization && events.len() > 1 {
            return Err(unsupported(
                &self.trigger,
                "map initialization combined with other events",
            ));
        }
        events.retain(|eca| eca.name.as_slice() != MAP_INITIALIZATION_EVENT.as_bytes());

        let trigger_global = format!("{TRIGGER_PREFIX}{id}");
        let mut init = format!("function InitTrig_{id} takes nothing returns nothing\n");
        let _ = writeln!(init, "{INDENT}set {trigger_global} = CreateTrigger()");
        if trigger.is_initially_off != 0 {
            let _ = writeln!(init, "{INDENT}call DisableTrigger({trigger_global})");
        }
        for eca in &events {
            let call = self.call(eca, Kind::Event, Some(&trigger_global))?;
            let _ = writeln!(init, "{INDENT}call {call}");
        }
        if !conditions.is_empty() {
            let _ = writeln!(
                init,
                "{INDENT}call TriggerAddCondition({trigger_global}, Condition(function Trig_{id}_Conditions))"
            );
        }
        let _ = writeln!(
            init,
            "{INDENT}call TriggerAddAction({trigger_global}, function Trig_{id}_Actions)"
        );
        init.push_str("endfunction\n\n");

        let mut functions = String::new();
        let mut body = String::new();
        for eca in &actions {
            let name = self.name(&eca.name)?.to_string();
            for line in self.statement(&name, &eca.params)?.lines() {
                let _ = writeln!(body, "{INDENT}{line}");
            }
        }
        let _ = write!(
            functions,
            "function Trig_{id}_Actions takes nothing returns nothing\n{body}endfunction\n\n"
        );
        if !conditions.is_empty() {
            let mut checks = String::new();
            for eca in &conditions {
                let expression = self.call(eca, Kind::Condition, None)?;
                let _ = write!(
                    checks,
                    "{INDENT}if (not {expression}) then\n{INDENT}{INDENT}return false\n{INDENT}endif\n"
                );
            }
            let _ = write!(
                functions,
                "function Trig_{id}_Conditions takes nothing returns boolean\n{checks}{INDENT}return true\nendfunction\n\n"
            );
        }
        functions.push_str(&init);
        Ok(LoweredTrigger {
            id,
            functions,
            map_initialization,
        })
    }

    /// The statement for an action name with its parameters. Wrapped actions and
    /// branches take the same route, so no statement form escapes this dispatch.
    fn statement(&mut self, name: &str, params: &[Param]) -> Result<String, LowerError> {
        match name {
            "IfThenElse" => self.if_then_else(params),
            "ForLoopA" | "ForLoopB" => self.for_loop(name, params),
            "ReturnAction" => {
                self.definition(name, Kind::Action)?;
                Ok("return".into())
            }
            "SetVariable" => {
                let [target, value] = params else {
                    return Err(malformed(&self.trigger, "SetVariable arity".into()));
                };
                if target.kind != ParamKind::Variable {
                    return Err(unsupported(
                        &self.trigger,
                        "SetVariable target that is not a variable",
                    ));
                }
                let target_type = self.variable_type(target)?.to_string();
                let lhs = self.expression(target, "")?;
                let rhs = self.expression(value, &target_type)?;
                Ok(format!("set {lhs} = {rhs}"))
            }
            _ => Ok(format!(
                "call {}",
                self.emit_call(name, params, Kind::Action, None)?
            )),
        }
    }

    /// A call expression for an ECA. `leading` is the first argument the editor
    /// supplies itself (the trigger handle, for an event).
    fn call(&mut self, eca: &Eca, kind: Kind, leading: Option<&str>) -> Result<String, LowerError> {
        let name = self.name(&eca.name)?.to_string();
        self.emit_call(&name, &eca.params, kind, leading)
    }

    fn emit_call(
        &mut self,
        name: &str,
        params: &[Param],
        kind: Kind,
        leading: Option<&str>,
    ) -> Result<String, LowerError> {
        let definition = self.definition(name, kind)?;
        if EDITOR_EXPANDED.contains(&name) {
            return Err(unsupported(
                &self.trigger,
                &format!("editor-expanded {name}"),
            ));
        }
        let is_operator = definition.parameter_types.len() == 3
            && matches!(
                definition.parameter_types[1].as_str(),
                "ArithmeticOperator" | "ComparisonOperator" | "EqualNotEqualOperator"
            );
        if is_operator {
            // The middle parameter is the operator's code text, written infix.
            let [left, op, right] = params else {
                return Err(malformed(&self.trigger, format!("{name} operator arity")));
            };
            let types = &definition.parameter_types;
            let left = self.expression(left, &types[0])?;
            let op = self.expression(op, "")?;
            let right = self.expression(right, &types[2])?;
            return Ok(format!("({left} {op} {right})"));
        }
        let callee = definition
            .script_name
            .as_deref()
            .unwrap_or(&definition.name);
        let mut arguments: Vec<String> = leading.map(String::from).into_iter().collect();
        for (index, param) in params.iter().enumerate() {
            let expected = definition
                .parameter_types
                .get(index)
                .map_or("", String::as_str);
            arguments.push(self.expression(param, expected)?);
        }
        Ok(format!("{callee}({})", arguments.join(", ")))
    }

    fn name<'a>(&self, bytes: &'a [u8]) -> Result<&'a str, LowerError> {
        text(bytes, &self.trigger, "ECA name")
    }

    /// The definition a name has in the table its record kind names.
    fn definition(&self, name: &str, kind: Kind) -> Result<&'c Definition, LowerError> {
        let catalog: &'c Catalog = self.catalog;
        catalog
            .get(kind, name)
            .ok_or_else(|| LowerError::UnknownDefinition {
                context: self.trigger.clone(),
                kind,
                name: name.into(),
            })
    }

    /// The first table that defines `name`, in the order given.
    fn resolve(&self, name: &str, kinds: [Kind; 2]) -> Result<Kind, LowerError> {
        kinds
            .into_iter()
            .find(|&kind| self.catalog.get(kind, name).is_some())
            .ok_or_else(|| LowerError::UnknownDefinition {
                context: self.trigger.clone(),
                kind: kinds[0],
                name: name.into(),
            })
    }

    /// The TriggerData type name of a declared variable.
    fn variable_type(&self, param: &Param) -> Result<&'c str, LowerError> {
        let name = identifier(&param.value, &self.trigger, "variable reference")?;
        self.variables
            .get(name)
            .map(String::as_str)
            .ok_or_else(|| malformed(&self.trigger, format!("undeclared variable {name:?}")))
    }

    /// Lowers one parameter. `expected` is the TriggerData type the parameter
    /// takes (empty when none applies). The file stores numeric literals as
    /// string-kind values; they are written raw, and any other string is quoted.
    fn expression(&mut self, param: &Param, expected: &str) -> Result<String, LowerError> {
        match param.kind {
            ParamKind::Preset => {
                let key = text(&param.value, &self.trigger, "preset key")?;
                let code =
                    self.catalog
                        .preset_code(key)
                        .ok_or_else(|| LowerError::UnknownPreset {
                            context: self.trigger.clone(),
                            key: key.into(),
                        })?;
                // Backquoted code text is a string literal; double quotes delimit
                // the code text itself (operators such as `"=="`).
                if let Some(literal) = code.strip_prefix('`').and_then(|c| c.strip_suffix('`')) {
                    return self.string_literal(literal);
                }
                Ok(code.trim_matches('"').to_string())
            }
            ParamKind::Variable => {
                let name = identifier(&param.value, &self.trigger, "variable reference")?;
                let base = if name.starts_with("gg_") {
                    name.to_string()
                } else {
                    self.variable_type(param)?;
                    format!("{VARIABLE_PREFIX}{name}")
                };
                if expected.starts_with("VarAsString") {
                    if param.array_index.is_some() {
                        return Err(unsupported(
                            &self.trigger,
                            "array variable named as a string",
                        ));
                    }
                    return Ok(format!("\"{base}\""));
                }
                match &param.array_index {
                    Some(index) => Ok(format!("{base}[{}]", self.expression(index, "integer")?)),
                    None => Ok(base),
                }
            }
            ParamKind::Str => {
                let literal = text(&param.value, &self.trigger, "string")?;
                match self.catalog.jass_type(expected) {
                    Some("integer" | "real") => self.number_literal(literal),
                    Some("boolean") if matches!(literal, "true" | "false") => {
                        Ok(literal.to_string())
                    }
                    Some("boolean") => Err(malformed(
                        &self.trigger,
                        format!("boolean literal {literal:?}"),
                    )),
                    _ => self.string_literal(&literal.replace('\\', "\\\\")),
                }
            }
            ParamKind::Function => {
                if param.array_index.is_some() {
                    return Err(malformed(
                        &self.trigger,
                        "array index on a function parameter".into(),
                    ));
                }
                let sub = param.sub.as_ref().ok_or_else(|| {
                    malformed(
                        &self.trigger,
                        "function parameter without a sub-record".into(),
                    )
                })?;
                self.function_parameter(sub, expected)
            }
        }
    }

    fn function_parameter(&mut self, sub: &Sub, expected: &str) -> Result<String, LowerError> {
        let name = self.name(&sub.name)?.to_string();
        match sub.kind {
            SubKind::NestedCall => {
                let kind = self.resolve(&name, [Kind::Call, Kind::Condition])?;
                self.emit_call(&name, &sub.params, kind, None)
            }
            SubKind::WrappedCondition => {
                let kind = self.resolve(&name, [Kind::Condition, Kind::Call])?;
                let expression = self.emit_call(&name, &sub.params, kind, None)?;
                let function = self.wrapper('C', "boolean", &format!("return {expression}"));
                // A boolcall parameter takes the boolean value; a boolexpr takes a condition handle.
                if expected == "boolcall" {
                    Ok(format!("{function}()"))
                } else {
                    Ok(format!("Condition(function {function})"))
                }
            }
            SubKind::WrappedAction => {
                let statement = self.statement(&name, &sub.params)?;
                let function = self.wrapper('A', "nothing", &statement);
                Ok(format!("function {function}"))
            }
        }
    }

    /// `IfThenElse` with a wrapped condition and one wrapped action per branch. The
    /// condition is a boolean wrapper called from the `if`, as the editor writes it.
    fn if_then_else(&mut self, params: &[Param]) -> Result<String, LowerError> {
        let [condition, then_branch, else_branch] = params else {
            return Err(malformed(&self.trigger, "IfThenElse arity".into()));
        };
        let sub = match condition.sub.as_ref() {
            Some(sub)
                if condition.kind == ParamKind::Function
                    && sub.kind == SubKind::WrappedCondition =>
            {
                sub
            }
            _ => {
                return Err(unsupported(
                    &self.trigger,
                    "IfThenElse condition that is not a wrapped condition",
                ));
            }
        };
        let name = self.name(&sub.name)?.to_string();
        let kind = self.resolve(&name, [Kind::Condition, Kind::Call])?;
        let expression = self.emit_call(&name, &sub.params, kind, None)?;
        let function = self.wrapper('C', "boolean", &format!("return {expression}"));
        let then_statement = self.branch(then_branch)?;
        let else_statement = self.branch(else_branch)?;
        Ok(format!(
            "if ({function}()) then\n{INDENT}{then_statement}\nelse\n{INDENT}{else_statement}\nendif"
        ))
    }

    /// `ForLoopA` or `ForLoopB`: the index and end globals come from Blizzard.j,
    /// and the body statement reads the index through its own nested call.
    fn for_loop(&mut self, name: &str, params: &[Param]) -> Result<String, LowerError> {
        let [start, end, body] = params else {
            return Err(malformed(&self.trigger, format!("{name} arity")));
        };
        let letter = &name["ForLoop".len()..];
        let index = format!("bj_forLoop{letter}Index");
        let end_index = format!("bj_forLoop{letter}IndexEnd");
        let start = self.expression(start, "integer")?;
        let end = self.expression(end, "integer")?;
        let body = self.branch(body)?;
        Ok(format!(
            "set {index} = {start}\nset {end_index} = {end}\nloop\n{INDENT}exitwhen {index} > {end_index}\n{INDENT}{body}\n{INDENT}set {index} = {index} + 1\nendloop"
        ))
    }

    /// A numeric literal: a decimal, or an object code written bare or quoted.
    fn number_literal(&self, literal: &str) -> Result<String, LowerError> {
        number_text(literal)
            .ok_or_else(|| malformed(&self.trigger, format!("numeric literal {literal:?}")))
    }

    /// The single wrapped action an `IfThenElse` branch holds.
    fn branch(&mut self, param: &Param) -> Result<String, LowerError> {
        match param.sub.as_ref() {
            Some(sub)
                if param.kind == ParamKind::Function && sub.kind == SubKind::WrappedAction =>
            {
                let name = self.name(&sub.name)?.to_string();
                self.statement(&name, &sub.params)
            }
            _ => Err(unsupported(
                &self.trigger,
                "code parameter that is not a single wrapped action",
            )),
        }
    }

    /// Declares a wrapper function and returns its name. Its body is queued
    /// ahead of the trigger's own functions, so every wrapper is defined
    /// before the function that refers to it. `suffix` is `C` for a boolexpr
    /// wrapper and `A` for a code wrapper.
    fn wrapper(&mut self, suffix: char, return_type: &str, body: &str) -> String {
        self.wrapper_count += 1;
        let name = format!("Trig_{}_Func{:03}{suffix}", self.id, self.wrapper_count);
        let _ = writeln!(
            self.wrappers,
            "function {name} takes nothing returns {return_type}"
        );
        for line in body.lines() {
            let _ = writeln!(self.wrappers, "{INDENT}{line}");
        }
        self.wrappers.push_str("endfunction\n\n");
        name
    }

    fn string_literal(&self, value: &str) -> Result<String, LowerError> {
        string_text(value, &self.trigger)
    }
}
