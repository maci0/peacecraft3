//! Explicit force memberships on the existing session arena.
use super::super::{Entry, Error, Failure, below};
use super::ObjectHost;
use crate::map_admission::ParsedMapContext;
use pcx_script::{HandleId, Value};
use std::sync::Arc;
/// Caller limits independent of the shared identity cap.
#[derive(Clone, Copy, Debug)]
pub struct ForceBudget {
    pub max_records: usize,
    /// Aggregate distinct memberships across all retained forces; also bounds
    /// the separately retained startup participation input.
    pub max_memberships: usize,
}
pub(super) struct Admission {
    budget: ForceBudget,
    records: usize,
    memberships: usize,
    startup_players: Option<Vec<HandleId>>,
}
impl ObjectHost {
    /// Admit loaded force creation and explicit player insertion.
    ///
    /// Requires admitted players and the same selected declaration source as
    /// `Player` and `CreateTimer`. Forces start with no explicit members and live
    /// until Host/session drop. Enumeration requires separate participation admission;
    /// callbacks and destruction refuse.
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
            startup_players: None,
        });
        Ok(self)
    }
    /// Admit caller-qualified startup participation for null-filter enumeration.
    ///
    /// Exact context identity binds these existing player tokens to this Host's
    /// map; it does not establish capture provenance or lobby eligibility. The
    /// caller must qualify the current set and invalidate it on external
    /// participation changes. Accepted controller writes invalidate automatically.
    /// Successful re-admission replaces the snapshot; failure preserves it.
    /// No controller defaults, candidate discovery or predicate execution occur.
    pub fn admit_startup_force_players(
        &mut self,
        context: &Arc<ParsedMapContext>,
        players: &[HandleId],
    ) -> Result<(), Error> {
        let name = "ForceEnumPlayers";
        let fail = |reason| self.inner.error(name, reason);
        let admission = self
            .forces
            .as_ref()
            .ok_or_else(|| fail(Failure::ForceState("forces are not admitted")))?;
        if self
            .player_context()
            .is_none_or(|selected| !Arc::ptr_eq(selected, context))
        {
            return Err(fail(Failure::ForceState(
                "participation map context differs",
            )));
        }
        Self::contract(
            &self.inner,
            name,
            &["force", "boolexpr"],
            "nothing",
            false,
            Some("force"),
        )?;
        let native = self
            .inner
            .program
            .natives()
            .get(name)
            .ok_or_else(|| fail(Failure::Signature))?;
        let create = self
            .inner
            .program
            .natives()
            .get("CreateForce")
            .ok_or_else(|| fail(Failure::Signature))?;
        if native.source != create.source {
            return Err(fail(Failure::Source));
        }
        let mut kind = "boolexpr";
        if !below(&self.inner.program, kind, "agent") {
            return Err(fail(Failure::Signature));
        }
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
            if ty.source != native.source {
                return Err(fail(Failure::Source));
            }
            kind = &ty.parent;
        }
        if kind != "handle" {
            return Err(fail(Failure::Signature));
        }
        if players.len() > admission.budget.max_memberships {
            return Err(fail(Failure::Budget("force participation")));
        }
        for (at, player) in players.iter().enumerate() {
            self.player_settings(*player).map_err(|e| fail(e.failure))?;
            if players[..at].contains(player) {
                return Err(fail(Failure::ForceState("duplicate participant")));
            }
        }
        let mut selected = Vec::new();
        selected
            .try_reserve_exact(players.len())
            .map_err(|_| fail(Failure::Capacity))?;
        selected.extend_from_slice(players);
        self.forces
            .as_mut()
            .expect("checked force admission")
            .startup_players = Some(selected);
        Ok(())
    }
    /// Retained force memberships stay intact; further enumeration needs fresh input.
    /// Call on external departures, lobby changes or other participation changes.
    pub fn invalidate_force_enumeration(&mut self) {
        if let Some(admission) = self.forces.as_mut() {
            admission.startup_players = None;
        }
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
            "ForceEnumPlayers" if self.forces.as_ref()?.startup_players.is_some() => {
                Some(self.force_enum_players(args))
            }
            _ => None,
        }
    }
    fn force_enum_players(&mut self, args: &[Value]) -> Result<Value, Error> {
        let name = "ForceEnumPlayers";
        let [Value::Handle(force), filter] = args else {
            return Err(self
                .inner
                .error(name, Failure::ForceState("expected force and null filter")));
        };
        let old = self
            .force_members(*force)
            .map_err(|e| self.inner.error(name, e.failure))?
            .len();
        if !matches!(filter, Value::Null) {
            return Err(self.inner.error(
                name,
                Failure::ForceState("non-null filters are unsupported"),
            ));
        }
        let admission = self
            .forces
            .as_ref()
            .ok_or_else(|| self.inner.error(name, Failure::ForceState("not admitted")))?;
        let selected = admission.startup_players.as_ref().ok_or_else(|| {
            self.inner
                .error(name, Failure::ForceState("participation is not admitted"))
        })?;
        let total = admission
            .memberships
            .checked_sub(old)
            .and_then(|count| count.checked_add(selected.len()))
            .filter(|count| *count <= admission.budget.max_memberships)
            .ok_or_else(|| self.inner.error(name, Failure::Budget("force memberships")))?;
        let at = usize::try_from(force.token() - 1)
            .map_err(|_| self.inner.error(name, Failure::Identity))?;
        let Entry::Force { members } = &mut self.inner.arena[at] else {
            return Err(self.inner.error(name, Failure::WrongFamily));
        };
        if members
            .try_reserve(selected.len().saturating_sub(members.len()))
            .is_err()
        {
            return Err(self.inner.error(name, Failure::Capacity));
        }
        members.clear();
        members.extend_from_slice(selected);
        self.forces
            .as_mut()
            .expect("checked force admission")
            .memberships = total;
        Ok(Value::Null)
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
