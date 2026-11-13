//! Checked localization native admission over selected UI source banks.
use super::{Error, Failure, ObjectHost};
use crate::script_localization::Bank;
use pcx_script::Value;
const STRING: &str = "GetLocalizedString";
const HOTKEY: &str = "GetLocalizedHotkey";
impl ObjectHost {
    pub fn with_localized_queries(mut self, bank: Bank) -> Result<Self, Error> {
        if self.localized_queries.is_some() {
            return Err(self.inner.error(
                STRING,
                Failure::Presentation("localized queries already admitted"),
            ));
        }
        let source = &self
            .inner
            .program
            .natives()
            .get("CreateTimer")
            .expect("checked host")
            .source;
        for (name, result) in [(STRING, "string"), (HOTKEY, "integer")] {
            if self.inner.program.natives().get(name).is_none()
                && self.inner.program.natives().ambiguous(name).is_none()
            {
                continue;
            }
            Self::contract_in(&self.inner, name, &["string"], result, false, None, false)?;
            let native = self
                .inner
                .program
                .natives()
                .get(name)
                .expect("checked localization contract");
            if &native.source != source {
                return Err(self.inner.error(name, Failure::Source));
            }
        }
        self.localized_queries = Some(bank);
        Ok(self)
    }
    pub(super) fn localized_query_native(
        &self,
        name: &str,
        args: &[Value],
        strings: Option<&pcx_script::text::NativeStringLookup<'_>>,
    ) -> Option<Result<Value, String>> {
        if !matches!(name, STRING | HOTKEY) {
            return None;
        }
        let bank = self.localized_queries.as_ref()?;
        self.inner.program.natives().get(name)?;
        Some(match args {
            [Value::Str(key)] => {
                let key = pcx_script::text::native_c_bytes(key.as_bytes());
                if name == STRING {
                    if key.is_empty() {
                        return Some(Ok(Value::Null));
                    }
                    pcx_script::text::native_map_bytes(key, strings)
                        .and_then(|key| bank.localized_bytes_after_map_lookup(&key))
                        .map(|v| v.map_or(Value::Null, Value::Str))
                } else {
                    bank.localized_hotkey(key).map(Value::Int)
                }
            }
            [Value::Null] => Ok(if name == STRING {
                Value::Null
            } else {
                Value::Int(0)
            }),
            _ => Err(self.inner.error(name, Failure::Signature).to_string()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        script_localization::tests::{bank, origin},
        scripts::{MapScripts, ScriptAsset},
    };
    use pcx_script::Host;
    use std::sync::Arc;
    const BASE: &str = "type agent extends handle\ntype timer extends agent\nnative CreateTimer takes nothing returns timer\nnative DestroyTimer takes timer t returns nothing\n";
    const QUERIES: &str = "native GetLocalizedString takes string s returns string\nnative GetLocalizedHotkey takes string s returns integer\n";
    fn scripts(extra: &str, map: &str) -> Arc<MapScripts> {
        Arc::new(
            MapScripts::compile(
                origin(),
                ScriptAsset {
                    label: "fixture:common",
                    bytes: format!("{BASE}{extra}").as_bytes(),
                },
                ScriptAsset {
                    label: "fixture:BJ",
                    bytes: b"// fixture",
                },
                ScriptAsset {
                    label: "fixture:map",
                    bytes: map.as_bytes(),
                },
                Some(ScriptAsset {
                    label: "fixture:WTS",
                    bytes: b"STRING 4\r\n{\r\nMap authored |cffffff00text|r\r\n}\r\nSTRING 5\r\n{\r\ngameover_restart\r\n}\r\nSTRING 6\r\n{\r\nTRIGSTR_4\r\n}\r\n",
                }),
            )
            .unwrap(),
        )
    }
    fn host(scripts: &MapScripts) -> ObjectHost {
        let inner = super::super::super::EnumHost::checked_selected(
            scripts.shared_program(),
            "fixture:map",
            &[],
            super::super::super::Limits {
                max_handles: 32,
                admission_steps: 1000,
                expression_depth: 16,
                expression_work: 1000,
            },
        )
        .unwrap();
        ObjectHost::checked(inner).unwrap()
    }
    #[test]
    fn localized_queries_run_in_owned_vm_and_keep_existing_wts_resolution() {
        let scripts = scripts(
            QUERIES,
            "function label takes nothing returns string\nreturn GetLocalizedString(\"GAMEOVER_RESTART\")\nendfunction\nfunction key takes nothing returns integer\nreturn GetLocalizedHotkey(\"gameover_restart\")\nendfunction\nfunction mapped takes nothing returns string\nreturn GetLocalizedString(\"TRIGSTR_4\")\nendfunction\nfunction dynamic takes string key returns string\nreturn GetLocalizedString(key)\nendfunction\n",
        );
        let h = host(&scripts).with_localized_queries(bank()).unwrap();
        let mut vm = scripts.owned_vm(h);
        vm.init_globals().unwrap();
        assert_eq!(
            vm.call("label", &[]).unwrap(),
            Value::Str("|cffffffffR|restart".into())
        );
        assert_eq!(vm.call("key", &[]).unwrap(), Value::Int(i32::from(b'R')));
        assert_eq!(
            vm.call("mapped", &[]).unwrap(),
            Value::Str("Map authored |cffffff00text|r".into())
        );
        assert_eq!(
            vm.call("dynamic", &[Value::Str("MISSING".into())]).unwrap(),
            Value::Str("MISSING".into())
        );
        assert_eq!(
            vm.call("dynamic", &[Value::Str("TRIGSTR_4".into())])
                .unwrap(),
            Value::Str("Map authored |cffffff00text|r".into())
        );
    }
    #[test]
    fn localized_byte_results_run_through_vm_without_lossy_text_or_wts_fallback() {
        let s = scripts(
            &format!("{QUERIES}native StringLength takes string s returns integer\n"),
            "function lookup takes string key returns string\nreturn GetLocalizedString(key)\nendfunction\nfunction size takes string key returns integer\nreturn StringLength(GetLocalizedString(key))\nendfunction\n",
        );
        let h = host(&s)
            .with_localized_queries(crate::script_localization::tests::byte_bound_bank())
            .unwrap();
        let mut vm = s.owned_vm(h);
        vm.init_globals().unwrap();
        assert_eq!(
            vm.call("lookup", &[Value::Str("UTF".into())]).unwrap(),
            Value::Str(vec![0xc3].into())
        );
        assert_eq!(
            vm.call("size", &[Value::Str("UTF".into())]).unwrap(),
            Value::Int(1)
        );
        let raw = Value::Str(vec![0xff, 0xc3].into());
        assert_eq!(vm.call("lookup", std::slice::from_ref(&raw)).unwrap(), raw);
        assert_eq!(
            vm.call("lookup", &[Value::Str(vec![0xff, 0, b'x'].into())])
                .unwrap(),
            Value::Str(vec![0xff].into())
        );
        assert_eq!(
            vm.call("lookup", &[Value::Str("TRIGSTR_5".into())])
                .unwrap(),
            Value::Str("gameover_restart".into())
        );
    }
    #[test]
    fn localized_queries_refuse_repeated_and_ambiguous_admission() {
        let s = scripts(QUERIES, "// fixture");
        let h = host(&s).with_localized_queries(bank()).unwrap();
        assert!(
            h.with_localized_queries(bank())
                .err()
                .unwrap()
                .to_string()
                .contains("already admitted")
        );
        let s = MapScripts::compile(
            origin(),
            ScriptAsset {
                label: "fixture:common",
                bytes: format!("{BASE}{QUERIES}").as_bytes(),
            },
            ScriptAsset {
                label: "fixture:conflict",
                bytes: b"native GetLocalizedString takes string s returns integer\n",
            },
            ScriptAsset {
                label: "fixture:map",
                bytes: b"// fixture",
            },
            None,
        )
        .unwrap();
        assert!(s.program().natives().ambiguous(STRING).is_some());
        assert!(host(&s).with_localized_queries(bank()).is_err());
    }
    #[test]
    fn localized_queries_check_declarations_admission_and_arguments() {
        for decl in [
            "native GetLocalizedString takes nothing returns string\n",
            "constant native GetLocalizedString takes string s returns string\n",
            "native GetLocalizedHotkey takes string s returns string\n",
        ] {
            let s = scripts(decl, "// fixture");
            let error = host(&s)
                .with_localized_queries(bank())
                .err()
                .unwrap()
                .to_string();
            assert!(
                error.contains("fixture:common") && error.contains("native/type declaration"),
                "{error}"
            );
        }
        let foreign = MapScripts::compile(
            origin(),
            ScriptAsset {
                label: "fixture:common",
                bytes: BASE.as_bytes(),
            },
            ScriptAsset {
                label: "fixture:foreign",
                bytes: QUERIES.as_bytes(),
            },
            ScriptAsset {
                label: "fixture:map",
                bytes: b"// fixture",
            },
            None,
        )
        .unwrap();
        let error = host(&foreign)
            .with_localized_queries(bank())
            .err()
            .unwrap()
            .to_string();
        assert!(
            error.contains("fixture:foreign") && error.contains("declaration sources"),
            "{error}"
        );
        let s = scripts(QUERIES, "// fixture");
        let mut h = host(&s);
        assert!(
            h.native(STRING, &[Value::Str("x".into())])
                .unwrap_err()
                .contains("unsupported")
        );
        let mut h = h.with_localized_queries(bank()).unwrap();
        assert!(
            h.native(STRING, &[Value::Int(4)])
                .unwrap_err()
                .contains("fixture:common")
        );
        assert_eq!(h.native(STRING, &[Value::Null]).unwrap(), Value::Null);
        assert_eq!(h.native(HOTKEY, &[Value::Null]).unwrap(), Value::Int(0));
        assert_eq!(
            h.native(HOTKEY, &[Value::Str("missing".into())]).unwrap(),
            Value::Int(17)
        );
        let s = scripts("", "// fixture");
        let mut h = host(&s).with_localized_queries(bank()).unwrap();
        assert!(h.native(STRING, &[Value::Str("x".into())]).is_err());
    }
    #[test]
    fn native_string_localization_keeps_hotkey_raw_and_dictionary_lookup_separate() {
        let scripts = scripts(
            QUERIES,
            "function raw takes nothing returns integer\nreturn GetLocalizedHotkey(\"TRIGSTR_5\")\nendfunction\nfunction nested takes nothing returns string\nreturn GetLocalizedString(\"TRIGSTR_6\")\nendfunction\n",
        );
        let mut owned = std::sync::Arc::clone(&scripts)
            .owned_vm(host(&scripts).with_localized_queries(bank()).unwrap());
        let mut borrowed = scripts.vm(host(&scripts).with_localized_queries(bank()).unwrap());
        assert_eq!(owned.call("raw", &[]).unwrap(), Value::Int(17));
        assert_eq!(borrowed.call("raw", &[]).unwrap(), Value::Int(17));
        assert_eq!(
            owned.call("nested", &[]).unwrap(),
            Value::Str("TRIGSTR_4".into())
        );
        assert_eq!(
            borrowed.call("nested", &[]).unwrap(),
            Value::Str("TRIGSTR_4".into())
        );
    }
}
