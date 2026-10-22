//! Executed destructable-creation arguments on the shared arena.
//!
//! One allocation path serves the four declared create natives. A record keeps
//! the executed id, finite reals, variation, whether that native supplied `z`,
//! and whether the native requested the dead form. It is not a mesh, terrain
//! height, life total, pathing cell, or world object.
use super::super::{Entry, Error, Failure, below};
use super::ObjectHost;
use pcx_objects::editor::{
    FieldDef, FieldKind, FieldSource, Overlay, SlkId,
    defaults::{DefaultTables, Family, Object, Unresolved},
};
use pcx_script::span::Span;
use pcx_script::{HandleId, Value};
use std::sync::Arc;

const KIND: &str = "destructable";
const BASE: &[&str] = &["integer", "real", "real", "real", "real", "integer"];
const WITH_Z: &[&str] = &["integer", "real", "real", "real", "real", "real", "integer"];

struct Shape {
    name: &'static str,
    parameters: &'static [&'static str],
    /// The native's parameter list includes `z`. Absence is not a height.
    z: bool,
    /// The native is one of the `CreateDead*` declarations. Not a life total.
    dead: bool,
}

const SHAPES: &[Shape] = &[
    Shape {
        name: "CreateDestructable",
        parameters: BASE,
        z: false,
        dead: false,
    },
    Shape {
        name: "CreateDestructableZ",
        parameters: WITH_Z,
        z: true,
        dead: false,
    },
    Shape {
        name: "CreateDeadDestructable",
        parameters: BASE,
        z: false,
        dead: true,
    },
    Shape {
        name: "CreateDeadDestructableZ",
        parameters: WITH_Z,
        z: true,
        dead: true,
    },
];

/// Caller bound on creation records. Not a world or rendering limit.
#[derive(Clone, Copy, Debug)]
pub struct DestructableBudget {
    pub max_records: usize,
}

/// Where a destructable entered the session. Placed objects keep the authored
/// creation number that also keys their collision layer. The complete authored
/// `.doo` record is retained separately by handle; the creation scalars here are
/// the shared script-facing projection, not the full placed origin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DestructableOrigin {
    Placed { creation_number: u32 },
    Script,
}

/// The complete authored `.doo` record for a placed destructable.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlacedSource {
    pub creation_number: u32,
    pub type_id: [u8; 4],
    pub variation: u32,
    pub position: [f32; 3],
    pub angle: f32,
    pub scale: [f32; 3],
    pub flags: u8,
    pub life: u8,
}

/// One executed creation. Coordinates, facing and scale are the call's finite reals.
/// Facing is retained in the declared real, not converted from the source comment.
/// `z` is `None` when the creating native has no `z` parameter. `dead` is the
/// source-requested dead form, not a computed life or a later kill.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DestructableCreation {
    pub origin: DestructableOrigin,
    pub objectid: i32,
    pub x: f32,
    pub y: f32,
    pub z: Option<f32>,
    pub face: f32,
    pub scale: f32,
    pub variation: i32,
    pub dead: bool,
}

/// Script-changed runtime state; `dead` starts from the creation variant.
/// Life is `None` until the selected object catalog supplies the record's HP,
/// so no caller can observe a fabricated zero total.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DestructableState {
    pub invulnerable: bool,
    pub dead: bool,
    pub hidden: bool,
    pub life: Option<f32>,
    pub max_life: Option<f32>,
}

/// Source-derived destructable fields used by the life natives and create path.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DestructableStats {
    pub max_life: f32,
    pub tree: bool,
}

/// The selected object tables plus this map's `.w3b`/`.w3d` overlays.
#[derive(Clone)]
pub struct DestructableCatalog {
    tables: Arc<DefaultTables>,
    destructables: Overlay,
    decorations: Overlay,
}

impl DestructableCatalog {
    pub fn new(tables: Arc<DefaultTables>, destructables: Overlay, decorations: Overlay) -> Self {
        Self {
            tables,
            destructables,
            decorations,
        }
    }

