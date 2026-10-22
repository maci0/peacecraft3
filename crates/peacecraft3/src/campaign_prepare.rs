//! Normal application producer for the existing admitted-map startup inputs.
use crate::campaign_input::CampaignInput;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use pcx_play::{
    campaign_inputs::MapSources, map_admission::ParsedMapContext, player_command_log::StartIdentity,
};
use peacecraft3::{
    browser_configuration::SelectedMapSources,
    campaign_startup::{
        CampaignHostSupply, CampaignSeed, CampaignStartup, CampaignStartupSet, StartupError,
    },
    map_requests::{MapBindings, MapRequestSet, MapRequests},
};
use std::sync::{Arc, Weak};

#[derive(Resource, Default)]
struct Prepared {
    context: Option<Weak<ParsedMapContext>>,
    revision: u64,
}

#[derive(SystemParam)]
struct PrepareSources<'w> {
    requests: Res<'w, MapRequests>,
    sources: Res<'w, SelectedMapSources>,
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
    selected: PrepareSources,
    mut prepared: ResMut<Prepared>,
    mut supply: ResMut<CampaignHostSupply>,
    mut startup: ResMut<CampaignStartup>,
    mut seed: ResMut<CampaignSeed>,
    mut input: ResMut<CampaignInput>,
) {
    let PrepareSources { requests, sources } = selected;
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
        let install = std::path::Path::new(&bindings.receipts.common.provenance.source.origin);
        crate::menu_lighting::with_client_image(Some(install), |source, image| {
            pcx_play::campaign_inputs::prepare_new_game(
                &context,
                &map_sources(bindings),
                &reader,
                image,
                pcx_play::campaign_inputs::client_identity(image, source),
                chosen_seed,
                matches!(reader.mode().addon, pcx_assets::Addon::Tft),
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

/// The application's bindings, as the Bevy-free startup preparation reads them.
fn map_sources(bindings: &MapBindings) -> MapSources {
    MapSources {
        map_label: bindings.map_label.clone(),
        map_bytes: Arc::clone(&bindings.catalog_map_bytes),
        catalog_identity: bindings.catalog_identity.clone(),
        catalog: Arc::clone(&bindings.catalog.0),
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
    use super::*;
    use pcx_play::map_startup_host::MapStartupHost;
    use peacecraft3::map_script_session::MapScriptSession;

    #[test]
    fn explicit_seed_retains_zero_and_signed_bits() {
        for seed in [0, -1, i32::MIN, i32::MAX] {
            assert_eq!(super::choose_seed(Some(seed)).unwrap(), seed);
        }
    }

    #[test]
    #[ignore = "requires selected PCX_NATIVE_CHAPTER_INSTALL, MAP and ADDON inputs"]
    fn admitted_preparation_ignores_later_launcher_install_and_edition_changes() {
        let mut app = crate::native_campaign::tests::started_app();
        let context = app
            .world()
            .resource::<MapScriptSession<MapStartupHost>>()
            .context()
            .unwrap()
            .clone();
        let requests = app.world().resource::<MapRequests>();
        let bindings = requests.bindings.as_ref().unwrap();
        let expected_image =
            std::path::Path::new(&bindings.receipts.common.provenance.source.origin)
                .join("Warcraft III.exe");
        let reader = app
            .world()
            .resource::<SelectedMapSources>()
            .for_receipts(&bindings.receipts)
            .unwrap();
        let expansion = matches!(reader.mode().addon, pcx_assets::Addon::Tft);
        app.insert_resource(crate::launch_gui::Launcher::test_source(
            "/missing-install-not-the-admitted-source".into(),
            !expansion,
        ));
        app.insert_resource(Prepared::default());
        app.update();
        let supply = app.world().resource::<CampaignHostSupply>();
        let inputs = supply
            .inputs
            .as_ref()
            .expect("the admitted source must still prepare");
        assert_eq!(
            inputs.message_layout.as_ref().unwrap().origin.source,
            expected_image.display().to_string()
        );
        assert_eq!(
            inputs.post_config.version_global.as_deref(),
            Some(if expansion {
                "VERSION_FROZEN_THRONE"
            } else {
                "VERSION_REIGN_OF_CHAOS"
            })
        );
        assert!(
            app.world()
                .resource::<CampaignStartup>()
                .last_error
                .is_none()
        );
        assert!(Arc::ptr_eq(
            app.world()
                .resource::<MapScriptSession<MapStartupHost>>()
                .context()
                .unwrap(),
            &context
        ));
    }
}
