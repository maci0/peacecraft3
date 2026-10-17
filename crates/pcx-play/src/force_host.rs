//! Explicit force memberships on the existing session arena.
use super::super::{Entry, Error, Failure};
use super::ObjectHost;
use pcx_script::{HandleId, Value};
/// Caller limits independent of the shared identity cap.
#[derive(Clone, Copy, Debug)]
pub struct ForceBudget {
    pub max_records: usize,
    /// Aggregate distinct memberships across all retained forces.
    pub max_memberships: usize,
}
pub(super) struct Admission {
    budget: ForceBudget,
    records: usize,
    memberships: usize,
}
impl ObjectHost {
    /// Admit loaded force creation and explicit player insertion.
    ///
    /// Requires admitted players and the same selected declaration source as
    /// `Player` and `CreateTimer`. Forces start with no explicit members and live
    /// until Host/session drop. Enumeration, callbacks and destruction refuse.
    pub fn with_forces(mut self, budget: ForceBudget) -> Result<Self, Error> {
        let fail = |reason| self.inner.error("CreateForce", reason);
        if self.forces.is_some() {
            return Err(fail(Failure::ForceState("already admitted")));
        }
        if self.players.is_none() {
            return Err(fail(Failure::ForceState("players are not admitted")));
        }
        std::alloc::Layout::array::<Vec<HandleId>>(budget.max_records)
            .and_then(|_| std::alloc::Layout::array::<HandleId>(budget.max_memberships))
            .map_err(|_| fail(Failure::Capacity))?;
        let source = &self
            .inner
            .program
            .natives()
            .get("CreateTimer")
            .ok_or_else(|| fail(Failure::Signature))?
            .source;
        let player = self
            .inner
            .program
            .natives()
            .get("Player")
            .ok_or_else(|| fail(Failure::Signature))?;
        if &player.source != source {
            return Err(fail(Failure::Source));
        }
        for (name, params, result) in [
            ("CreateForce", &[][..], "force"),
            ("ForceAddPlayer", &["force", "player"][..], "nothing"),
        ] {
            Self::contract(&self.inner, name, params, result, false, Some("force"))?;
            let native = self
                .inner
                .program
                .natives()
                .get(name)
                .ok_or_else(|| fail(Failure::Signature))?;
            if &native.source != source {
                return Err(self.inner.error(name, Failure::Source));
            }
        }
        let mut kind = self
            .inner
            .program
            .natives()
            .get("CreateForce")
            .ok_or_else(|| fail(Failure::Signature))?
            .ret
            .kind
            .name();
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
        self.forces = Some(Admission {
            budget,
            records: 0,
            memberships: 0,
        });
        Ok(self)
    }
    /// Inspect actual explicit memberships; a reserved same-kind token refuses.
    pub fn force_members(&self, id: HandleId) -> Result<&[HandleId], Error> {
        if self.forces.is_none() {
            return Err(self
                .inner
                .error("CreateForce", Failure::ForceState("not admitted")));
        }
        match self
            .inner
            .entry(id)
            .map_err(|e| self.inner.error("CreateForce", e.failure))?
        {
            Entry::Force { members } => Ok(members),
            _ => Err(self.inner.error("CreateForce", Failure::WrongFamily)),
        }
    }
    pub(super) fn force_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        self.forces.as_ref()?;
        match name {
            "CreateForce" => Some(self.create_force(args)),
            "ForceAddPlayer" => Some(self.force_add_player(args)),
            _ => None,
        }
    }
    fn create_force(&mut self, args: &[Value]) -> Result<Value, Error> {
        let name = "CreateForce";
        if !args.is_empty() {
            return Err(self
                .inner
                .error(name, Failure::ForceState("expected no arguments")));
        }
        let admission = self
            .forces
            .as_mut()
            .ok_or_else(|| self.inner.error(name, Failure::ForceState("not admitted")))?;
        if admission.records >= admission.budget.max_records {
            return Err(self.inner.error(name, Failure::Budget("force records")));
        }
        let id = self.inner.allocate(
            Entry::Force {
                members: Vec::new(),
            },
            name,
        )?;
        admission.records += 1;
        Ok(Value::Handle(id))
    }
    fn force_add_player(&mut self, args: &[Value]) -> Result<Value, Error> {
        let name = "ForceAddPlayer";
        let [Value::Handle(force), Value::Handle(player)] = args else {
            return Err(self.inner.error(
                name,
                Failure::ForceState("expected force and player identities"),
            ));
        };
        let members = self
            .force_members(*force)
            .map_err(|e| self.inner.error(name, e.failure))?;
        self.player_settings(*player)
            .map_err(|e| self.inner.error(name, e.failure))?;
        // shortcut: duplicate adds are idempotent; revisit if native duplicate-input traces differ.
        if members.contains(player) {
            return Ok(Value::Null);
        }
        let admission = self
            .forces
            .as_mut()
            .ok_or_else(|| self.inner.error(name, Failure::ForceState("not admitted")))?;
        if admission.memberships >= admission.budget.max_memberships {
            return Err(self.inner.error(name, Failure::Budget("force memberships")));
        }
        let at = usize::try_from(force.token() - 1)
            .map_err(|_| self.inner.error(name, Failure::Identity))?;
        let Entry::Force { members } = &mut self.inner.arena[at] else {
            return Err(self.inner.error(name, Failure::WrongFamily));
        };
        if members.try_reserve(1).is_err() {
            return Err(self.inner.error(name, Failure::Capacity));
        }
        members.push(*player);
        admission.memberships += 1;
        Ok(Value::Null)
    }
}
