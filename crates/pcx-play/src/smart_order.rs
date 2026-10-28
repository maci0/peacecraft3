//! The player's `smart` order (right click) resolved to the concrete order a unit gets.
//!
//! The client sends one order id for a right click (`0xd0003`) and each ability of the
//! selected units decides what it means for the target under the cursor. The recreation
//! resolves it before issuing: the unit's abilities come from its ability list, the
//! order each ability gives comes from its `Order` profile, and target classes come
//! from unit data and live state. Classes the shipped data and the executable evidence
//! name but the simulation cannot run refuse with [`SmartRefusal::NotSimulated`].
//! Evidence and the rules table: `docs/re-notes/input-smart-order.md`.
use pcx_script::{HandleId, Host, Value};

use super::{Error, ObjectHost};
use crate::standard_command::Kind;

/// Ability class of the rally point button. The shipped data gives it no `Order` profile.
const RALLY_ABILITY: &str = "ARal";
/// Orders named by the `Order` profile of the abilities that read a smart order.
const HARVEST: &str = "harvest";
const REPAIR: &str = "repair";
const LOAD: &str = "load";
const RETURN_RESOURCES: &str = "returnresources";
const SET_RALLY: &str = "setrally";
const SMART: &str = "smart";

/// What the cursor is over.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SmartTarget {
    Ground([f32; 2]),
    /// A destructable; `at` is the clicked ground point the ground rules fall back to.
    Destructable {
        target: HandleId,
        at: [f32; 2],
    },
    /// An item on the ground; `at` as for a destructable.
    Item {
        target: HandleId,
        at: [f32; 2],
    },
    Unit(HandleId),
}

/// A concrete unit order by registry id.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OrderForm {
    Immediate { order: i32 },
    Point { order: i32, at: [f32; 2] },
    Target { order: i32, target: HandleId },
}

impl OrderForm {
    /// The registered order id.
    pub fn order(self) -> i32 {
        match self {
            Self::Immediate { order } | Self::Point { order, .. } | Self::Target { order, .. } => {
                order
            }
        }
    }
}

/// Why a smart order gave no order. [`SmartRefusal::shown`] separates the refusals
/// the player is told about from the silent ones.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SmartRefusal {
    /// The unit is the target. The move class answers a smart order on itself with a
    /// silent refusal (`0xb64440`).
    SelfTarget,
    /// None of the unit's abilities reads a smart order for this target.
    NoOrder(&'static str),
    /// The class is established from the shipped data but the recreation does not
    /// simulate it, so no order is issued.
    NotSimulated(&'static str),
    /// The data needed to classify the target is missing or malformed.
    Data(String),
}

impl SmartRefusal {
    /// Whether the player is told. A unit with nothing to do and a click on itself are
    /// not errors in the client.
    pub fn shown(&self) -> bool {
        matches!(self, Self::NotSimulated(_) | Self::Data(_))
    }
}

impl std::fmt::Display for SmartRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SelfTarget => write!(f, "a unit cannot target itself"),
            Self::NoOrder(why) => write!(f, "no smart order: {why}"),
            Self::NotSimulated(what) => write!(f, "{what} is not simulated yet"),
            Self::Data(why) => write!(f, "smart order data: {why}"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Relation {
    Own,
    Friend,
    Enemy,
    Neutral,
}

impl Relation {
    fn friendly(self) -> bool {
        matches!(self, Self::Own | Self::Friend)
    }
}

/// Order ids a smart order can resolve to; an order the client's registry lacks is absent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Ids {
    pub smart: Option<i32>,
    pub harvest: Option<i32>,
    pub repair: Option<i32>,
    pub return_resources: Option<i32>,
    pub set_rally: Option<i32>,
}

/// What the selected unit can do.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Capabilities {
    pub move_order: Option<i32>,
    pub attack_order: Option<i32>,
    pub rally: bool,
    pub harvest: bool,
    pub repair: bool,
    pub load: bool,
    pub inventory: bool,
}

/// The unit under the cursor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct UnitTarget {
    pub relation: Relation,
    pub structure: bool,
    pub mechanical: bool,
    pub damaged: bool,
    pub under_construction: bool,
    pub resource: bool,
    pub transport: bool,
    pub harvestable: bool,
    pub takes_cargo: bool,
}

