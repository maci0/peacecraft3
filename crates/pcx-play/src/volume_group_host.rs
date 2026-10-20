//! Executed volume-group scales for the audio consumer. Unset groups keep the
//! user's mixer level; no gain law is applied here.
use super::{Entry, Error, Failure, ObjectHost};
use pcx_script::Value;
use std::collections::BTreeMap;

const SET: &str = "VolumeGroupSetVolume";
const RESET: &str = "VolumeGroupReset";

impl ObjectHost {
    /// Admit the declared pair; both must share `CreateSound`'s source.
    pub fn with_volume_groups(mut self) -> Result<Self, Error> {
        let natives = self.inner.program.natives();
        let source = natives.get("CreateSound").map(|n| n.source.clone());
        for (name, args) in [(SET, &["volumegroup", "real"][..]), (RESET, &[][..])] {
            let Some(native) = self.inner.program.natives().get(name) else {
                continue;
            };
            Self::contract_in(&self.inner, name, args, "nothing", false, None, false)?;
            if Some(&native.source) != source.as_ref() {
                return Err(self.inner.error(name, Failure::Source));
            }
        }
        self.volume_groups = Some(BTreeMap::new());
        Ok(self)
    }

    /// Scale per `volumegroup` enumerand, as last set since a reset.
    pub fn volume_group_scales(&self) -> Option<&BTreeMap<i32, f32>> {
        self.volume_groups.as_ref()
    }

    pub(super) fn volume_group_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        self.volume_groups.as_ref()?;
        if !matches!(name, SET | RESET) {
            return None;
        }
        self.inner.program.natives().get(name)?;
        Some((|| {
            match (name, args) {
                (RESET, []) => self.volume_groups.as_mut().expect("admitted").clear(),
                (SET, [Value::Handle(group), scale]) => {
                    let Entry::Symbolic { kind, enumerand } = self.inner.entry(*group)? else {
                        return Err(self.inner.error(name, Failure::WrongFamily));
                    };
                    if kind != "volumegroup" {
                        return Err(self.inner.error(name, Failure::WrongFamily));
                    }
                    let enumerand = *enumerand;
                    let scale = scale
                        .as_real()
                        .filter(|v| v.is_finite())
                        .ok_or_else(|| self.inner.error(name, Failure::Signature))?;
                    self.volume_groups
                        .as_mut()
                        .expect("admitted")
                        .insert(enumerand, scale);
                }
                _ => return Err(self.inner.error(name, Failure::Signature)),
            }
            Ok(Value::Null)
        })())
    }
}
