//! Owned caller configuration delivered to the existing bounded map consumer.
use crate::map_binding::{BindingInputs, BindingLimits, BindingReceipts, IdentifiedMember};
use crate::map_requests::{MapOutcome, MapRequests};
use bevy::prelude::*;
use pcx_assets::{Resolver, SourceInfo};
use pcx_objects::editor::path::SlkId;
use pcx_play::scripts::AssetIdentity;
use std::sync::{Arc, Mutex};

pub struct Member {
    pub path: String,
    pub label: String,
    pub identity: AssetIdentity,
    pub source: SourceInfo,
}
impl Member {
    fn borrowed(&self) -> IdentifiedMember<'_> {
        IdentifiedMember {
            path: &self.path,
            label: &self.label,
            identity: &self.identity,
            source: &self.source,
        }
    }
}
/// All policy is supplied, including the already caller-configured source resolver.
pub struct Configuration {
    pub resolver: Resolver,
    pub map: Member,
    pub common: Member,
    pub blizzard: Member,
    pub catalog_identity: AssetIdentity,
    pub catalog_label: String,
    pub catalog_sources: Vec<SourceInfo>,
    pub required_catalog_tables: Vec<SlkId>,
    pub required_fields: Vec<[u8; 4]>,
    pub revision: u64,
    pub limits: BindingLimits,
}
impl Configuration {
    fn input(&self) -> BindingInputs<'_> {
        BindingInputs {
            map: self.map.borrowed(),
            common: self.common.borrowed(),
            blizzard: self.blizzard.borrowed(),
            catalog_identity: &self.catalog_identity,
            catalog_label: &self.catalog_label,
            catalog_sources: &self.catalog_sources,
            required_catalog_tables: &self.required_catalog_tables,
            required_fields: &self.required_fields,
            revision: self.revision,
        }
    }
}
/// The selected reader survives configuration for gameplay asset preparation.
/// Retaining a reader does not bypass its source-generation checks or read budgets.
#[derive(Resource, Default)]
pub struct SelectedMapSources {
    published: Option<(Arc<BindingReceipts>, Arc<Resolver>)>,
}
impl SelectedMapSources {
    /// Only the receipts from this publication can select its reader.
    pub fn for_receipts(&self, receipts: &Arc<BindingReceipts>) -> Option<Arc<Resolver>> {
        let (published, resolver) = self.published.as_ref()?;
        Arc::ptr_eq(published, receipts).then(|| Arc::clone(resolver))
    }
}
#[derive(Debug, thiserror::Error)]
pub enum InboxError {
    #[error("acknowledge the existing configuration outcome before submitting another request")]
    Busy,
    #[error("configuration request {id} is unknown or already acknowledged")]
    Unknown { id: u32 },
    #[error("configuration inbox lock is poisoned")]
    Poisoned,
    #[error("configuration request identifiers exhausted")]
    Exhausted,
}
#[derive(Default)]
struct Inbox {
    next_id: u32,
    pending: Option<(u32, Configuration)>,
    in_flight: Option<u32>,
    completed: Option<(u32, Box<MapOutcome>)>,
}
/// One pending request or unacknowledged outcome. No request/outcome is overwritten.
#[derive(Resource, Clone, Default)]
pub struct ConfigurationInbox(Arc<Mutex<Inbox>>);
pub enum Poll {
    Pending,
    Completed(Box<MapOutcome>),
}
impl ConfigurationInbox {
    pub fn enqueue(&self, request: Configuration) -> Result<u32, InboxError> {
        let mut inbox = self.0.lock().map_err(|_| InboxError::Poisoned)?;
        if inbox.pending.is_some() || inbox.in_flight.is_some() || inbox.completed.is_some() {
            return Err(InboxError::Busy);
        }
        let id = inbox.next_id.checked_add(1).ok_or(InboxError::Exhausted)?;
        inbox.next_id = id;
        inbox.pending = Some((id, request));
        Ok(id)
    }
    /// Completed outcomes move to the caller, retaining the original typed failure.
    pub fn poll(&self, id: u32) -> Result<Poll, InboxError> {
        let mut inbox = self.0.lock().map_err(|_| InboxError::Poisoned)?;
        if inbox.in_flight == Some(id)
            || inbox
                .pending
                .as_ref()
                .is_some_and(|(pending, _)| *pending == id)
        {
            return Ok(Poll::Pending);
        }
        if inbox
            .completed
            .as_ref()
            .is_some_and(|(completed, _)| *completed == id)
        {
            return Ok(Poll::Completed(
                inbox.completed.take().expect("matched outcome").1,
            ));
        }
        Err(InboxError::Unknown { id })
    }
}
pub fn install(app: &mut App, inbox: ConfigurationInbox) {
    app.insert_resource(inbox)
        .init_resource::<SelectedMapSources>()
        .add_systems(PreUpdate, consume);
}
fn consume(
    inbox: Res<ConfigurationInbox>,
    mut active: ResMut<MapRequests>,
    mut sources: ResMut<SelectedMapSources>,
) -> bevy::ecs::error::Result {
    let (id, request) = {
        let mut queue = inbox.0.lock().map_err(|_| InboxError::Poisoned)?;
        if queue.in_flight.is_some() {
            return Ok(());
        }
        let Some((id, request)) = queue.pending.take() else {
            return Ok(());
        };
        queue.in_flight = Some(id);
        (id, request)
    };
    let mut candidate = MapRequests::default();
    candidate.replace_selected(&request.resolver, request.input(), request.limits);
    let outcome = candidate
        .last_outcome
        .take()
        .expect("replace_selected reports an outcome");
    if outcome.result.is_ok() {
        candidate.last_outcome = Some(MapOutcome {
            label: outcome.label.clone(),
            identity: outcome.identity.clone(),
            binding_revision: outcome.binding_revision,
            result: Ok(()),
        });
    }
    // Failure preserves every old active field, including the old last_outcome.
    let mut queue = inbox.0.lock().map_err(|_| InboxError::Poisoned)?;
    let (previous, retired_sources) = if outcome.result.is_ok() {
        let receipts = candidate
            .published_receipts
            .as_ref()
            .expect("successful configuration publishes receipts")
            .clone();
        let reader = Arc::new(request.resolver);
        let retired = sources.published.replace((receipts, reader));
        (Some(std::mem::replace(&mut *active, candidate)), retired)
    } else {
        (None, None)
    };
    queue.completed = Some((id, Box::new(outcome)));
    queue.in_flight = None;
    drop(queue);
    // Retiring caller-owned catalogs/sources can itself run external Drop code.
    // The published resolver is dropped here too, after the inbox lock is gone.
    drop(previous);
    drop(retired_sources);
    Ok(())
}

#[cfg(test)]
#[path = "../tests/support/browser_configuration_private.rs"]
mod private_tests;