    /// Resolve one destructable rawcode; `None` means the rawcode names a
    /// decoration rather than a destructable. A record can be a custom object
    /// whose base chain must already end in a shipped row.
    pub fn stats_if_destructable(
        &self,
        objectid: i32,
    ) -> Result<Option<DestructableStats>, String> {
        let id = objectid.to_be_bytes();
        let name = std::str::from_utf8(&id).map_err(|_| "non-UTF8 destructable identity")?;
        let Some((family, slk, object, mods)) = self.find(name)? else {
            return Err(format!("{name}: missing destructable catalog record"));
        };
        if family != Family::Destructable {
            return Ok(None);
        }
        let max_life = numeric_field(self.tables.editor(), object, family, slk, "HP", mods, name)?;
        if !max_life.is_finite() || max_life <= 0.0 {
            return Err(format!("{name}: invalid authored HP"));
        }
        let targ = text_field(
            self.tables.editor(),
            object,
            family,
            slk,
            "targType",
            mods,
            name,
        )?;
        let tree = targ.split(',').any(|kind| kind.trim() == "tree");
        Ok(Some(DestructableStats { max_life, tree }))
    }

    pub fn stats(&self, objectid: i32) -> Result<DestructableStats, String> {
        self.stats_if_destructable(objectid)?
            .ok_or_else(|| "stationary record is not a destructable".into())
    }

    pub(crate) fn tables(&self) -> Arc<DefaultTables> {
        Arc::clone(&self.tables)
    }

    fn find(&self, name: &str) -> Result<Option<Found<'_>>, String> {
        let mut found = None;
        for (family, slk, overlay) in [
            (
                Family::Destructable,
                SlkId::DestructableData,
                &self.destructables,
            ),
            (Family::Doodad, SlkId::Doodads, &self.decorations),
        ] {
            let custom = overlay.new_object(name);
            let (base, mods) = custom.unwrap_or((name, overlay.on(name)));
            if overlay.new_object(base).is_some() {
                return Err(format!("{name}: custom destructable base chain"));
            }
            let present = custom.is_some() || overlay.mods.contains_key(name);
            match self.tables.object(family, base) {
                Ok(object) => {
                    if object.rawcode() != base {
                        return Err(format!(
                            "{name}: catalog lookup resolved case-insensitively to {}",
                            object.rawcode()
                        ));
                    }
                    if found.is_some() {
                        return Err(format!("{name}: ambiguous destructable family"));
                    }
                    if mods.len() > 4096 {
                        return Err(format!("{name}: destructable modification budget"));
                    }
                    found = Some((family, slk, object, mods));
                }
                Err(Unresolved::Object { .. }) if !present => {}
                Err(error) => {
                    return Err(format!("{name}: malformed catalog record: {error}"));
                }
            }
        }
        Ok(found)
    }
}

type Found<'a> = (Family, SlkId, Object<'a>, &'a [pcx_objects::Mod]);

fn definition<'a>(
    editor: &'a pcx_objects::editor::Editor,
    family: Family,
    slk: SlkId,
    column: &str,
    name: &str,
) -> Result<&'a FieldDef, String> {
    let mut fields = editor.fields().iter().filter(|f| {
        f.from == family.metadata()
            && matches!(&f.source, FieldSource::Column { slk: s, column: c } if *s == slk && c == column)
    });
    let metadata = fields
        .next()
        .ok_or_else(|| format!("{name}: missing {column} metadata"))?;
    if fields.next().is_some() || !matches!(metadata.index, -1 | 0) {
        return Err(format!("{name}: unsupported {column} metadata"));
    }
    if !metadata.measured {
        return Err(format!("{name}: unmeasured {column} metadata"));
    }
    Ok(metadata)
}

fn chosen_mod<'a>(
    mods: &'a [pcx_objects::Mod],
    definition: &FieldDef,
    name: &str,
    column: &str,
) -> Result<Option<&'a pcx_objects::Mod>, String> {
    let Some(edit) = mods.iter().rev().find(|m| m.field == definition.fourcc) else {
        return Ok(None);
    };
    if edit.selectors.is_some_and(|s| s != [0, 0]) {
        return Err(format!("{name}: unsupported {column} edit selectors"));
    }
    if edit.value_type.word() != definition.kind.wire_word() {
        return Err(format!(
            "{name}: {column} edit has {:?} wire type, metadata declares {}",
            edit.value_type, definition.kind
        ));
    }
    Ok(Some(edit))
}