/// The rules table. Pure: the host gathers the facts and this decides.
pub(crate) fn classify_ground(
    unit: &Capabilities,
    ids: &Ids,
    at: [f32; 2],
) -> Result<OrderForm, SmartRefusal> {
    if unit.rally {
        return ids
            .set_rally
            .map(|order| OrderForm::Point { order, at })
            .ok_or_else(|| SmartRefusal::Data("the order registry has no setrally".into()));
    }
    unit.move_order
        .map(|order| OrderForm::Point { order, at })
        .ok_or(SmartRefusal::NoOrder("the unit cannot move"))
}

pub(crate) fn classify_unit(
    unit: &Capabilities,
    ids: &Ids,
    target: HandleId,
    facts: &UnitTarget,
) -> Result<OrderForm, SmartRefusal> {
    if unit.rally {
        return Err(SmartRefusal::NotSimulated("a rally point on a unit"));
    }
    if facts.harvestable
        && let Some(order) = ids.harvest.filter(|_| unit.harvest)
    {
        return Ok(OrderForm::Target { order, target });
    }
    if facts.resource && unit.harvest {
        return Err(SmartRefusal::NotSimulated(
            "harvesting this kind of gold mine",
        ));
    }
    if facts.relation == Relation::Enemy {
        if let Some(order) = unit.attack_order {
            return Ok(OrderForm::Target { order, target });
        }
    } else if facts.relation.friendly() {
        if facts.takes_cargo
            && let Some(order) = ids.return_resources
        {
            return Ok(OrderForm::Immediate { order });
        }
        if unit.repair && (facts.structure || facts.mechanical) {
            if facts.under_construction {
                if let Some(order) = ids.repair {
                    return Ok(OrderForm::Target { order, target });
                }
            } else if facts.damaged {
                return Err(SmartRefusal::NotSimulated(
                    "repairing a building or mechanical unit",
                ));
            }
        }
        if (unit.load && !facts.structure) || facts.transport {
            return Err(SmartRefusal::NotSimulated("loading units into a transport"));
        }
    }
    unit.move_order
        .map(|order| OrderForm::Target { order, target })
        .ok_or(SmartRefusal::NoOrder("the unit cannot move"))
}

pub(crate) fn classify_item(
    unit: &Capabilities,
    ids: &Ids,
    target: HandleId,
    at: [f32; 2],
) -> Result<OrderForm, SmartRefusal> {
    if unit.rally {
        return Err(SmartRefusal::NotSimulated("a rally point on an item"));
    }
    match ids.smart {
        Some(order) if unit.inventory => Ok(OrderForm::Target { order, target }),
        _ => classify_ground(unit, ids, at),
    }
}

/// What a destructable is to the unit: a tree, and whether this unit can harvest it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Destructable {
    pub tree: bool,
    pub harvestable: bool,
}

pub(crate) fn classify_destructable(
    unit: &Capabilities,
    ids: &Ids,
    target: HandleId,
    at: [f32; 2],
    facts: Destructable,
) -> Result<OrderForm, SmartRefusal> {
    if unit.harvest && !unit.rally && facts.tree {
        return match (facts.harvestable, ids.harvest) {
            (true, Some(order)) => Ok(OrderForm::Target { order, target }),
            // A harvest ability whose rules the runtime does not read (Wisp, Acolyte).
            _ => Err(SmartRefusal::NotSimulated(
                "harvesting with this unit's harvest ability",
            )),
        };
    }
    classify_ground(unit, ids, at)
}

impl ObjectHost {
    fn smart_ids(&self) -> Ids {
        let id = |name: &str| self.order_registry.as_ref().and_then(|r| r.id(name));
        Ids {
            smart: id(SMART),
            harvest: id(HARVEST),
            repair: id(REPAIR),
            return_resources: id(RETURN_RESOURCES),
            set_rally: id(SET_RALLY),
        }
    }

    /// Orders and classes of a unit's abilities, from the ability list and the `Order`
    /// profile of each ability.
    fn smart_capabilities(&self, unit: HandleId) -> Result<Capabilities, SmartRefusal> {
        let data = SmartRefusal::Data;
        let record = self.executed_unit(unit).map_err(|e| data(e.to_string()))?;
        let mut found = Capabilities {
            move_order: self
                .standard_command_order(unit, Kind::Move)
                .map_err(|e| data(e.to_string()))?,
            attack_order: self
                .standard_command_order(unit, Kind::Attack)
                .map_err(|e| data(e.to_string()))?,
            inventory: self
                .inventory_policy(unit)
                .ok()
                .is_some_and(|policy| policy.slots > 0),
            ..Capabilities::default()
        };
        // The shipped barracks lists no rally ability; the client gives one to every unit
        // that trains (`Trains` in the unit profile). Some rows list `ARal` themselves.
        found.rally = match super::unit_training::profile(&record.definition, "Trains", "unitList")
        {
            Ok(list) => list.split(',').any(|token| !token.trim().is_empty()),
            Err(why) if why.ends_with("metadata is not admitted") => false,
            Err(why) => return Err(data(why)),
        };
        let Some(abilities) = record.abilities.as_ref() else {
            return Ok(found);
        };
        for (alias, level) in abilities.levels() {
            if level <= 0 {
                continue;
            }
            if self.hero_ability_base(alias).map_err(data)? == RALLY_ABILITY {
                found.rally = true;
            }
            for order in self.hero_ability_profiles(alias, "Order").map_err(data)? {
                match order.trim() {
                    HARVEST => found.harvest = true,
                    REPAIR => found.repair = true,
                    LOAD => found.load = true,
                    _ => {}
                }
            }
        }
        Ok(found)
    }

