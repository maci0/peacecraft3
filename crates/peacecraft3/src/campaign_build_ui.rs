//! Construction presentation and session-bound ground placement.
use pcx_play::{
    playercolor_host::{BuildUnavailable, ConstructionPreview, ObjectHost},
    training_command::Command,
};
use pcx_script::{HandleId, Host, Value};
use std::sync::Weak;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Choice {
    pub command: Command,
    pub unavailable: Option<BuildUnavailable>,
}

pub(crate) fn choices(objects: &ObjectHost, worker: HandleId) -> Result<Vec<Choice>, String> {
    objects
        .unit_build_options(worker)
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|option| {
            let definition = objects
                .unit_catalog()
                .map_err(|error| error.to_string())?
                .resolve(option.kind.to_be_bytes())
                .map_err(|error| format!("building {:?}: {error:?}", option.kind.to_be_bytes()))?;
            Ok(Choice {
                command: Command::read(&definition)?,
                unavailable: option.unavailable,
            })
        })
        .collect()
}

#[derive(Clone)]
pub(crate) struct Intent {
    worker: HandleId,
    kind: i32,
    session: Weak<()>,
}

/// The caller journals this committed request, including when its callback cancels it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Accepted {
    pub worker: HandleId,
    pub kind: i32,
    pub position: [f32; 2],
}

impl Intent {
    pub(crate) fn arm(
        objects: &ObjectHost,
        worker: HandleId,
        kind: i32,
        session: Weak<()>,
    ) -> Result<Option<Self>, String> {
        let intent = Self {
            worker,
            kind,
            session,
        };
        if !intent.ready(objects, Some(&intent.session))? {
            return Ok(None);
        }
        Ok(Some(intent))
    }

    fn bound_to(&self, session: Option<&Weak<()>>, primary: Option<HandleId>) -> bool {
        self.session.strong_count() != 0
            && primary == Some(self.worker)
            && session.is_some_and(|session| Weak::ptr_eq(session, &self.session))
    }

    pub(crate) fn current(
        &self,
        objects: &ObjectHost,
        session: Option<&Weak<()>>,
    ) -> Result<bool, String> {
        if !self.bound_to(session, objects.selection().first().copied()) {
            return Ok(false);
        }
        let record = objects
            .executed_unit(self.worker)
            .map_err(|error| error.to_string())?;
        let owner = objects
            .player_settings(record.owner)
            .map_err(|error| error.to_string())?;
        Ok(Some(owner.number) == objects.local_player_number()
            && record.shown
            && record.vitals.is_some_and(|vitals| vitals.life > 0.)
            && !record.combat.as_ref().is_some_and(|combat| combat.dead))
    }

    pub(crate) fn ready(
        &self,
        objects: &ObjectHost,
        session: Option<&Weak<()>>,
    ) -> Result<bool, String> {
        Ok(self.current(objects, session)?
            && objects
                .unit_build_options(self.worker)
                .map_err(|error| error.to_string())?
                .iter()
                .any(|option| option.kind == self.kind && option.unavailable.is_none()))
    }

    pub(crate) fn preview(
        &self,
        objects: &mut ObjectHost,
        session: Option<&Weak<()>>,
        point: [f32; 2],
    ) -> Result<Option<ConstructionPreview>, String> {
        if !self.current(objects, session)? {
            return Ok(None);
        }
        objects
            .preview_unit_construction(self.worker, self.kind, point)
            .map(Some)
            .map_err(|error| error.to_string())
    }