fn numeric_field(
    editor: &pcx_objects::editor::Editor,
    object: Object<'_>,
    family: Family,
    slk: SlkId,
    column: &str,
    mods: &[pcx_objects::Mod],
    name: &str,
) -> Result<f32, String> {
    let definition = definition(editor, family, slk, column, name)?;
    if !matches!(
        definition.kind,
        FieldKind::Int | FieldKind::Real | FieldKind::Unreal
    ) {
        return Err(format!(
            "{name}: unsupported {column} metadata type {}",
            definition.kind
        ));
    }
    let mapped = object
        .field(definition.fourcc)
        .map_err(|error| format!("{name}: {column}: {error}"))?;
    let base = match (definition.kind, mapped.value) {
        (FieldKind::Int, pcx_objects::editor::defaults::Value::Integer(value)) => value as f64,
        (
            FieldKind::Real | FieldKind::Unreal,
            pcx_objects::editor::defaults::Value::Real(value),
        ) => value,
        _ => return Err(format!("{name}: invalid authored {column}")),
    };
    let value = match chosen_mod(mods, definition, name, column)? {
        None => base,
        Some(edit) => match (definition.kind, &edit.value) {
            (FieldKind::Int, pcx_objects::Value::Int(value)) => f64::from(*value),
            (
                FieldKind::Real | FieldKind::Unreal,
                pcx_objects::Value::Real(value) | pcx_objects::Value::Unreal(value),
            ) => f64::from(*value),
            _ => return Err(format!("{name}: invalid {column} edit value")),
        },
    };
    if !value.is_finite() {
        return Err(format!("{name}: nonfinite authored {column}"));
    }
    Ok(value as f32)
}

fn text_field(
    editor: &pcx_objects::editor::Editor,
    object: Object<'_>,
    family: Family,
    slk: SlkId,
    column: &str,
    mods: &[pcx_objects::Mod],
    name: &str,
) -> Result<String, String> {
    let definition = definition(editor, family, slk, column, name)?;
    if definition.kind != FieldKind::Token {
        return Err(format!(
            "{name}: unsupported {column} metadata type {}",
            definition.kind
        ));
    }
    let mapped = object
        .field(definition.fourcc)
        .map_err(|error| format!("{name}: {column}: {error}"))?;
    let base = match mapped.value {
        pcx_objects::editor::defaults::Value::Text(value)
        | pcx_objects::editor::defaults::Value::Marker(value) => value.to_owned(),
        _ => return Err(format!("{name}: invalid authored {column}")),
    };
    match chosen_mod(mods, definition, name, column)? {
        None => Ok(base),
        Some(edit) => match &edit.value {
            pcx_objects::Value::Str(raw) => std::str::from_utf8(raw)
                .map(str::to_owned)
                .map_err(|_| format!("{name}: non-UTF8 {column}")),
            _ => Err(format!("{name}: invalid {column} edit value")),
        },
    }
}

/// One placed destructable, preflighted by the map reader.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlacedDestructable {
    pub creation: DestructableCreation,
    pub state: DestructableState,
    pub source: PlacedSource,
}

const RUNTIME: [(&str, &[&str], &str); 10] = [
    (
        "SetDestructableInvulnerable",
        &["destructable", "boolean"],
        "nothing",
    ),
    ("IsDestructableInvulnerable", &["destructable"], "boolean"),
    ("KillDestructable", &["destructable"], "nothing"),
    ("ShowDestructable", &["destructable", "boolean"], "nothing"),
    ("GetDestructableX", &["destructable"], "real"),
    ("GetDestructableY", &["destructable"], "real"),
    ("GetDestructableTypeId", &["destructable"], "integer"),
    ("GetDestructableLife", &["destructable"], "real"),
    ("GetDestructableMaxLife", &["destructable"], "real"),
    ("SetDestructableLife", &["destructable", "real"], "nothing"),
];

pub(super) struct Admission {
    max_records: usize,
    records: usize,
    source: String,
    spans: [Option<Span>; SHAPES.len()],
}