    /// The `setrally` order of a unit that trains or has the rally ability, which is the
    /// condition the smart order uses to give a ground click the rally meaning.
    pub(super) fn standard_rally_order(&self, unit: HandleId) -> Result<Option<i32>, Error> {
        let caps = self.smart_capabilities(unit).map_err(|refusal| {
            self.inner.error(
                "standard command",
                super::Failure::UnitActivity(refusal.to_string()),
            )
        })?;
        Ok(caps.rally.then(|| self.smart_ids().set_rally).flatten())
    }

    fn smart_relation(&self, unit: HandleId, target: HandleId) -> Result<Relation, SmartRefusal> {
        let data = |e: Error| SmartRefusal::Data(e.to_string());
        let (a, b) = (
            self.executed_unit(unit).map_err(data)?,
            self.executed_unit(target).map_err(data)?,
        );
        Ok(if a.owner == b.owner {
            Relation::Own
        } else if self.is_enemy(unit, target) {
            Relation::Enemy
        } else if self.is_ally(unit, target) {
            Relation::Friend
        } else {
            Relation::Neutral
        })
    }

    fn smart_unit_target(
        &mut self,
        unit: HandleId,
        target: HandleId,
        caps: &Capabilities,
    ) -> Result<UnitTarget, SmartRefusal> {
        let data = |e: String| SmartRefusal::Data(e);
        let relation = self.smart_relation(unit, target)?;
        let record = self
            .executed_unit(target)
            .map_err(|e| data(e.to_string()))?;
        let structure = super::unit_types::structure(&record.definition).map_err(data)?;
        // Only the repair rule reads it, so a unit that cannot repair needs no `type` data.
        let mechanical = caps.repair
            && super::unit_types::class(&record.definition, "UNIT_TYPE_MECHANICAL")
                .map_err(data)?;
        let damaged = record
            .vitals
            .is_some_and(|vitals| vitals.life < vitals.max_life);
        let under_construction = record.construction.is_some();
        let resource = record.resource_amount.is_some();
        let mut transport = false;
        if let Some(abilities) = record.abilities.as_ref() {
            for (alias, level) in abilities.levels().filter(|(_, level)| *level > 0) {
                let _ = level;
                transport |= self
                    .hero_ability_profiles(alias, "Order")
                    .map_err(data)?
                    .iter()
                    .any(|order| order.trim() == LOAD);
            }
        }
        let harvestable = if caps.harvest {
            self.smart_harvest_target(unit, target)
                .map_err(|e| data(e.to_string()))?
                .is_some()
        } else {
            false
        };
        let takes_cargo = relation == Relation::Own
            && caps.harvest
            && self
                .smart_returns_cargo(unit, target)
                .map_err(|e| data(e.to_string()))?;
        Ok(UnitTarget {
            relation,
            structure,
            mechanical,
            damaged,
            under_construction,
            resource,
            transport,
            harvestable,
            takes_cargo,
        })
    }

    /// The order a smart order from `unit` on `target` gives, or why it gives none.
    pub fn resolve_smart_order(
        &mut self,
        unit: HandleId,
        target: SmartTarget,
    ) -> Result<OrderForm, SmartRefusal> {
        let caps = self.smart_capabilities(unit)?;
        let ids = self.smart_ids();
        match target {
            SmartTarget::Ground(at) => classify_ground(&caps, &ids, at),
            SmartTarget::Item { target, at } => classify_item(&caps, &ids, target, at),
            SmartTarget::Destructable { target, at } => {
                let data = |e: Error| SmartRefusal::Data(e.to_string());
                let facts = Destructable {
                    tree: self.smart_is_tree(target).map_err(data)?,
                    harvestable: caps.harvest
                        && self
                            .smart_harvest_target(unit, target)
                            .map_err(data)?
                            .is_some(),
                };
                classify_destructable(&caps, &ids, target, at, facts)
            }
            SmartTarget::Unit(target) => {
                if target == unit {
                    return Err(SmartRefusal::SelfTarget);
                }
                let facts = self.smart_unit_target(unit, target, &caps)?;
                classify_unit(&caps, &ids, target, &facts)
            }
        }
    }

