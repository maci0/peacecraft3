use super::super::EventSubject;
use super::*;
use crate::inventory::{InventoryBudget, Policy, Rules};
use pcx_objects::editor::{FieldSource, SlkId};
use pcx_sim::world::FieldValue;

pub(super) struct State {
    rules: Rules,
    budget: InventoryBudget,
    pickup: i32,
    drop: i32,
    pending: Vec<(HandleId, HandleId, i32)>,
}
const CALLS: &[(&str, &[&str], &str, bool)] = &[
    ("UnitHasItem", &["unit", "item"], "boolean", false),
    ("UnitAddItem", &["unit", "item"], "boolean", false),
    ("UnitRemoveItem", &["unit", "item"], "nothing", false),
    (
        "UnitRemoveItemFromSlot",
        &["unit", "integer"],
        "item",
        false,
    ),
    ("UnitItemInSlot", &["unit", "integer"], "item", false),
    ("UnitInventorySize", &["unit"], "integer", false),
    ("RemoveItem", &["item"], "nothing", false),
    ("GetItemX", &["item"], "real", false),
    ("GetItemY", &["item"], "real", false),
    ("GetItemTypeId", &["item"], "integer", false),
    (
        "SetItemInvulnerable",
        &["item", "boolean"],
        "nothing",
        false,
    ),
    ("IsItemInvulnerable", &["item"], "boolean", false),
    ("SetItemDroppable", &["item", "boolean"], "nothing", false),
    ("SetItemDropOnDeath", &["item", "boolean"], "nothing", false),
];
impl ObjectHost {
    pub fn with_inventory(mut self, rules: Rules, budget: InventoryBudget) -> Result<Self, Error> {
        let fail = |s| self.inner.error("inventory", Failure::ItemState(s));
        if self.items.as_ref().is_none_or(|s| s.inventory.is_some()) {
            return Err(fail("item catalog required; inventory admits once"));
        }
        if self.order_registry.as_ref().is_none_or(|registry| {
            registry.binding().image_sha256 != rules.order_source.image_sha256
        }) {
            return Err(fail("matching selected order registry required"));
        }
        std::alloc::Layout::array::<HandleId>(budget.max_slots_per_unit)
            .map_err(|_| self.inner.error("inventory", Failure::Capacity))?;
        std::alloc::Layout::array::<(HandleId, HandleId, i32)>(budget.max_pending_orders)
            .map_err(|_| self.inner.error("inventory", Failure::Capacity))?;
        let source = &self
            .inner
            .program
            .natives()
            .get(CREATE)
            .ok_or_else(|| fail("CreateItem declaration missing"))?
            .source;
        for &(name, args, result, constant) in CALLS {
            if let Some(native) = self.inner.program.natives().get(name) {
                Self::contract(&self.inner, name, args, result, constant, Some("item"))?;
                if &native.source != source {
                    return Err(self.inner.error(name, Failure::Source));
                }
            }
        }
        for (name, result) in [
            ("GetManipulatedItem", "item"),
            ("GetManipulatingUnit", "unit"),
        ] {
            if let Some(native) = self.inner.program.natives().get(name) {
                Self::contract(&self.inner, name, &[], result, true, Some("item"))?;
                if &native.source != source {
                    return Err(self.inner.error(name, Failure::Source));
                }
            }
        }
        for (name, args) in [
            ("IssueTargetOrder", &["unit", "string", "widget"][..]),
            ("IssueTargetOrderById", &["unit", "integer", "widget"][..]),
        ] {
            if let Some(native) = self.inner.program.natives().get(name) {
                Self::contract(&self.inner, name, args, "boolean", false, Some("unit"))?;
                if &native.source != source {
                    return Err(self.inner.error(name, Failure::Source));
                }
            }
        }
        let pickup =
            self.source_constant_enumerand("EVENT_PLAYER_UNIT_PICKUP_ITEM", "playerunitevent")?;
        let drop =
            self.source_constant_enumerand("EVENT_PLAYER_UNIT_DROP_ITEM", "playerunitevent")?;
        self.items.as_mut().expect("admitted").inventory = Some(State {
            rules,
            budget,
            pickup,
            drop,
            pending: Vec::new(),
        });
        Ok(self)
    }
    fn inventory_state(&self) -> Result<&State, Error> {
        self.items
            .as_ref()
            .and_then(|s| s.inventory.as_ref())
            .ok_or_else(|| {
                self.inner
                    .error("inventory", Failure::ItemState("inventory not admitted"))
            })
    }
    pub fn inventory_policy(&self, unit: HandleId) -> Result<Policy, Error> {
        let state = self.inventory_state()?;
        let unit = self.executed_unit(unit)?;
        let abilities = unit.abilities.as_ref().ok_or_else(|| {
            self.inner.error(
                "inventory",
                Failure::ItemState("unit ability state not admitted"),
            )
        })?;
        state
            .rules
            .policy(
                abilities
                    .levels()
                    .filter(|(alias, _)| Some(*alias) != self.unit_implicit_hero_alias()),
                state.budget.max_slots_per_unit,
            )
            .map_err(|s| self.inner.error("inventory", Failure::UnitActivity(s)))
    }
    pub fn can_drop_inventory_item(&self, unit: HandleId, item: HandleId) -> Result<bool, Error> {
        Ok(self.inventory_policy(unit)?.drop_items
            && self
                .inventory_item(item)?
                .is_some_and(|r| r.held_by.is_some_and(|(u, _)| u == unit))
            && self.item_flag(item, "droppable")?)
    }
    pub fn can_use_inventory_item(&self, unit: HandleId, item: HandleId) -> Result<bool, Error> {
        Ok(self.inventory_policy(unit)?.use_items
            && self
                .inventory_item(item)?
                .is_some_and(|r| r.held_by.is_some_and(|(u, _)| u == unit))
            && self.item_flag(item, "usable")?)
    }
    pub(crate) fn cancel_inventory_order(&mut self, unit: HandleId) {
        if let Some(state) = self.items.as_mut().and_then(|s| s.inventory.as_mut()) {
            state.pending.retain(|(u, _, _)| *u != unit);
        }
    }
    pub(crate) fn inventory_order(&self, unit: HandleId) -> Option<i32> {
        self.items
            .as_ref()?
            .inventory
            .as_ref()?
            .pending
            .iter()
            .find(|(u, _, _)| *u == unit)
            .map(|(_, _, o)| *o)
    }
    fn inventory_item(&self, item: HandleId) -> Result<Option<&ExecutedItem>, Error> {
        match self.inner.entry(item)? {
            Entry::Item { record } => Ok(Some(record)),
            Entry::RemovedItem => Ok(None),
            _ => Err(self.inner.error("inventory", Failure::WrongFamily)),
        }
    }
    fn item_flag(&self, item: HandleId, column: &str) -> Result<bool, Error> {
        let item = self
            .inventory_item(item)?
            .ok_or_else(|| self.inner.error("inventory", Failure::Identity))?;
        if column == "drop"
            && let Some(value) = item.drop_on_death
        {
            return Ok(value);
        }
        if column == "droppable"
            && let Some(value) = item.droppable
        {
            return Ok(value);
        }
        let mut fields = item.definition.fields.iter().filter(|(id,field)| {
            field.metadata.as_ref().is_some_and(|metadata| metadata.fourcc==**id
                && matches!(metadata.kind,pcx_objects::editor::FieldKind::Bool|pcx_objects::editor::FieldKind::Int)
                && matches!(&metadata.source,FieldSource::Column{slk:SlkId::ItemData,column:source}if source==column))
        });
        let (_, field) = fields.next().ok_or_else(|| {
            self.inner
                .error("inventory", Failure::ItemState("item policy field missing"))
        })?;
        if fields.next().is_some() {
            return Err(self.inner.error(
                "inventory",
                Failure::ItemState("item policy field ambiguous"),
            ));
        }
        match field.value.as_ref() {
            Ok(FieldValue::Cell(s)) if s.trim() == "0" => Ok(false),
            Ok(FieldValue::Cell(s)) if s.trim() == "1" => Ok(true),
            Ok(FieldValue::Override(pcx_objects::Value::Int(n))) if *n == 0 || *n == 1 => {
                Ok(*n != 0)
            }
            _ => Err(self.inner.error(
                "inventory",
                Failure::ItemState("item policy is not boolean"),
            )),
        }
    }
    fn held_slot(&self, unit: HandleId, slot: usize) -> Option<HandleId> {
        self.executed_items()
            .find(|(_, r)| r.held_by == Some((unit, slot)))
            .map(|(id, _)| id)
    }
    fn reserve_item_events(&mut self, changes: &[(HandleId, bool)]) -> Result<(), Error> {
        let state = self.inventory_state()?;
        let mut count = 0usize;
        for &(unit, pickup) in changes {
            let owner = self.executed_unit(unit)?.owner;
            let enumerand = if pickup { state.pickup } else { state.drop };
            for entry in &self.inner.arena {
                if let Entry::RegisteredPlayerUnitEvent(r) = entry
                    && r.player == owner
                    && r.enumerand == enumerand
                {
                    if let Some(filter) = r.filter {
                        self.filter_callback(filter)?;
                    }
                    count = count
                        .checked_add(1)
                        .ok_or_else(|| self.inner.error("inventory", Failure::Capacity))?;
                }
            }
        }
        self.reserve_event_delivery(count)
    }
    fn item_event(&mut self, unit: HandleId, item: HandleId, pickup: bool) -> Result<(), Error> {
        let owner = self.executed_unit(unit)?.owner;
        let state = self.inventory_state()?;
        let enumerand = if pickup { state.pickup } else { state.drop };
        let fired: Vec<_> = self
            .inner
            .arena
            .iter()
            .filter_map(|e| match e {
                Entry::RegisteredPlayerUnitEvent(r)
                    if r.player == owner && r.enumerand == enumerand =>
                {
                    Some((r.trigger, r.filter, r.playerunitevent, r.player))
                }
                _ => None,
            })
            .collect();
        for (trigger, filter, event, player) in fired {
            self.queue_event(
                trigger,
                filter,
                EventSubject {
                    unit: Some(unit),
                    item: Some(item),
                    player: Some(player),
                    ..EventSubject::default()
                },
                Some(event),
            )?;
        }
        Ok(())
    }
    fn add_inventory_item(&mut self, unit: HandleId, item: HandleId) -> Result<bool, Error> {
        let policy = self.inventory_policy(unit)?;
        let Some(record) = self.inventory_item(item)? else {
            return Ok(false);
        };
        if record.held_by.is_some_and(|(owner, _)| owner == unit) {
            return Ok(true);
        }
        if !policy.get_items
            || self
                .executed_unit(unit)?
                .combat
                .as_ref()
                .is_some_and(|c| c.dead)
        {
            return Ok(false);
        }
        if self.item_flag(item, "powerup")? {
            return Err(self.inner.error(
                "inventory",
                Failure::ItemState("automatic-use powerup effects are not admitted"),
            ));
        }
        let Some(slot) = (0..policy.slots).find(|&s| self.held_slot(unit, s).is_none()) else {
            return Ok(false);
        };
        let holder = record.held_by;
        let mut changes = vec![(unit, true)];
        if let Some((owner, _)) = holder {
            changes.push((owner, false));
        }
        self.reserve_item_events(&changes)?;
        if let Some((owner, _)) = holder {
            self.drop_inventory_item(owner, item)?;
        }
        if let Entry::Item { record } = self.inner.entry_mut(item)? {
            record.held_by = Some((unit, slot));
        }
        self.item_event(unit, item, true)?;
        Ok(true)
    }
    fn drop_inventory_item(&mut self, unit: HandleId, item: HandleId) -> Result<(), Error> {
        let position = self.executed_unit(unit)?.position.ok_or_else(|| {
            self.inner
                .error("inventory", Failure::ItemState("unit position missing"))
        })?;
        if !self
            .inventory_item(item)?
            .is_some_and(|r| r.held_by.is_some_and(|(u, _)| u == unit))
        {
            return Ok(());
        }
        self.reserve_item_events(&[(unit, false)])?;
        if let Entry::Item { record } = self.inner.entry_mut(item)? {
            record.held_by = None;
            record.x = position[0];
            record.y = position[1];
        }
        self.item_event(unit, item, false)
    }
    pub(crate) fn forget_inventory_unit(
        &mut self,
        unit: HandleId,
        removed: bool,
    ) -> Result<(), Error> {
        if self
            .items
            .as_ref()
            .and_then(|state| state.inventory.as_ref())
            .is_none()
        {
            return Ok(());
        }
        self.executed_unit(unit)?;
        let candidates: Vec<_> = self
            .executed_items()
            .filter(|(_, record)| record.held_by.is_some_and(|(owner, _)| owner == unit))
            .map(|(item, _)| item)
            .collect();
        if candidates.is_empty() {
            self.cancel_inventory_order(unit);
            return Ok(());
        }
        let drop_all = removed || self.inventory_policy(unit)?.drop_on_death;
        let held = candidates
            .into_iter()
            .filter_map(|item| {
                if drop_all {
                    return Some(Ok(item));
                }
                match self.item_flag(item, "drop") {
                    Ok(true) => Some(Ok(item)),
                    Ok(false) => None,
                    Err(error) => Some(Err(error)),
                }
            })
            .collect::<Result<Vec<_>, Error>>()?;
        if !held.is_empty() {
            self.executed_unit(unit)?.position.ok_or_else(|| {
                self.inner
                    .error("inventory", Failure::ItemState("unit position missing"))
            })?;
            self.reserve_item_events(&vec![(unit, false); held.len()])?;
        }
        self.cancel_inventory_order(unit);
        for item in held {
            self.drop_inventory_item(unit, item)?;
        }
        Ok(())
    }
    pub(crate) fn inventory_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        self.items.as_ref()?.inventory.as_ref()?;
        self.inner.program.natives().get(name)?;
        if matches!(name, "IssueTargetOrder" | "IssueTargetOrderById") {
            return self.inventory_target_order(name, args);
        }
        if !CALLS.iter().any(|c| c.0 == name) {
            return None;
        }
        Some(self.inventory_call(name, args))
    }
    fn inventory_call(&mut self, name: &str, args: &[Value]) -> Result<Value, Error> {
        let fail = || self.inner.error(name, Failure::Signature);
        let spec = CALLS.iter().find(|c| c.0 == name).ok_or_else(fail)?;
        if args.len() != spec.1.len()
            || args.iter().zip(spec.1).any(|(value, kind)| match *kind {
                "unit" | "item" => !matches!(value, Value::Handle(_) | Value::Null),
                "integer" => !matches!(value, Value::Int(_)),
                "boolean" => !matches!(value, Value::Bool(_)),
                _ => true,
            })
        {
            return Err(fail());
        }
        let null = || match name {
            "UnitHasItem" | "UnitAddItem" | "IsItemInvulnerable" => Value::Bool(false),
            "UnitInventorySize" | "GetItemTypeId" => Value::Int(0),
            "GetItemX" | "GetItemY" => Value::Real(0.),
            _ => Value::Null,
        };
        let Some(first) = args.first() else {
            return Err(fail());
        };
        if *first == Value::Null {
            return Ok(null());
        }
        let Value::Handle(first) = first else {
            return Err(fail());
        };
        if name.starts_with("Unit") {
            match self.inner.entry(*first)? {
                Entry::RemovedUnit => return Ok(null()),
                Entry::Unit { .. } => {}
                _ => return Err(self.inner.error(name, Failure::WrongFamily)),
            }
            let policy = self.inventory_policy(*first)?;
            if name == "UnitInventorySize" && args.len() == 1 {
                return Ok(Value::Int(policy.slots as i32));
            }
            if matches!(name, "UnitItemInSlot" | "UnitRemoveItemFromSlot") {
                let [_, Value::Int(slot)] = args else {
                    return Err(fail());
                };
                let item = usize::try_from(*slot)
                    .ok()
                    .filter(|s| *s < policy.slots)
                    .and_then(|s| self.held_slot(*first, s));
                if name == "UnitRemoveItemFromSlot"
                    && let Some(item) = item
                {
                    self.drop_inventory_item(*first, item)?;
                }
                return Ok(item.map_or(Value::Null, Value::Handle));
            }
            let [_, second] = args else {
                return Err(fail());
            };
            if *second == Value::Null {
                return Ok(null());
            }
            let Value::Handle(item) = second else {
                return Err(fail());
            };
            let held = self
                .inventory_item(*item)?
                .is_some_and(|r| r.held_by.is_some_and(|(u, _)| u == *first));
            return match name {
                "UnitHasItem" => Ok(Value::Bool(held)),
                "UnitAddItem" => self.add_inventory_item(*first, *item).map(Value::Bool),
                "UnitRemoveItem" => {
                    self.drop_inventory_item(*first, *item)?;
                    Ok(Value::Null)
                }
                _ => Err(fail()),
            };
        }
        let Some(record) = self.inventory_item(*first)? else {
            return Ok(null());
        };
        match name {
            "GetItemTypeId" if args.len() == 1 => {
                Ok(Value::Int(i32::from_be_bytes(record.definition.rawcode)))
            }
            "GetItemX" | "GetItemY" if args.len() == 1 => Ok(Value::Real(if name == "GetItemX" {
                record.x
            } else {
                record.y
            })),
            "IsItemInvulnerable" if args.len() == 1 => {
                record.invulnerable.map(Value::Bool).ok_or_else(|| {
                    self.inner.error(
                        name,
                        Failure::ItemState("initial item invulnerability not derived"),
                    )
                })
            }
            "RemoveItem" if args.len() == 1 => {
                let holder = record.held_by;
                if let Some((unit, _)) = holder {
                    self.drop_inventory_item(unit, *first)?;
                }
                *self.inner.entry_mut(*first)? = Entry::RemovedItem;
                Ok(Value::Null)
            }
            "SetItemDroppable" | "SetItemInvulnerable" | "SetItemDropOnDeath" => {
                let [_, Value::Bool(value)] = args else {
                    return Err(fail());
                };
                if let Entry::Item { record } = self.inner.entry_mut(*first)? {
                    if name == "SetItemDropOnDeath" {
                        record.drop_on_death = Some(*value);
                    } else if name == "SetItemDroppable" {
                        record.droppable = Some(*value)
                    } else {
                        record.invulnerable = Some(*value)
                    }
                }
                Ok(Value::Null)
            }
            _ => Err(fail()),
        }
    }
    fn inventory_target_order(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        let [Value::Handle(unit), order, target] = args else {
            return None;
        };
        let state = self.inventory_state().ok()?;
        let order = match (name, order) {
            ("IssueTargetOrder", Value::Str(s)) => self.order_registry.as_ref()?.id(s)?,
            ("IssueTargetOrderById", Value::Int(n)) => *n,
            _ => return None,
        };
        if order != state.rules.smart && order != state.rules.get {
            return None;
        }
        let Value::Handle(item) = target else {
            return Some(Ok(Value::Bool(false)));
        };
        if !matches!(
            self.inner.entry(*item),
            Ok(Entry::Item { .. } | Entry::RemovedItem)
        ) {
            return None;
        }
        Some(
            self.start_item_pickup(*unit, *item, order, name)
                .map(Value::Bool),
        )
    }
    fn start_item_pickup(
        &mut self,
        unit: HandleId,
        item: HandleId,
        order: i32,
        name: &str,
    ) -> Result<bool, Error> {
        let Some(record) = self.inventory_item(item)? else {
            return Ok(false);
        };
        if record.held_by.is_some()
            || self
                .executed_unit(unit)?
                .combat
                .as_ref()
                .is_some_and(|c| c.dead)
        {
            return Ok(false);
        }
        let destination = [record.x, record.y];
        if self.item_flag(item, "powerup")? {
            return Err(self.inner.error(
                name,
                Failure::ItemState("automatic-use powerup effects are not admitted"),
            ));
        }
        let policy = self.inventory_policy(unit)?;
        if !policy.get_items || (0..policy.slots).all(|s| self.held_slot(unit, s).is_some()) {
            return Ok(false);
        }
        let position = self.executed_unit(unit)?.position.ok_or_else(|| {
            self.inner
                .error(name, Failure::ItemState("unit position missing"))
        })?;
        let range = self.inventory_state()?.rules.pickup_range;
        if (position[0] - destination[0]).hypot(position[1] - destination[1]) <= range {
            self.reserve_item_events(&[(unit, true)])?;
            self.steer_unit(unit, None, name)?;
            self.cancel_inventory_order(unit);
            self.clear_combat_target(unit);
            return self.add_inventory_item(unit, item);
        }
        let state = self.inventory_state()?;
        if state.pending.len() >= state.budget.max_pending_orders
            && !state.pending.iter().any(|(u, _, _)| *u == unit)
        {
            return Err(self
                .inner
                .error(name, Failure::Budget("pending item orders")));
        }
        self.items
            .as_mut()
            .expect("items")
            .inventory
            .as_mut()
            .expect("inventory")
            .pending
            .try_reserve(1)
            .map_err(|_| self.inner.error(name, Failure::Capacity))?;
        if !self.steer_unit(unit, Some(destination), name)? {
            return Ok(false);
        }
        self.cancel_inventory_order(unit);
        self.clear_combat_target(unit);
        self.items
            .as_mut()
            .expect("items")
            .inventory
            .as_mut()
            .expect("inventory")
            .pending
            .push((unit, item, order));
        Ok(true)
    }
    pub(crate) fn advance_inventory(&mut self, work: &mut usize) -> Result<(), Error> {
        let Some(state) = self.items.as_ref().and_then(|s| s.inventory.as_ref()) else {
            return Ok(());
        };
        let mut pending = Vec::new();
        pending
            .try_reserve_exact(state.pending.len())
            .map_err(|_| self.inner.error("inventory", Failure::Capacity))?;
        pending.extend_from_slice(&state.pending);
        let range = state.rules.pickup_range;
        for (unit, item, _) in pending {
            *work = work.checked_sub(1).ok_or_else(|| {
                self.inner
                    .error("inventory", Failure::Budget("pickup tick work"))
            })?;
            let Some(record) = self.inventory_item(item)? else {
                self.cancel_inventory_order(unit);
                continue;
            };
            if record.held_by.is_some() {
                self.cancel_inventory_order(unit);
                continue;
            }
            let position = match self.executed_unit(unit) {
                Ok(u) if u.combat.as_ref().is_none_or(|c| !c.dead) => u.position,
                _ => None,
            };
            let Some(position) = position else {
                self.cancel_inventory_order(unit);
                continue;
            };
            if (position[0] - record.x).hypot(position[1] - record.y) <= range {
                self.reserve_item_events(&[(unit, true)])?;
                self.cancel_inventory_order(unit);
                self.steer_unit(unit, None, "pickup")?;
                self.add_inventory_item(unit, item)?;
            }
        }
        Ok(())
    }
}