    /// A native failure can have effects; callers must not retry this request blindly.
    pub(crate) fn commit(
        &self,
        objects: &mut ObjectHost,
        session: Option<&Weak<()>>,
        point: [f32; 2],
    ) -> Result<Option<Accepted>, String> {
        let Some(ConstructionPreview::Allowed { position }) =
            self.preview(objects, session, point)?
        else {
            return Ok(None);
        };
        let before = objects
            .construction_request_revision(self.worker)
            .map_err(|e| e.to_string())?;
        let result = objects.native(
            "IssuePointOrderById",
            &[
                Value::Handle(self.worker),
                Value::Int(self.kind),
                Value::Real(position[0]),
                Value::Real(position[1]),
            ],
        )?;
        if !matches!(result, Value::Bool(_)) {
            return Err(format!(
                "construction order returned {result:?}, expected boolean"
            ));
        }
        let after = objects
            .construction_request_revision(self.worker)
            .map_err(|e| e.to_string())?;
        if after == before {
            return Ok(None);
        }
        if before.checked_add(1) != Some(after) {
            return Err(format!(
                "construction revision changed unexpectedly: {before} -> {after}"
            ));
        }
        Ok(Some(Accepted {
            worker: self.worker,
            kind: self.kind,
            position,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn placement_belongs_to_its_live_session_and_primary_worker() {
        let session = Arc::new(());
        let replacement = Arc::new(());
        let worker = HandleId::new(std::num::NonZeroU64::new(17).unwrap());
        let intent = Intent {
            worker,
            kind: i32::from_be_bytes(*b"b001"),
            session: Arc::downgrade(&session),
        };
        assert!(intent.bound_to(Some(&Arc::downgrade(&session)), Some(worker)));
        assert!(!intent.bound_to(Some(&Arc::downgrade(&replacement)), Some(worker)));
        assert!(!intent.bound_to(
            Some(&Arc::downgrade(&session)),
            Some(HandleId::new(std::num::NonZeroU64::new(18).unwrap()))
        ));
        assert!(!intent.bound_to(None, Some(worker)));
        drop(session);
        assert!(!intent.bound_to(Some(&intent.session), Some(worker)));
    }
    #[cfg(all(feature = "test-fixtures", not(target_arch = "wasm32")))]
    #[test]
    #[ignore = "requires unchanged Human02 and a private clone of an earned cache via PCX_NATIVE_CHAPTER_*"]
    fn selected_build_buttons_revalidate_selection_session_resources_and_commit_once() {
        use crate::campaign_command_card::{ButtonOrder, Kind};
        use crate::campaign_input::{CampaignInput, place_build, run_command_button};
        use bevy::{
            prelude::*,
            ui_widgets::{Activate, Button},
        };
        use pcx_play::{map_startup_host::MapStartupHost, player_command_log::PlayerCommand};
        use peacecraft3::map_script_session::MapScriptSession;
        let (mut app, mut runtime) = crate::campaign_human_route::human02_controlled();
        crate::campaign_human_route::human02_workers_ready(&mut app, &mut runtime);
        let (worker, kind, witness, owner, bank, state, point) = {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            let witness = session.session_witness().unwrap();
            let state = session.global("PLAYER_STATE_RESOURCE_GOLD").unwrap();
            let objects = session.host().unwrap().objects_mut();
            let owner = objects.local_player_settings().unwrap().identity;
            let (worker, kind) = objects
                .executed_units()
                .filter(|(_, unit)| unit.owner == owner && unit.shown)
                .find_map(|(id, _)| {
                    let options = objects.unit_build_options(id).unwrap();
                    options
                        .into_iter()
                        .find(|option| option.unavailable.is_none())
                        .map(|option| (id, option.kind))
                })
                .expect("ready source worker/build choice");
            objects.select_units(&[worker], true);
            let bank = objects
                .player_resource_value(owner, "PLAYER_STATE_RESOURCE_GOLD")
                .unwrap()
                .unwrap();
            let origin = objects.executed_unit(worker).unwrap().position.unwrap();
            let Value::Real(radius) = objects
                .native("BlzGetUnitCollisionSize", &[Value::Handle(worker)])
                .unwrap()
            else {
                panic!("worker radius")
            };
            assert!(radius.is_finite() && radius > 0.);
            // Bounded diagnostic search, with spacing from the admitted worker collision radius.
            let mut point = None;
            'search: for y in -4..=4 {
                for x in -4..=4 {
                    let candidate = [
                        origin[0] + x as f32 * radius * 4.,
                        origin[1] + y as f32 * radius * 4.,
                    ];
                    if let ConstructionPreview::Allowed { position } = objects
                        .preview_unit_construction(worker, kind, candidate)
                        .unwrap()
                    {
                        point = Some(position);
                        break 'search;
                    }
                }
            }
            (
                worker,
                kind,
                witness,
                owner,
                bank,
                state,
                point.expect("source-admitted nearby placement"),
            )
        };
        let activate = |app: &mut App, unit, kind, witness| {
            app.world_mut()
                .resource_mut::<crate::button_actions::ButtonActions>()
                .entities
                .clear();
            let button = app
                .world_mut()
                .spawn((
                    Button,
                    ButtonOrder {
                        unit,
                        kind,
                        session: witness,
                        order: None,
                    },
                ))
                .id();
            app.world_mut().trigger(Activate { entity: button });
            run_command_button(app);
            app.world_mut().despawn(button);
            assert!(
                app.world().resource::<CampaignInput>().last_error.is_none(),
                "{:?}",
                app.world().resource::<CampaignInput>().last_error
            );
        };
        activate(&mut app, worker, Kind::BuildMenu, witness.clone());
        assert!(
            app.world()
                .resource::<crate::campaign_command_card::CommandCard>()
                .build_page_open()
        );
        let stale = Arc::new(());
        activate(&mut app, worker, Kind::Build(kind), Arc::downgrade(&stale));
        assert!(app.world().resource::<CampaignInput>().build.is_none());
        activate(&mut app, worker, Kind::Build(kind), witness.clone());
        assert!(app.world().resource::<CampaignInput>().build.is_some());
        {
            {
                let mut session = app
                    .world_mut()
                    .resource_mut::<MapScriptSession<MapStartupHost>>();
                session
                    .host()
                    .unwrap()
                    .objects_mut()
                    .select_units(&[], true);
            }
            let mut input = app.world_mut().remove_resource::<CampaignInput>().unwrap();
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            assert!(
                !place_build(session.host().unwrap(), &mut input, Some(&witness), point).unwrap()
            );
            assert!(input.build.is_none());
            app.insert_resource(input);
        }
        {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            session
                .host()
                .unwrap()
                .objects_mut()
                .select_units(&[worker], true);
        }
        activate(&mut app, worker, Kind::Build(kind), witness.clone());
        let mut input = app.world_mut().remove_resource::<CampaignInput>().unwrap();
        {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            let host = session.host().unwrap();
            let before = host
                .objects()
                .construction_request_revision(worker)
                .unwrap();
            // Diagnostic-only readiness change; this fixture does not count as mission progress.
            host.objects_mut()
                .native(
                    "SetPlayerState",
                    &[Value::Handle(owner), state.clone(), Value::Int(0)],
                )
                .unwrap();
            assert!(!place_build(host, &mut input, Some(&witness), point).unwrap());
            assert_eq!(
                host.objects()
                    .construction_request_revision(worker)
                    .unwrap(),
                before
            );
            assert!(
                input
                    .commands
                    .entries()
                    .iter()
                    .all(|entry| !matches!(entry.command, PlayerCommand::PointById { .. }))
            );
            assert_eq!(
                host.objects()
                    .player_resource_value(owner, "PLAYER_STATE_RESOURCE_GOLD")
                    .unwrap(),
                Some(0)
            );
            host.objects_mut()
                .native(
                    "SetPlayerState",
                    &[Value::Handle(owner), state, Value::Int(bank)],
                )
                .unwrap();
            assert!(input.build.is_none(), "lost readiness disarms placement");
        }
        app.insert_resource(input);
        activate(&mut app, worker, Kind::Build(kind), witness.clone());
        let mut input = app.world_mut().remove_resource::<CampaignInput>().unwrap();
        {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            let host = session.host().unwrap();
            let before = host
                .objects()
                .construction_request_revision(worker)
                .unwrap();
            assert!(place_build(host, &mut input, Some(&witness), point).unwrap());
            assert_eq!(
                host.objects()
                    .construction_request_revision(worker)
                    .unwrap(),
                before + 1
            );
            let paid = host
                .objects()
                .player_resource_value(owner, "PLAYER_STATE_RESOURCE_GOLD")
                .unwrap()
                .unwrap();
            assert!(paid < bank);
            assert!(!place_build(host, &mut input, Some(&witness), point).unwrap());
            assert_eq!(
                host.objects()
                    .player_resource_value(owner, "PLAYER_STATE_RESOURCE_GOLD")
                    .unwrap(),
                Some(paid)
            );
            assert_eq!(
                host.objects()
                    .construction_request_revision(worker)
                    .unwrap(),
                before + 1
            );
        }
        let commands: Vec<_> = input
            .commands
            .entries()
            .iter()
            .filter(|entry| matches!(entry.command, PlayerCommand::PointById { .. }))
            .collect();
        assert_eq!(commands.len(), 1);
        assert_eq!(
            commands[0].command,
            PlayerCommand::PointById {
                unit: worker,
                order: kind,
                x: point[0],
                y: point[1]
            }
        );
        app.insert_resource(input);
        let snapshot = |session: &mut MapScriptSession<MapStartupHost>,
                        tick: u64,
                        error: &dyn std::fmt::Display| {
            let globals: Vec<_> = [
                "udg_Gold01",
                "udg_Gold02",
                "udg_Gold03",
                "udg_Lumber01",
                "udg_Lumber02",
            ]
            .into_iter()
            .map(|name| (name, session.global(name)))
            .collect();
            let objects = session.host().unwrap().objects_mut();
            let worker_state = objects
                .executed_unit(worker)
                .map(|unit| (unit.owner, unit.position, unit.vitals));
            println!(
                "construction ECS poststate: completed_tick={tick} host_clock={:?} worker={worker:?} kind={:?} point={point:?} original_owner={owner:?} owner_settings={:?} worker_state={worker_state:?} order={:?} revision={:?} globals={globals:?} error={error}",
                objects.timer_game_time(),
                kind.to_be_bytes(),
                objects.player_settings(owner).map(|player| (
                    player.identity,
                    player.number,
                    player.controller
                )),
                objects.unit_construction_order(worker),
                objects.construction_request_revision(worker)
            );
        };
        let mut site = None;
        for _ in 0..120 {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            if let Err(error) = runtime.advance_ticks(&mut session, 64) {
                snapshot(&mut session, runtime.tick(), &error);
                panic!("{error}; failing tick may have partial effects");
            }
            match session
                .host()
                .unwrap()
                .objects()
                .unit_construction_order(worker)
            {
                Ok(order) => site = order.and_then(|order| order.site),
                Err(error) => {
                    snapshot(&mut session, runtime.tick(), &error);
                    panic!("post-step worker read failed: {error}");
                }
            }
            if site.is_some() {
                break;
            }
        }
        let site = site.expect("public worker travel must create an incomplete site");
        let before_cancel = {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            let objects = session.host().unwrap().objects_mut();
            assert!(objects.unit_construction(site).unwrap().is_some());
            objects.select_units(&[site], true);
            objects
                .player_resource_value(owner, "PLAYER_STATE_RESOURCE_GOLD")
                .unwrap()
                .unwrap()
        };
        activate(
            &mut app,
            site,
            Kind::CancelConstruction,
            Arc::downgrade(&stale),
        );
        {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            let objects = session.host().unwrap().objects();
            assert_eq!(
                objects
                    .player_resource_value(owner, "PLAYER_STATE_RESOURCE_GOLD")
                    .unwrap(),
                Some(before_cancel)
            );
        }
        activate(&mut app, site, Kind::CancelConstruction, witness.clone());
        {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            assert!(
                session
                    .host()
                    .unwrap()
                    .objects()
                    .player_resource_value(owner, "PLAYER_STATE_RESOURCE_GOLD")
                    .unwrap()
                    .unwrap()
                    > before_cancel
            );
            runtime.advance_ticks(&mut session, 1).unwrap();
            assert!(
                session
                    .host()
                    .unwrap()
                    .objects()
                    .executed_unit(site)
                    .is_err()
            );
        }
        let input = app.world().resource::<CampaignInput>();
        let cancellations: Vec<_> = input
            .commands
            .entries()
            .iter()
            .filter(|entry| matches!(entry.command, PlayerCommand::CancelConstruction { .. }))
            .collect();
        assert_eq!(cancellations.len(), 1);
        assert_eq!(
            cancellations[0].command,
            PlayerCommand::CancelConstruction { site }
        );
    }
}