impl ObjectHost {
    /// Admit destructable creation on the shared arena.
    ///
    /// `CreateDestructable` is required. Each of the other three create natives
    /// is admitted only when that program declares it with the same source and
    /// the matching parameter list. A program that omits a variant still
    /// admits the base native. The type must extend `widget`. New records are
    /// refused at `budget.max_records` before allocation. A non-finite real is
    /// refused as this recreation's policy; that is not a claim about the
    /// shipped native.
    pub fn with_destructables(mut self, budget: DestructableBudget) -> Result<Self, Error> {
        let fail = |reason| self.inner.error(SHAPES[0].name, reason);
        if self.destructables.is_some() {
            return Err(fail(Failure::DestructableState("already admitted")));
        }
        std::alloc::Layout::array::<DestructableCreation>(budget.max_records)
            .map_err(|_| fail(Failure::Capacity))?;
        if !below(&self.inner.program, KIND, "widget") {
            return Err(fail(Failure::Signature));
        }
        let mut spans = [None; SHAPES.len()];
        let mut source = String::new();
        for (index, shape) in SHAPES.iter().enumerate() {
            let present = self.inner.program.natives().get(shape.name).is_some();
            if index == 0 && !present {
                return Err(fail(Failure::Signature));
            }
            if !present {
                continue;
            }
            Self::contract(
                &self.inner,
                shape.name,
                shape.parameters,
                KIND,
                false,
                Some(KIND),
            )?;
            let native = self
                .inner
                .program
                .natives()
                .get(shape.name)
                .ok_or_else(|| self.inner.error(shape.name, Failure::Signature))?;
            if index == 0 {
                source
                    .try_reserve_exact(native.source.len())
                    .map_err(|_| fail(Failure::Capacity))?;
                source.push_str(&native.source);
            } else if native.source != source {
                return Err(self.inner.error(shape.name, Failure::Source));
            }
            spans[index] = Some(native.span);
        }
        for (name, args, result) in RUNTIME {
            let Some(native) = self.inner.program.natives().get(name) else {
                continue;
            };
            Self::contract(&self.inner, name, args, result, false, Some(KIND))?;
            if native.source != source {
                return Err(self.inner.error(name, Failure::Source));
            }
        }
        self.destructables = Some(Admission {
            max_records: budget.max_records,
            records: 0,
            source,
            spans,
        });
        Ok(self)
    }

    /// Attach the selected object catalog used by life natives and creation.
    pub fn admit_destructable_catalog(
        &mut self,
        catalog: DestructableCatalog,
    ) -> Result<(), Error> {
        if self.destructable_catalog.is_some() {
            return Err(self.inner.error(
                SHAPES[0].name,
                Failure::DestructableState("catalog already admitted"),
            ));
        }
        self.destructable_catalog = Some(catalog);
        Ok(())
    }

    /// Admit source-derived placed destructables in one preflighted batch.
    ///
    /// The caller must run this before the collision cache is assembled. Records,
    /// handle capacity and arena space are checked before the first allocation so
    /// a map-source refusal cannot leave half the objects admitted.
    pub fn admit_placed_destructables(
        &mut self,
        rows: &[PlacedDestructable],
    ) -> Result<Vec<HandleId>, Error> {
        let name = SHAPES[0].name;
        if self.unit_placement.is_some() {
            return Err(self.inner.error(
                name,
                Failure::DestructableState(
                    "stationary collision cache must be built after placed destructables",
                ),
            ));
        }
        if rows.is_empty() {
            return Ok(Vec::new());
        }
        let (records, max_records) = {
            let admitted = self.destructables.as_ref().ok_or_else(|| {
                self.inner
                    .error(name, Failure::DestructableState("not admitted"))
            })?;
            (admitted.records, admitted.max_records)
        };
        let total = records.checked_add(rows.len()).ok_or_else(|| {
            self.inner
                .error(name, Failure::Budget("destructable creation"))
        })?;
        if total > max_records {
            return Err(self
                .inner
                .error(name, Failure::Budget("destructable creation")));
        }
        self.inner.reserve_handles(rows.len(), name)?;
        let mut ids = Vec::new();
        ids.try_reserve_exact(rows.len())
            .map_err(|_| self.inner.error(name, Failure::Capacity))?;
        for row in rows {
            let id = self.inner.allocate(
                Entry::Destructable {
                    creation: row.creation,
                    state: row.state,
                },
                name,
            )?;
            self.placed_sources.insert(id, row.source);
            ids.push(id);
        }
        if let Some(admitted) = self.destructables.as_mut() {
            admitted.records = total;
        }
        Ok(ids)
    }

