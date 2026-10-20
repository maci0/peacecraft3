//! Opt-in queries over selected-client new-session cheat flags; no mutation shortcuts.
use super::{Error, Failure, ObjectHost};
use crate::cheat_rules::Rules;
use pcx_script::Value;
const QUERIES: [&str; 2] = ["IsNoDefeatCheat", "IsNoVictoryCheat"];
impl ObjectHost {
    pub fn with_cheat_queries(mut self, rules: Rules) -> Result<Self, Error> {
        let source = &self
            .inner
            .program
            .natives()
            .get("CreateTimer")
            .expect("checked host")
            .source;
        for name in QUERIES {
            let Some(native) = self.inner.program.natives().get(name) else {
                continue;
            };
            Self::contract_in(&self.inner, name, &[], "boolean", false, None, false)?;
            if &native.source != source {
                return Err(self.inner.error(name, Failure::Source));
            }
        }
        self.cheat_queries = Some(rules);
        Ok(self)
    }
    pub(super) fn cheat_query_native(
        &self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        let rules = self.cheat_queries.as_ref()?;
        if !QUERIES.contains(&name) {
            return None;
        }
        self.inner.program.natives().get(name)?;
        Some(if args.is_empty() {
            Ok(Value::Bool(if name == QUERIES[0] {
                rules.no_defeat()
            } else {
                rules.no_victory()
            }))
        } else {
            Err(self.inner.error(name, Failure::Signature))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pcx_script::{Host, Prelude, Value, Vm};
    use std::sync::Arc;
    const BASE: &str = "type agent extends handle\ntype timer extends agent\nnative CreateTimer takes nothing returns timer\nnative DestroyTimer takes timer t returns nothing\n";
    const QUERIES: &str = "native IsNoDefeatCheat takes nothing returns boolean\nnative IsNoVictoryCheat takes nothing returns boolean\nnative Cheat takes string input returns nothing\n";
    fn program(common: &str, other: &str, map: &str) -> Arc<pcx_script::Program> {
        let common = pcx_script::parse_script(&format!("{BASE}{common}")).unwrap();
        let other = pcx_script::parse_script(other).unwrap();
        let map = pcx_script::parse_script(map).unwrap();
        Arc::new(
            pcx_script::resolve_with(
                &map,
                &[
                    Prelude::new("synthetic:common", &common),
                    Prelude::new("synthetic:other", &other),
                ],
            )
            .unwrap(),
        )
    }
    fn host(program: Arc<pcx_script::Program>) -> ObjectHost {
        let inner = super::super::super::EnumHost::checked_selected(
            program,
            "synthetic:map",
            &[],
            super::super::super::Limits {
                max_handles: 16,
                admission_steps: 1000,
                expression_depth: 16,
                expression_work: 1000,
            },
        )
        .unwrap();
        ObjectHost::checked(inner).unwrap()
    }
    #[test]
    fn cheat_queries_execute_in_vm_and_preserve_normal_defeat() {
        let map = "function allowDefeat takes nothing returns boolean\nreturn not IsNoDefeatCheat()\nendfunction\nfunction victoryBlocked takes nothing returns boolean\nreturn IsNoVictoryCheat()\nendfunction\nfunction mutate takes nothing returns nothing\ncall Cheat(\"synthetic-command\")\nendfunction\n";
        for (flags, defeat, victory) in [
            (0, true, false),
            (2, false, false),
            (8, true, true),
            (10, false, true),
        ] {
            let p = program(QUERIES, "", map);
            let h = host(p.clone())
                .with_cheat_queries(crate::cheat_rules::tests::fixture(flags, 2, 8))
                .unwrap();
            let mut vm = Vm::new(&p, h);
            vm.init_globals().unwrap();
            assert_eq!(vm.call("allowDefeat", &[]).unwrap(), Value::Bool(defeat));
            assert_eq!(
                vm.call("victoryBlocked", &[]).unwrap(),
                Value::Bool(victory)
            );
            let error = vm.call("mutate", &[]).unwrap_err().to_string();
            assert!(
                error.contains("Cheat") && error.contains("unsupported"),
                "{error}"
            );
            assert_eq!(
                vm.host().native("IsNoDefeatCheat", &[]).unwrap(),
                Value::Bool(!defeat)
            );
        }
    }
    #[test]
    fn cheat_queries_refuse_wrong_declarations_sources_and_arguments() {
        for declaration in [
            "native IsNoDefeatCheat takes integer x returns boolean\n",
            "native IsNoDefeatCheat takes nothing returns integer\n",
            "constant native IsNoDefeatCheat takes nothing returns boolean\n",
        ] {
            let p = program(declaration, "", "");
            let error = host(p)
                .with_cheat_queries(crate::cheat_rules::tests::fixture(0, 2, 8))
                .err()
                .unwrap()
                .to_string();
            assert!(
                error.contains("IsNoDefeatCheat")
                    && error.contains("synthetic:common")
                    && error.contains("native/type declaration"),
                "{error}"
            );
        }
        let p = program(
            "",
            "native IsNoDefeatCheat takes nothing returns boolean\n",
            "",
        );
        let error = host(p)
            .with_cheat_queries(crate::cheat_rules::tests::fixture(0, 2, 8))
            .err()
            .unwrap()
            .to_string();
        assert!(
            error.contains("synthetic:other") && error.contains("declaration sources"),
            "{error}"
        );
        let p = program(QUERIES, "", "");
        let mut h = host(p)
            .with_cheat_queries(crate::cheat_rules::tests::fixture(0, 2, 8))
            .unwrap();
        assert!(
            h.native("IsNoDefeatCheat", &[Value::Int(1)])
                .unwrap_err()
                .contains("native/type declaration")
        );
    }
    #[test]
    fn cheat_queries_require_opt_in_and_leave_absent_declarations_unsupported() {
        let p = program(QUERIES, "", "");
        let mut h = host(p);
        assert!(
            h.native("IsNoDefeatCheat", &[])
                .unwrap_err()
                .contains("unsupported")
        );
        let p = program("", "", "");
        let mut h = host(p)
            .with_cheat_queries(crate::cheat_rules::tests::fixture(0, 2, 8))
            .unwrap();
        assert!(h.native("IsNoDefeatCheat", &[]).is_err());
    }
}
