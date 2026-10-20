//! Normal application producer for the existing admitted-map startup inputs.
use crate::campaign_input::CampaignInput;
use bevy::prelude::*;
use pcx_play::{map_admission::ParsedMapContext, player_command_log::StartIdentity};
use peacecraft3::{
    browser_configuration::SelectedMapSources,
    campaign_startup::{
        CampaignHostSupply, CampaignSeed, CampaignStartup, CampaignStartupSet, StartupError,
    },
    map_requests::{MapRequestSet, MapRequests},
};
use std::sync::{Arc, Weak};

#[derive(Resource, Default)]
struct Prepared {
    context: Option<Weak<ParsedMapContext>>,
    revision: u64,
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<Prepared>().add_systems(
        Update,
        prepare
            .after(MapRequestSet::Deliver)
            .before(CampaignStartupSet),
    );
}

fn prepare(
    requests: Res<MapRequests>,
    sources: Res<SelectedMapSources>,
    launcher: Res<crate::launch_gui::Launcher>,
    mut prepared: ResMut<Prepared>,
    mut supply: ResMut<CampaignHostSupply>,
    mut startup: ResMut<CampaignStartup>,
    mut seed: ResMut<CampaignSeed>,
    mut input: ResMut<CampaignInput>,
) {
    let (Some(bindings), Some(outcome), Some(context)) = (
        requests.bindings.as_ref(),
        requests.last_outcome.as_ref(),
        requests.admission.shared_context(),
    ) else {
        return;
    };
    if outcome.result.is_err()
        || outcome.binding_revision != Some(bindings.revision)
        || outcome.identity.as_ref() != Some(&context.sources.map)
        || !requests
            .published_receipts
            .as_ref()
            .is_some_and(|r| Arc::ptr_eq(r, &bindings.receipts))
    {
        return;
    }
    let weak = Arc::downgrade(&context);
    if prepared.revision == bindings.revision
        && prepared
            .context
            .as_ref()
            .is_some_and(|previous| Weak::ptr_eq(previous, &weak))
    {
        return;
    }
    prepared.revision = bindings.revision;
    prepared.context = Some(weak);
    if supply.revision == bindings.revision && supply.inputs.is_some() {
        return;
    }
    let result = (|| {
        let chosen_seed = choose_seed(seed.requested)?;
        let reader = sources
            .for_receipts(&bindings.receipts)
            .ok_or("admitted map has no matching source reader")?;
        let install = crate::launch_gui::selected_install(&launcher);
        crate::menu_lighting::with_client_image(install.as_deref(), |source, image| {
            peacecraft3::campaign_inputs::prepare_new_game(
                &context,
                bindings,
                &reader,
                image,
                pcx_play::scripts::AssetIdentity {
                    source: source.into(),
                    edition: "selected campaign client".into(),
                    version: "reviewed Legacy 1.29.2.9231".into(),
                },
                chosen_seed,
                launcher.expansion(),
            )
        })
        .map(|inputs| (inputs, chosen_seed))
    })();
    match result {
        Ok((inputs, chosen_seed)) => {
            seed.prepared = Some(chosen_seed);
            input
                .commands
                .begin(StartIdentity::from_context(chosen_seed, &context));
            info!("Campaign new-game seed: {chosen_seed}");
            supply.revision = bindings.revision;
            supply.inputs = Some(inputs);
            startup.last_error = None;
        }
        Err(reason) => {
            warn!("Campaign source preparation failed: {reason}");
            startup.last_error = Some(StartupError::Preparation {
                binding: bindings.revision,
                reason,
            });
        }
    }
}

fn choose_seed(requested: Option<i32>) -> Result<i32, String> {
    if let Some(seed) = requested {
        return Ok(seed);
    }
    let mut bytes = [0; 4];
    getrandom::fill(&mut bytes).map_err(|e| format!("new-game entropy unavailable: {e}"))?;
    Ok(i32::from_le_bytes(bytes))
}

#[cfg(test)]
mod tests {
    #[test]
    fn explicit_seed_retains_zero_and_signed_bits() {
        for seed in [0, -1, i32::MIN, i32::MAX] {
            assert_eq!(super::choose_seed(Some(seed)).unwrap(), seed);
        }
    }
}