    /// Apply damage through the same state and collision path as a kill.
    /// `invulnerable` leaves both life and collision untouched.
    pub fn damage_destructable(&mut self, id: HandleId, amount: f32) -> Result<f32, Error> {
        let name = SHAPES[0].name;
        if !amount.is_finite() || amount < 0.0 {
            return Err(self.inner.error(
                name,
                Failure::DestructableState("finite nonnegative damage"),
            ));
        }
        let (origin, life, changed) = {
            let Ok(Entry::Destructable { creation, state }) = self.inner.entry_mut(id) else {
                return Err(self.inner.error(name, Failure::WrongFamily));
            };
            let (Some(max_life), Some(current)) = (state.max_life, state.life) else {
                return Err(self
                    .inner
                    .error(name, Failure::DestructableState("catalog HP not resolved")));
            };
            if state.invulnerable {
                return Ok(current);
            }
            let life = (current - amount).max(0.0).min(max_life);
            state.life = Some(life);
            state.dead |= life <= 0.0;
            (creation.origin, life, !state.dead)
        };
        self.sync_destructable_collision(id, origin, changed)?;
        Ok(life)
    }

    /// Base `CreateDestructable` declaration retained at admission.
    /// The record itself is not a world object.
    pub fn destructable_declaration(&self) -> Result<(&str, Span), Error> {
        let admitted = self.admitted()?;
        let span = admitted.spans[0].ok_or_else(|| {
            self.inner
                .error(SHAPES[0].name, Failure::DestructableState("not admitted"))
        })?;
        Ok((&admitted.source, span))
    }

    /// Span of an admitted create variant. `Ok(None)` means that native was
    /// absent from the program, so calls stay unsupported.
    pub fn destructable_variant_declaration(
        &self,
        name: &str,
    ) -> Result<Option<(&str, Span)>, Error> {
        let index = SHAPES
            .iter()
            .position(|shape| shape.name == name)
            .ok_or_else(|| self.inner.error(name, Failure::Signature))?;
        let admitted = self.admitted()?;
        Ok(admitted.spans[index].map(|span| (admitted.source.as_str(), span)))
    }

    /// Executed creation for an identity of this family. Other families are refused.
    pub fn destructable_creation(&self, id: HandleId) -> Result<DestructableCreation, Error> {
        match self.inner.entry(id)? {
            Entry::Destructable { creation, .. } => Ok(*creation),
            _ => Err(self.inner.error(SHAPES[0].name, Failure::WrongFamily)),
        }
    }

