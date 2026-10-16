//! Quest admission validates the selected declarations' handle ancestry instead
//! of requiring the generic object contract's immediate `agent` parent.
use super::super::{Entry, EnumHost, Error, Failure};
use super::ObjectHost;
use crate::quest_session::{
    ObjectClass, ObjectKind, QuestArena, QuestFailure, QuestSession, accepts,
};
use pcx_script::{HandleId, Value};

pub use crate::quest_session::{QuestBudget, QuestEvent, QuestFlag};

struct EnumBridge<'a> {
    inner: &'a mut EnumHost,
}

impl QuestArena for EnumBridge<'_> {
    fn alloc(&mut self, kind: ObjectKind) -> Result<HandleId, QuestFailure> {
        let entry = match kind {
            ObjectKind::Quest => Entry::Quest { live: true },
            ObjectKind::Item => Entry::QuestItem { live: true },
        };
        self.inner
            .allocate(entry, "CreateQuest")
            .map_err(|_| QuestFailure::Capacity)
    }

    fn classify(&self, id: HandleId) -> Result<ObjectClass, QuestFailure> {
        match self.inner.entry(id) {
            Ok(Entry::Quest { live: true }) => Ok(ObjectClass::Quest),
            Ok(Entry::Quest { live: false }) => Ok(ObjectClass::DeadQuest),
            Ok(Entry::QuestItem { live: true }) => Ok(ObjectClass::Item),
            Ok(Entry::QuestItem { live: false }) => Ok(ObjectClass::DeadItem),
            Ok(_) => Ok(ObjectClass::Other),
            Err(_) => Err(QuestFailure::Identity),
        }
    }

    fn kill(&mut self, id: HandleId) -> Result<(), QuestFailure> {
        let at = usize::try_from(id.token() - 1).map_err(|_| QuestFailure::Identity)?;
        match self.inner.arena.get_mut(at) {
            Some(Entry::Quest { live }) if *live => {
                *live = false;
                Ok(())
            }
            Some(Entry::QuestItem { live }) if *live => {
                *live = false;
                Ok(())
            }
            Some(Entry::Quest { live: false } | Entry::QuestItem { live: false }) | None => {
                Err(QuestFailure::Identity)
            }
            Some(_) => Err(QuestFailure::WrongFamily),
        }
    }
}

fn map_failure(failure: QuestFailure) -> Failure {
    match failure {
        QuestFailure::Arguments => Failure::QuestArguments,
        QuestFailure::Identity => Failure::Identity,
        QuestFailure::WrongFamily => Failure::WrongFamily,
        QuestFailure::Budget(what) => Failure::Budget(what),
        QuestFailure::Capacity => Failure::Capacity,
        QuestFailure::Signature => Failure::Signature,
        QuestFailure::Source => Failure::Source,
        QuestFailure::State(detail) => Failure::QuestState(detail),
    }
}

impl ObjectHost {
    pub fn with_quests(mut self, budget: QuestBudget) -> Result<Self, Error> {
        if self.quests.is_some() {
            return Err(self
                .inner
                .error("CreateQuest", Failure::QuestState("already admitted")));
        }
        let kinds = self
            .inner
            .conversions
            .values()
            .map(|conversion| conversion.kind.clone())
            .collect::<Vec<_>>();
        let admitted = QuestSession::admit(&self.inner.program, budget, &kinds);
        let session = match admitted {
            Ok(session) => session,
            Err(error) => {
                return Err(self.inner.error(error.native, map_failure(error.failure)));
            }
        };
        self.quests = Some(session);
        Ok(self)
    }

    pub fn quest_events(&self) -> Option<&[QuestEvent]> {
        self.quests.as_ref().map(QuestSession::events)
    }

    pub(super) fn quest_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if self.quests.is_none() || !accepts(name) {
            return None;
        }
        Some(self.apply_quest(name, args))
    }

    fn apply_quest(&mut self, name: &str, args: &[Value]) -> Result<Value, Error> {
        let result = {
            let ObjectHost { inner, quests, .. } = self;
            if let Some(quests) = quests.as_mut() {
                let mut bridge = EnumBridge { inner };
                Some(quests.call(&mut bridge, name, args))
            } else {
                None
            }
        };
        match result {
            None => Err(self.inner.error(name, Failure::QuestState("not admitted"))),
            Some(value) => value.map_err(|failure| self.inner.error(name, map_failure(failure))),
        }
    }
}

#[cfg(test)]
#[path = "quest_host_tests.rs"]
mod integration;