    /// Issues `form` to `unit` through the script order natives, so every order host
    /// sees it as it sees a script order. The native's answer is whether the unit took it.
    pub fn issue_order_form(&mut self, unit: HandleId, form: OrderForm) -> Result<bool, String> {
        let result = match form {
            OrderForm::Immediate { order } => self.native(
                "IssueImmediateOrderById",
                &[Value::Handle(unit), Value::Int(order)],
            ),
            OrderForm::Point { order, at } => self.native(
                "IssuePointOrderById",
                &[
                    Value::Handle(unit),
                    Value::Int(order),
                    Value::Real(at[0]),
                    Value::Real(at[1]),
                ],
            ),
            OrderForm::Target { order, target } => self.native(
                "IssueTargetOrderById",
                &[
                    Value::Handle(unit),
                    Value::Int(order),
                    Value::Handle(target),
                ],
            ),
        }?;
        match result {
            Value::Bool(took) => Ok(took),
            other => Err(format!("order native returned {other:?}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GROUND: [f32; 2] = [10.0, 20.0];

    fn ids() -> Ids {
        Ids {
            smart: Some(851971),
            harvest: Some(851970),
            repair: Some(851972),
            return_resources: Some(851973),
            set_rally: Some(851974),
        }
    }

    fn walker() -> Capabilities {
        Capabilities {
            move_order: Some(100),
            attack_order: Some(101),
            ..Capabilities::default()
        }
    }

    fn worker() -> Capabilities {
        Capabilities {
            harvest: true,
            repair: true,
            ..walker()
        }
    }

    fn hall() -> Capabilities {
        Capabilities {
            rally: true,
            ..Capabilities::default()
        }
    }

    fn unit(relation: Relation) -> UnitTarget {
        UnitTarget {
            relation,
            structure: false,
            mechanical: false,
            damaged: false,
            under_construction: false,
            resource: false,
            transport: false,
            harvestable: false,
            takes_cargo: false,
        }
    }

    fn handle(raw: u32) -> HandleId {
        HandleId::new(std::num::NonZeroU64::new(u64::from(raw)).unwrap())
    }

    #[test]
    fn ground_moves_a_walker_and_sets_the_rally_of_a_structure() {
        assert_eq!(
            classify_ground(&walker(), &ids(), GROUND),
            Ok(OrderForm::Point {
                order: 100,
                at: GROUND
            })
        );
        assert_eq!(
            classify_ground(&hall(), &ids(), GROUND),
            Ok(OrderForm::Point {
                order: 851974,
                at: GROUND
            })
        );
        assert!(matches!(
            classify_ground(&Capabilities::default(), &ids(), GROUND),
            Err(SmartRefusal::NoOrder(_))
        ));
    }

    #[test]
    fn a_rally_structure_without_the_registry_order_refuses_with_data() {
        let none = Ids {
            set_rally: None,
            ..ids()
        };
        let refusal = classify_ground(&hall(), &none, GROUND).unwrap_err();
        assert!(matches!(refusal, SmartRefusal::Data(_)));
        assert!(refusal.shown());
    }

    #[test]
    fn an_enemy_is_attacked_or_followed_when_the_unit_cannot_attack() {
        let enemy = handle(7);
        assert_eq!(
            classify_unit(&walker(), &ids(), enemy, &unit(Relation::Enemy)),
            Ok(OrderForm::Target {
                order: 101,
                target: enemy
            })
        );
        let unarmed = Capabilities {
            attack_order: None,
            ..walker()
        };
        assert_eq!(
            classify_unit(&unarmed, &ids(), enemy, &unit(Relation::Enemy)),
            Ok(OrderForm::Target {
                order: 100,
                target: enemy
            })
        );
    }

    #[test]
    fn own_allied_and_neutral_units_are_followed() {
        let other = handle(8);
        for relation in [Relation::Own, Relation::Friend, Relation::Neutral] {
            assert_eq!(
                classify_unit(&walker(), &ids(), other, &unit(relation)),
                Ok(OrderForm::Target {
                    order: 100,
                    target: other
                })
            );
        }
    }

    #[test]
    fn a_worker_harvests_returns_cargo_and_resumes_construction() {
        let target = handle(9);
        let harvestable = UnitTarget {
            resource: true,
            harvestable: true,
            ..unit(Relation::Neutral)
        };
        assert_eq!(
            classify_unit(&worker(), &ids(), target, &harvestable),
            Ok(OrderForm::Target {
                order: 851970,
                target
            })
        );
        let depot = UnitTarget {
            takes_cargo: true,
            structure: true,
            ..unit(Relation::Own)
        };
        assert_eq!(
            classify_unit(&worker(), &ids(), target, &depot),
            Ok(OrderForm::Immediate { order: 851973 })
        );
        let site = UnitTarget {
            structure: true,
            under_construction: true,
            ..unit(Relation::Own)
        };
        assert_eq!(
            classify_unit(&worker(), &ids(), target, &site),
            Ok(OrderForm::Target {
                order: 851972,
                target
            })
        );
    }

    #[test]
    fn a_worker_without_cargo_follows_a_full_health_hall() {
        let target = handle(9);
        let hall = UnitTarget {
            structure: true,
            ..unit(Relation::Own)
        };
        assert_eq!(
            classify_unit(&worker(), &ids(), target, &hall),
            Ok(OrderForm::Target { order: 100, target })
        );
    }

    #[test]
    fn classes_the_simulation_lacks_refuse_loudly() {
        let target = handle(9);
        let damaged = UnitTarget {
            structure: true,
            damaged: true,
            ..unit(Relation::Own)
        };
        let transport = UnitTarget {
            transport: true,
            ..unit(Relation::Own)
        };
        let odd_mine = UnitTarget {
            resource: true,
            ..unit(Relation::Neutral)
        };
        for (caps, facts) in [
            (worker(), damaged),
            (walker(), transport),
            (worker(), odd_mine),
            (hall(), unit(Relation::Own)),
        ] {
            let refusal = classify_unit(&caps, &ids(), target, &facts).unwrap_err();
            assert!(
                matches!(refusal, SmartRefusal::NotSimulated(_)),
                "{refusal:?}"
            );
            assert!(refusal.shown());
        }
    }

    #[test]
    fn items_need_an_inventory_and_otherwise_fall_back_to_the_ground_point() {
        let item = handle(5);
        let hero = Capabilities {
            inventory: true,
            ..walker()
        };
        assert_eq!(
            classify_item(&hero, &ids(), item, GROUND),
            Ok(OrderForm::Target {
                order: 851971,
                target: item
            })
        );
        assert_eq!(
            classify_item(&walker(), &ids(), item, GROUND),
            Ok(OrderForm::Point {
                order: 100,
                at: GROUND
            })
        );
        assert!(matches!(
            classify_item(&hall(), &ids(), item, GROUND),
            Err(SmartRefusal::NotSimulated(_))
        ));
    }

    #[test]
    fn a_tree_is_harvested_by_a_harvester_and_anything_else_falls_back_to_the_ground_point() {
        let tree = handle(6);
        let tree_for = |harvestable| Destructable {
            tree: true,
            harvestable,
        };
        assert_eq!(
            classify_destructable(&worker(), &ids(), tree, GROUND, tree_for(true)),
            Ok(OrderForm::Target {
                order: 851970,
                target: tree
            })
        );
        let ground = Ok(OrderForm::Point {
            order: 100,
            at: GROUND,
        });
        assert_eq!(
            classify_destructable(&walker(), &ids(), tree, GROUND, tree_for(false)),
            ground
        );
        let rock = Destructable {
            tree: false,
            harvestable: false,
        };
        assert_eq!(
            classify_destructable(&worker(), &ids(), tree, GROUND, rock),
            ground
        );
    }

    #[test]
    fn a_harvest_ability_the_runtime_does_not_read_refuses_on_a_tree() {
        let tree = Destructable {
            tree: true,
            harvestable: false,
        };
        let refusal = classify_destructable(&worker(), &ids(), handle(6), GROUND, tree);
        assert!(matches!(refusal, Err(SmartRefusal::NotSimulated(_))));
    }

    #[test]
    fn only_refusals_the_client_reports_are_shown() {
        assert!(!SmartRefusal::SelfTarget.shown());
        assert!(!SmartRefusal::NoOrder("x").shown());
        assert!(SmartRefusal::NotSimulated("x").shown());
        assert!(SmartRefusal::Data("x".into()).shown());
    }
}