    pub fn executed_destructables(&self) -> impl Iterator<Item = DestructableCreation> + '_ {
        self.inner.arena.iter().filter_map(|entry| match entry {
            Entry::Destructable { creation, .. } => Some(*creation),
            _ => None,
        })
    }

    pub fn destructables_with_handles(
        &self,
    ) -> impl Iterator<Item = (HandleId, DestructableCreation, DestructableState)> + '_ {
        self.inner
            .arena
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                let Entry::Destructable { creation, state } = entry else {
                    return None;
                };
                let token = u64::try_from(index).ok()?.checked_add(1)?;
                Some((
                    HandleId::new(std::num::NonZeroU64::new(token)?),
                    *creation,
                    *state,
                ))
            })
    }

    pub fn destructable_state(&self, id: HandleId) -> Result<DestructableState, Error> {
        match self.inner.entry(id)? {
            Entry::Destructable { state, .. } => Ok(*state),
            _ => Err(self.inner.error(SHAPES[0].name, Failure::WrongFamily)),
        }
    }

    pub(super) fn destructable_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if RUNTIME.iter().any(|&(n, ..)| n == name) {
            self.destructables.as_ref()?;
            self.inner.program.natives().get(name)?;
            return Some(self.destructable_runtime(name, args));
        }
        let index = SHAPES.iter().position(|shape| shape.name == name)?;
        self.destructables.as_ref()?.spans[index]?;
        Some(self.create_destructable(index, args))
    }

    /// A null destructable is ignored by setters and reads as zero/false, as
    /// with other widget natives in this host.
    fn destructable_runtime(&mut self, name: &str, args: &[Value]) -> Result<Value, Error> {
        let (target, rest) = args
            .split_first()
            .ok_or_else(|| self.inner.error(name, Failure::Signature))?;
        if !runtime_signature(name, rest) {
            return Err(self.inner.error(name, Failure::Signature));
        }
        let id = match target {
            Value::Handle(id) => *id,
            Value::Null => {
                return Ok(match name {
                    "IsDestructableInvulnerable" => Value::Bool(false),
                    "GetDestructableX"
                    | "GetDestructableY"
                    | "GetDestructableLife"
                    | "GetDestructableMaxLife" => Value::Real(0.0),
                    "GetDestructableTypeId" => Value::Int(0),
                    _ => Value::Null,
                });
            }
            _ => return Err(self.inner.error(name, Failure::WrongFamily)),
        };
        if !matches!(
            self.inner
                .entry(id)
                .map_err(|e| self.inner.error(name, e.failure))?,
            Entry::Destructable { .. }
        ) {
            return Err(self.inner.error(name, Failure::WrongFamily));
        }
        let (origin, value, changed) = {
            let Ok(Entry::Destructable { creation, state }) = self.inner.entry_mut(id) else {
                unreachable!("checked destructable entry");
            };
            match (name, rest) {
                ("SetDestructableInvulnerable", [Value::Bool(flag)]) => {
                    state.invulnerable = *flag;
                    (creation.origin, Value::Null, false)
                }
                ("ShowDestructable", [Value::Bool(flag)]) => {
                    state.hidden = !flag;
                    (creation.origin, Value::Null, false)
                }
                ("KillDestructable", []) => {
                    let Some(max_life) = state.max_life else {
                        return Err(self
                            .inner
                            .error(name, Failure::DestructableState("catalog HP not resolved")));
                    };
                    let changed = !state.dead;
                    state.life = Some(0.0f32.min(max_life));
                    state.dead = true;
                    (creation.origin, Value::Null, changed)
                }
                ("SetDestructableLife", [Value::Real(value)]) => {
                    let Some(max_life) = state.max_life else {
                        return Err(self
                            .inner
                            .error(name, Failure::DestructableState("catalog HP not resolved")));
                    };
                    let life = value.clamp(0.0, max_life.max(0.0));
                    state.life = Some(life);
                    state.dead = life <= 0.0;
                    (creation.origin, Value::Null, true)
                }
                ("IsDestructableInvulnerable", []) => {
                    (creation.origin, Value::Bool(state.invulnerable), false)
                }
                ("GetDestructableX", []) => (creation.origin, Value::Real(creation.x), false),
                ("GetDestructableY", []) => (creation.origin, Value::Real(creation.y), false),
                ("GetDestructableTypeId", []) => {
                    (creation.origin, Value::Int(creation.objectid), false)
                }
                ("GetDestructableLife", []) => {
                    let Some(life) = state.life else {
                        return Err(self
                            .inner
                            .error(name, Failure::DestructableState("catalog HP not resolved")));
                    };
                    (creation.origin, Value::Real(life), false)
                }
                ("GetDestructableMaxLife", []) => {
                    let Some(max_life) = state.max_life else {
                        return Err(self
                            .inner
                            .error(name, Failure::DestructableState("catalog HP not resolved")));
                    };
                    (creation.origin, Value::Real(max_life), false)
                }
                _ => return Err(self.inner.error(name, Failure::Signature)),
            }
        };
        if changed {
            self.sync_destructable_collision(id, origin, !self.destructable_dead(id, name)?)?;
        }
        Ok(value)
    }

    fn destructable_dead(&self, id: HandleId, name: &str) -> Result<bool, Error> {
        match self.inner.entry(id) {
            Ok(Entry::Destructable { state, .. }) => Ok(state.dead),
            _ => Err(self.inner.error(name, Failure::WrongFamily)),
        }
    }

    fn sync_destructable_collision(
        &mut self,
        id: HandleId,
        origin: DestructableOrigin,
        active: bool,
    ) -> Result<(), Error> {
        let Some(placement) = self.unit_placement.as_mut() else {
            return Ok(());
        };
        let result = match origin {
            DestructableOrigin::Placed { creation_number } => {
                placement.set_placed_active(creation_number, active)
            }
            DestructableOrigin::Script => placement.set_destructable_active(id, active),
        };
        result.map(|_| ()).map_err(|e| {
            self.inner
                .error(SHAPES[0].name, Failure::DestructableSource(e.0))
        })
    }

    fn admitted(&self) -> Result<&Admission, Error> {
        self.destructables.as_ref().ok_or_else(|| {
            self.inner
                .error(SHAPES[0].name, Failure::DestructableState("not admitted"))
        })
    }

    fn create_destructable(&mut self, index: usize, args: &[Value]) -> Result<Value, Error> {
        let shape = &SHAPES[index];
        let name = shape.name;
        let creation =
            creation_from(shape, args).map_err(|reason| self.inner.error(name, reason))?;
        let (records, max_records) = {
            let admitted = self.destructables.as_ref().ok_or_else(|| {
                self.inner
                    .error(name, Failure::DestructableState("not admitted"))
            })?;
            (admitted.records, admitted.max_records)
        };
        if records >= max_records {
            return Err(self
                .inner
                .error(name, Failure::Budget("destructable creation")));
        }
        let stats = self
            .destructable_catalog
            .as_ref()
            .map(|catalog| {
                catalog
                    .stats(creation.objectid)
                    .map_err(|e| self.inner.error(name, Failure::DestructableSource(e)))
            })
            .transpose()?;
        let max_life = stats.map(|stats| stats.max_life);
        let life = match stats {
            Some(_) if creation.dead => Some(0.0),
            Some(stats) => Some(stats.max_life),
            None => None,
        };
        let prepared = match self.unit_placement.as_mut() {
            Some(placement) => {
                self.inner.reserve_handles(1, name)?;
                Some(
                    placement
                        .prepare_script_destructable(
                            creation.objectid,
                            [creation.x, creation.y],
                            creation.face,
                            creation.dead,
                        )
                        .map_err(|e| self.inner.error(name, Failure::DestructableSource(e.0)))?,
                )
            }
            None => None,
        };
        let id = self.inner.allocate(
            Entry::Destructable {
                creation,
                state: DestructableState {
                    dead: creation.dead,
                    life,
                    max_life,
                    ..DestructableState::default()
                },
            },
            name,
        )?;
        if let Some(prepared) = prepared {
            let placement = self.unit_placement.as_mut().ok_or_else(|| {
                self.inner
                    .error(name, Failure::DestructableState("not admitted"))
            })?;
            placement.publish_script_destructable(id, prepared);
        }
        let Some(admitted) = self.destructables.as_mut() else {
            return Err(self
                .inner
                .error(name, Failure::DestructableState("not admitted")));
        };
        admitted.records += 1;
        Ok(Value::Handle(id))
    }
}

