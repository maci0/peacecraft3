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
    pub campaign: Option<Arc<pcx_play::campaign_data_set::CampaignRecipe>>,
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
/// The selected reader survives configuration for gameplay asset preparation.
/// Retaining a reader does not bypass its source-generation checks or read budgets.
#[derive(Resource, Default)]
pub struct SelectedMapSources {
    published: Option<PublishedSources>,
}
pub(crate) struct PublishedSources {
    receipts: Arc<BindingReceipts>,
    resolver: Arc<Resolver>,
    campaign: Option<Arc<pcx_play::campaign_data_set::CampaignRecipe>>,
}
impl SelectedMapSources {
    /// Only the receipts from this publication can select its reader.
    pub fn for_receipts(&self, receipts: &Arc<BindingReceipts>) -> Option<Arc<Resolver>> {
        let published = self.published.as_ref()?;
        Arc::ptr_eq(&published.receipts, receipts).then(|| Arc::clone(&published.resolver))
    }

    pub fn campaign_for_receipts(
        &self,
        receipts: &Arc<BindingReceipts>,
    ) -> Option<Arc<pcx_play::campaign_data_set::CampaignRecipe>> {
        let published = self.published.as_ref()?;
        Arc::ptr_eq(&published.receipts, receipts)
            .then(|| published.campaign.clone())
            .flatten()
    }

    /// Publish one receipt arc with the resolver that produced it.
    /// A later success replaces the pair; callers drop the returned pair
    /// themselves so retired source `Drop` does not run under a lock.
    pub(crate) fn publish(
        &mut self,
        receipts: Arc<BindingReceipts>,
        resolver: Arc<Resolver>,
        campaign: Option<Arc<pcx_play::campaign_data_set::CampaignRecipe>>,
    ) -> Option<PublishedSources> {
        self.published.replace(PublishedSources {
            receipts,
            resolver,
            campaign,
        })
    }

    /// Move a staged pair here.
    /// `None` means the stage was empty. `Some(None)` is a first publication.
    pub(crate) fn commit(&mut self, staged: &mut Self) -> Option<Option<PublishedSources>> {
        Some(self.published.replace(staged.published.take()?))
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
    let mut staged = SelectedMapSources::default();
    let crate::browser_configuration::Configuration {
        resolver,
        campaign,
        map,
        common,
        blizzard,
        catalog_identity,
        catalog_label,
        catalog_sources,
        required_catalog_tables,
        required_fields,
        revision,
        limits,
    } = request;
    let resolver = Arc::new(resolver);
    let input = BindingInputs {
        map: map.borrowed(),
        common: common.borrowed(),
        blizzard: blizzard.borrowed(),
        catalog_identity: &catalog_identity,
        catalog_label: &catalog_label,
        catalog_sources: &catalog_sources,
        required_catalog_tables: &required_catalog_tables,
        required_fields: &required_fields,
        revision,
    };
    candidate.replace_selected_with_campaign(&mut staged, resolver, campaign, input, limits);
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
        let retired = sources
            .commit(&mut staged)
            .expect("successful configuration publishes a reader");
        (Some(std::mem::replace(&mut *active, candidate)), retired)
    } else {
        (None, None::<PublishedSources>)
    };
    queue.completed = Some((id, Box::new(outcome)));
    queue.in_flight = None;
    drop(queue);
    // Retiring caller-owned catalogs/sources can itself run external Drop code.
    drop(previous);
    drop(retired_sources);
    Ok(())
}

#[cfg(test)]
#[path = "../tests/support/browser_configuration_private.rs"]
mod private_tests;
