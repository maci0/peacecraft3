//! Caller-qualified current controllers, separate from executed setup assignments.
use super::{Error, Failure, ObjectHost, ParsedMapContext};
use pcx_script::HandleId;
use std::sync::Arc;
pub(super) struct Controllers {
    current: Option<Vec<(i32, HandleId)>>,
}
impl Controllers {
    pub(super) fn get(&self, number: i32) -> Result<HandleId, Failure> {
        self.current
            .as_ref()
            .ok_or(Failure::PlayerState("controller input is invalidated"))?
            .iter()
            .find(|(n, _)| *n == number)
            .map(|(_, control)| *control)
            .ok_or(Failure::PlayerState("current controller is unset"))
    }
}
impl ObjectHost {
    /// Replace explicit `(source slot number, mapcontrol)` startup input.
    ///
    /// Entries may precede lazy Player creation within the admitted source domain.
    /// They do not create players or overwrite executed setup. Missing entries
    /// refuse, including on an explicitly empty snapshot. Exact context identity
    /// does not establish runtime provenance. The caller must refresh/invalidate
    /// external changes. Accepted controller writes conservatively retire this
    /// input; invalidation does not silently fall back to prior setup assignments.
    /// Without opt-in, the existing setup getter behavior remains unchanged.
    pub fn admit_startup_controllers(
        &mut self,
        context: &Arc<ParsedMapContext>,
        entries: &[(i32, HandleId)],
    ) -> Result<(), Error> {
        let name = "GetPlayerController";
        let fail = |reason| self.inner.error(name, reason);
        if self
            .player_context()
            .is_none_or(|selected| !Arc::ptr_eq(selected, context))
        {
            return Err(fail(Failure::PlayerState(
                "controller map context is not admitted",
            )));
        }
        let players = self.players.as_ref().expect("checked player context");
        if entries.len() > players.slots {
            return Err(fail(Failure::Budget("current controllers")));
        }
        let source = &self
            .inner
            .program
            .natives()
            .get("Player")
            .ok_or_else(|| fail(Failure::Signature))?
            .source;
        if &self
            .inner
            .program
            .natives()
            .get("CreateTimer")
            .ok_or_else(|| fail(Failure::Signature))?
            .source
            != source
        {
            return Err(fail(Failure::Source));
        }
        for (native, params, result, constant) in [
            (name, &["player"][..], "mapcontrol", false),
            (
                "SetPlayerController",
                &["player", "mapcontrol"][..],
                "nothing",
                false,
            ),
            ("ConvertMapControl", &["integer"][..], "mapcontrol", true),
        ] {
            Self::contract(&self.inner, native, params, result, constant, None)?;
            if &self
                .inner
                .program
                .natives()
                .get(native)
                .ok_or_else(|| fail(Failure::Signature))?
                .source
                != source
            {
                return Err(self.inner.error(native, Failure::Source));
            }
        }
        let conversion = self
            .inner
            .conversions
            .get("ConvertMapControl")
            .ok_or_else(|| fail(Failure::Signature))?;
        if &conversion.source != source {
            return Err(fail(Failure::Source));
        }
        if conversion.kind != "mapcontrol" {
            return Err(fail(Failure::Signature));
        }
        let mut kind = "mapcontrol";
        for _ in 0..=self.inner.program.types().count() {
            if kind == "handle" {
                break;
            }
            let ty = self
                .inner
                .program
                .types()
                .find(|ty| ty.name == kind)
                .ok_or_else(|| fail(Failure::Signature))?;
            if &ty.source != source {
                return Err(fail(Failure::Source));
            }
            kind = &ty.parent;
        }
        if kind != "handle" {
            return Err(fail(Failure::Signature));
        }
        for (at, (number, control)) in entries.iter().enumerate() {
            if usize::try_from(*number)
                .ok()
                .is_none_or(|n| n >= players.slots)
            {
                return Err(fail(Failure::OutsideDomain(*number)));
            }
            if entries[..at].iter().any(|(prior, _)| prior == number) {
                return Err(fail(Failure::PlayerState(
                    "duplicate current controller slot",
                )));
            }
            let enumerand = self
                .inner
                .symbolic_value(*control, "mapcontrol")
                .map_err(|e| fail(e.failure))?;
            if !conversion.domain.contains(&enumerand) {
                return Err(fail(Failure::OutsideDomain(enumerand)));
            }
        }
        let mut copy = Vec::new();
        copy.try_reserve_exact(entries.len())
            .map_err(|_| fail(Failure::Capacity))?;
        copy.extend_from_slice(entries);
        self.players
            .as_mut()
            .expect("checked player context")
            .controllers = Some(Controllers {
            current: Some(copy),
        });
        Ok(())
    }
    /// Retire current input without erasing executed setup or reverting getter policy.
    pub fn invalidate_startup_controllers(&mut self) {
        if let Some(input) = self.players.as_mut().and_then(|p| p.controllers.as_mut()) {
            input.current = None;
        }
    }
}