fn runtime_signature(name: &str, rest: &[Value]) -> bool {
    match name {
        "SetDestructableInvulnerable" | "ShowDestructable" => {
            matches!(rest, [Value::Bool(_)])
        }
        "KillDestructable"
        | "IsDestructableInvulnerable"
        | "GetDestructableX"
        | "GetDestructableY"
        | "GetDestructableTypeId"
        | "GetDestructableLife"
        | "GetDestructableMaxLife" => rest.is_empty(),
        "SetDestructableLife" => matches!(rest, [Value::Real(value)] if value.is_finite()),
        _ => false,
    }
}

fn creation_from(shape: &Shape, args: &[Value]) -> Result<DestructableCreation, Failure> {
    let mismatch =
        Failure::DestructableState("arguments do not match the destructable native declaration");
    if shape.z {
        let [
            Value::Int(objectid),
            x,
            y,
            z,
            face,
            scale,
            Value::Int(variation),
        ] = args
        else {
            return Err(mismatch);
        };
        Ok(DestructableCreation {
            origin: DestructableOrigin::Script,
            objectid: *objectid,
            x: finite_real(x)?,
            y: finite_real(y)?,
            z: Some(finite_real(z)?),
            face: finite_real(face)?,
            scale: finite_real(scale)?,
            variation: *variation,
            dead: shape.dead,
        })
    } else {
        let [
            Value::Int(objectid),
            x,
            y,
            face,
            scale,
            Value::Int(variation),
        ] = args
        else {
            return Err(mismatch);
        };
        Ok(DestructableCreation {
            origin: DestructableOrigin::Script,
            objectid: *objectid,
            x: finite_real(x)?,
            y: finite_real(y)?,
            z: None,
            face: finite_real(face)?,
            scale: finite_real(scale)?,
            variation: *variation,
            dead: shape.dead,
        })
    }
}

fn finite_real(value: &Value) -> Result<f32, Failure> {
    match value {
        Value::Real(real) if real.is_finite() => Ok(*real),
        Value::Real(_) => Err(Failure::DestructableState(
            "recreation policy: finite real required",
        )),
        _ => Err(Failure::DestructableState(
            "arguments do not match the destructable native declaration",
        )),
    }
}
