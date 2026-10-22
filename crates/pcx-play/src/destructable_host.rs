//! Executed destructable-creation arguments on the shared arena.
//!
//! One allocation path serves the four declared create natives. A record keeps
//! the executed id, finite reals, variation, whether that native supplied `z`,
//! and whether the native requested the dead form. It is not a mesh, terrain
//! height, life total, pathing cell, or world object.
use super::super::{Entry, Error, Failure, below};
use super::ObjectHost;
use crate::{
    camera_margins::SourceWindow,
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
    scripts::AssetIdentity,
};
use pcx_objects::editor::{
    FieldDef, FieldKind, FieldSource, Overlay, SlkId,
    defaults::{DefaultTables, Family, Object, Unresolved},
};
use pcx_script::span::Span;
use pcx_script::{HandleId, ScriptCall, Value};
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

/// The selected client's restore threshold and minimum life, read from the
/// native path that handles the declared three-argument restore call.
#[derive(Clone, Debug)]
pub struct RestoreLifeBinding {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    pub body: SourceWindow,
    pub zero_initializer: SourceWindow,
    pub threshold: SourceWindow,
    pub minimum: SourceWindow,
    pub subtract_va: u32,
    pub convert_va: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct RestoreLifePolicy {
    zero: f32,
    threshold: f32,
    minimum: f32,
}

impl RestoreLifePolicy {
    pub fn read(
        image: &[u8],
        binding: RestoreLifeBinding,
        budget: RuleInputBudget,
    ) -> Result<Self, String> {
        let fail =
            |reason: &str| format!("{}: destructable restore: {reason}", binding.origin.source);
        if image.len() > budget.image_bytes
            || sha256(image) != binding.image_sha256
            || [
                &binding.origin.source,
                &binding.origin.edition,
                &binding.origin.version,
            ]
            .iter()
            .any(|s| s.trim().is_empty())
        {
            return Err(fail("missing or mismatched client identity"));
        }
        let mut total = 0usize;
        for window in [
            &binding.body,
            &binding.zero_initializer,
            &binding.threshold,
            &binding.minimum,
        ] {
            total = total
                .checked_add(window.len as usize)
                .filter(|&bytes| bytes <= budget.profile_bytes)
                .ok_or_else(|| fail("restore windows exceed input budget"))?;
        }
        let pe = Pe::parse(image, budget).map_err(|e| fail(&e.to_string()))?;
        let body = pe
            .window(&binding.body, true)
            .map_err(|e| fail(&e.to_string()))?;
        let init = pe
            .window(&binding.zero_initializer, true)
            .map_err(|e| fail(&e.to_string()))?;
        let threshold = pe
            .window(&binding.threshold, false)
            .map_err(|e| fail(&e.to_string()))?;
        let minimum = pe
            .window(&binding.minimum, false)
            .map_err(|e| fail(&e.to_string()))?;
        let word = |bytes: &[u8], at: usize| -> Option<u32> {
            Some(u32::from_le_bytes(
                bytes.get(at..at.checked_add(4)?)?.try_into().ok()?,
            ))
        };
        let addressed = |bytes: &[u8], at: usize, va: u32| word(bytes, at) == Some(va);
        let call = |bytes: &[u8], at: usize, base: u32| -> Option<u32> {
            (bytes.get(at) == Some(&0xe8)).then_some(
                base.wrapping_add(at as u32 + 5)
                    .wrapping_add(word(bytes, at + 1)?),
            )
        };
        if body.len() != 0x75
            || init.len() != 16
            || threshold.len() != 4
            || minimum.len() != 4
            || body.get(0x28) != Some(&0x68)
            || !addressed(
                body,
                0x29,
                word(init, 3).ok_or_else(|| fail("truncated zero initializer"))?,
            )
            || body.get(0x33..0x37) != Some(&[0xf3, 0x0f, 0x10, 0x05])
            || !addressed(body, 0x37, binding.threshold.va)
            || body.get(0x40..0x45) != Some(&[0x25, 0xff, 0xff, 0xff, 0x7f])
            || body.get(0x48..0x4e) != Some(&[0x0f, 0x2f, 0x45, 0x08, 0x76, 0x12])
            || body.get(0x65..0x68) != Some(&[0x0f, 0x2f, 0x05])
            || !addressed(body, 0x68, binding.minimum.va)
            || body.get(0x6c) != Some(&0xa1)
            || !addressed(body, 0x6d, binding.minimum.va)
            || body.get(0x71..0x75) != Some(&[0x0f, 0x47, 0x45, 0xfc])
            || init.first() != Some(&0x6a)
            || init.get(2) != Some(&0x68)
            || init.get(7) != Some(&0xe8)
            || init.get(12..) != Some(&[0x83, 0xc4, 0x08, 0xc3])
            || call(body, 0x2e, binding.body.va) != Some(binding.subtract_va)
            || call(init, 7, binding.zero_initializer.va) != Some(binding.convert_va)
        {
            return Err(fail("unsupported restore or initializer operands"));
        }
        let threshold = f32::from_bits(word(threshold, 0).unwrap());
        let minimum = f32::from_bits(word(minimum, 0).unwrap());
        if !threshold.is_finite() || threshold <= 0.0 || !minimum.is_finite() || minimum <= 0.0 {
            return Err(fail("invalid restore threshold or minimum"));
        }
        Ok(Self {
            zero: f32::from(init[1] as i8),
            threshold,
            minimum,
        })
    }

    #[cfg(test)]
    pub(crate) fn synthetic(zero: f32, threshold: f32, minimum: f32) -> Self {
        Self {
            zero,
            threshold,
            minimum,
        }
    }

    fn life(self, requested: f32, maximum: f32) -> f32 {
        if (requested - self.zero).abs() < self.threshold {
            maximum
        } else {
            requested.max(self.minimum).min(maximum)
        }
    }
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
            (FieldKind::Int, pcx_objects::Value::Int(value)) => f64::from(*value as i32),
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

const RUNTIME: [(&str, &[&str], &str); 11] = [
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
    (
        "DestructableRestoreLife",
        &["destructable", "real", "boolean"],
        "nothing",
    ),
];

const DEATH_NATIVES: [&str; 3] = [
    "SetDestructableLife",
    "KillDestructable",
    "DestructableRestoreLife",
];

#[derive(Clone, Copy)]
struct LifePlan {
    id: HandleId,
    origin: DestructableOrigin,
    life: f32,
    dead: bool,
    was_dead: bool,
}

/// A preflighted damage transition that has reserved room for every death
/// event it will queue. It does not change life, death or collision until
/// [`ObjectHost::commit_destructable_damage`].
#[derive(Clone, Copy)]
pub(crate) struct PreparedDestructableDamage {
    id: HandleId,
    origin: DestructableOrigin,
    life: f32,
    dead: bool,
    was_dead: bool,
    events: usize,
}

impl PreparedDestructableDamage {
    /// Resulting life after the committed hit.
    pub(crate) fn life(&self) -> f32 {
        self.life
    }

    /// Whether the target was already dead before the planned hit.
    pub(crate) fn was_dead(&self) -> bool {
        self.was_dead
    }
}

pub(super) struct Admission {
    max_records: usize,
    records: usize,
    source: String,
    spans: [Option<Span>; SHAPES.len()],
}

impl ObjectHost {
    pub fn with_destructable_restore(mut self, policy: RestoreLifePolicy) -> Result<Self, Error> {
        if self.destructable_restore.is_some() {
            return Err(self.inner.error(
                "DestructableRestoreLife",
                Failure::DestructableState("restore policy already admitted"),
            ));
        }
        self.destructable_restore = Some(policy);
        Ok(self)
    }
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

    /// Apply damage through the same state, event and collision path as a kill.
    /// `invulnerable` leaves both life and collision untouched.
    pub fn damage_destructable(&mut self, id: HandleId, amount: f32) -> Result<f32, Error> {
        let Some(plan) = self.prepare_destructable_damage(id, amount)? else {
            return self.current_destructable_life(id);
        };
        let life = plan.life;
        self.commit_destructable_damage(plan);
        Ok(life)
    }

    /// Preflight a damage transition and reserve room for its death events.
    /// `None` means invulnerability left the target unchanged. The returned
    /// plan is committed with [`Self::commit_destructable_damage`].
    pub(crate) fn prepare_destructable_damage(
        &mut self,
        id: HandleId,
        amount: f32,
    ) -> Result<Option<PreparedDestructableDamage>, Error> {
        let name = SHAPES[0].name;
        if !amount.is_finite() || amount < 0.0 {
            return Err(self.inner.error(
                name,
                Failure::DestructableState("finite nonnegative damage"),
            ));
        }
        let (origin, life, was_dead) = {
            let Ok(Entry::Destructable { creation, state }) = self.inner.entry(id) else {
                return Err(self.inner.error(name, Failure::WrongFamily));
            };
            let (Some(max_life), Some(current)) = (state.max_life, state.life) else {
                return Err(self
                    .inner
                    .error(name, Failure::DestructableState("catalog HP not resolved")));
            };
            if state.invulnerable {
                return Ok(None);
            }
            let life = (current - amount).max(0.0).min(max_life);
            (creation.origin, life, state.dead)
        };
        let dead = was_dead || life <= 0.0;
        let events = if dead && !was_dead {
            self.death_event_count(id)
        } else {
            0
        };
        if events > 0 {
            self.reserve_event_delivery(events)?;
        }
        Ok(Some(PreparedDestructableDamage {
            id,
            origin,
            life,
            dead,
            was_dead,
            events,
        }))
    }

    /// Commit a prepared damage transition. All fallible work, including event
    /// queue reservation, happened in preparation; collision toggles for
    /// admitted stationary layers do not allocate or fail.
    pub(crate) fn commit_destructable_damage(&mut self, plan: PreparedDestructableDamage) {
        debug_assert_eq!(plan.dead, plan.was_dead() || plan.life() <= 0.0);
        match self.inner.entry_mut(plan.id) {
            Ok(Entry::Destructable { state, .. }) => {
                state.life = Some(plan.life);
                state.dead = plan.dead;
            }
            _ => unreachable!("prepared destructable entry"),
        }
        self.sync_destructable_collision(plan.id, plan.origin, !plan.dead);
        if plan.events > 0 {
            self.push_death_events(plan.id, plan.events);
        }
    }

    fn current_destructable_life(&self, id: HandleId) -> Result<f32, Error> {
        match self.inner.entry(id) {
            Ok(Entry::Destructable {
                state:
                    DestructableState {
                        life: Some(life), ..
                    },
                ..
            }) => Ok(*life),
            Ok(Entry::Destructable { .. }) => Err(self.inner.error(
                SHAPES[0].name,
                Failure::DestructableState("catalog HP not resolved"),
            )),
            _ => Err(self.inner.error(SHAPES[0].name, Failure::WrongFamily)),
        }
    }

    /// Intercept a death-producing destructable native so matching callbacks run
    /// before the native returns. Non-death writes and unregistered targets stay
    /// on the ordinary native path.
    pub(crate) fn begin_synchronous_destructable_death_call(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Result<Option<ScriptCall>, Error> {
        if !DEATH_NATIVES.contains(&name) || !self.destructable_death_registered() {
            return Ok(None);
        }
        let (target, rest) = args
            .split_first()
            .ok_or_else(|| self.inner.error(name, Failure::Signature))?;
        let Some(plan) = self.plan_life(name, target, rest)? else {
            return Ok(None);
        };
        if !plan.dead || plan.was_dead {
            return Ok(None);
        }
        let count = self.death_event_count(plan.id);
        if count == 0 {
            return Ok(None);
        }
        self.prepare_synchronous_frame(name)?;
        self.begin_synchronous_delivery();
        let result = (|| {
            self.reserve_event_delivery(count)?;
            self.commit_life(plan);
            self.push_death_events(plan.id, count);
            Ok(())
        })();
        let events = self.take_synchronous_delivery().ok_or_else(|| {
            self.inner
                .error(name, Failure::TriggerState("synchronous batch disappeared"))
        })?;
        result?;
        debug_assert_eq!(events.len(), count);
        self.begin_synchronous_events(events).map(Some)
    }

    fn plan_life(
        &self,
        name: &str,
        target: &Value,
        rest: &[Value],
    ) -> Result<Option<LifePlan>, Error> {
        if !runtime_signature(name, rest) {
            return Err(self.inner.error(name, Failure::Signature));
        }
        let id = match target {
            Value::Null => return Ok(None),
            Value::Handle(id) => *id,
            _ => return Err(self.inner.error(name, Failure::WrongFamily)),
        };
        let Ok(Entry::Destructable { creation, state }) = self.inner.entry(id) else {
            return Err(self.inner.error(name, Failure::WrongFamily));
        };
        let Some(max_life) = state.max_life else {
            return Err(self
                .inner
                .error(name, Failure::DestructableState("catalog HP not resolved")));
        };
        let ceiling = max_life.max(0.0);
        let (life, dead) = match (name, rest) {
            ("KillDestructable", []) => (0.0f32.min(max_life), true),
            ("SetDestructableLife", [Value::Real(value)]) => {
                let life = value.clamp(0.0, ceiling);
                (life, life <= 0.0)
            }
            ("DestructableRestoreLife", [Value::Real(value), Value::Bool(birth)]) => {
                if *birth {
                    return Err(self.inner.error(
                        name,
                        Failure::DestructableState("birth presentation is unsupported"),
                    ));
                }
                let policy = self.destructable_restore.ok_or_else(|| {
                    self.inner.error(
                        name,
                        Failure::DestructableState("restore source policy not admitted"),
                    )
                })?;
                if !state.dead {
                    return Ok(None);
                }
                if ceiling <= 0.0 {
                    return Err(self.inner.error(
                        name,
                        Failure::DestructableState("catalog HP is not positive"),
                    ));
                }
                (policy.life(*value, ceiling), false)
            }
            _ => return Err(self.inner.error(name, Failure::Signature)),
        };
        Ok(Some(LifePlan {
            id,
            origin: creation.origin,
            life,
            dead,
            was_dead: state.dead,
        }))
    }

    fn commit_life(&mut self, plan: LifePlan) {
        match self.inner.entry_mut(plan.id) {
            Ok(Entry::Destructable { state, .. }) => {
                state.life = Some(plan.life);
                state.dead = plan.dead;
            }
            _ => unreachable!("planned destructable entry"),
        }
        self.sync_destructable_collision(plan.id, plan.origin, !plan.dead);
    }

    fn death_event_count(&self, id: HandleId) -> usize {
        self.inner
            .arena
            .iter()
            .filter(|entry| matches!(entry, Entry::RegisteredDeathEvent(r) if r.widget == id))
            .count()
    }

    fn push_death_events(&mut self, id: HandleId, count: usize) {
        let event = self.destructable_death_event_handle();
        let mut index = 0;
        let mut pushed = 0;
        while index < self.inner.arena.len() && pushed < count {
            let trigger = match &self.inner.arena[index] {
                Entry::RegisteredDeathEvent(r) if r.widget == id => Some(r.trigger),
                _ => None,
            };
            if let Some(trigger) = trigger {
                self.push_reserved_event(
                    trigger,
                    None,
                    super::super::EventSubject {
                        destructable: Some(id),
                        ..Default::default()
                    },
                    event,
                );
                pushed += 1;
            }
            index += 1;
        }
        debug_assert_eq!(pushed, count);
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
        if DEATH_NATIVES.contains(&name) {
            let Some(plan) = self.plan_life(name, target, rest)? else {
                return Ok(Value::Null);
            };
            let count = if plan.dead && !plan.was_dead {
                self.death_event_count(id)
            } else {
                0
            };
            if count > 0 {
                self.reserve_event_delivery(count)?;
            }
            self.commit_life(plan);
            if count > 0 {
                self.push_death_events(id, count);
            }
            return Ok(Value::Null);
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
            self.sync_destructable_collision(id, origin, !self.destructable_dead(id, name)?);
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
    ) {
        let Some(placement) = self.unit_placement.as_mut() else {
            return;
        };
        match origin {
            DestructableOrigin::Placed { creation_number } => {
                placement.set_placed_active(creation_number, active);
            }
            DestructableOrigin::Script => {
                placement.set_destructable_active(id, active);
            }
        }
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
        "DestructableRestoreLife" => {
            matches!(rest, [Value::Real(value), Value::Bool(_)] if value.is_finite())
        }
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

#[cfg(test)]
mod restore_policy_tests {
    use super::*;
    use crate::pe_image::{
        TEXT,
        tests::{DATA, IMAGE_BASE, synthetic_image},
    };

    const BODY: u32 = IMAGE_BASE + 0x1000;
    const INIT: u32 = IMAGE_BASE + 0x1100;
    const THRESHOLD: u32 = IMAGE_BASE + 0x3000;
    const MINIMUM: u32 = IMAGE_BASE + 0x3004;

    fn image(zero: i8, threshold: f32, minimum: f32) -> (Vec<u8>, RestoreLifeBinding) {
        let mut text = [0x90; 0x110];
        let body = &mut text[..0x75];
        body[0x28] = 0x68;
        body[0x29..0x2d].copy_from_slice(&0x404000u32.to_le_bytes());
        body[0x2e] = 0xe8;
        body[0x2f..0x33].copy_from_slice(&(0x401800i32 - (BODY + 0x33) as i32).to_le_bytes());
        body[0x33..0x37].copy_from_slice(&[0xf3, 0x0f, 0x10, 0x05]);
        body[0x37..0x3b].copy_from_slice(&THRESHOLD.to_le_bytes());
        body[0x40..0x45].copy_from_slice(&[0x25, 0xff, 0xff, 0xff, 0x7f]);
        body[0x48..0x4e].copy_from_slice(&[0x0f, 0x2f, 0x45, 0x08, 0x76, 0x12]);
        body[0x65..0x68].copy_from_slice(&[0x0f, 0x2f, 0x05]);
        body[0x68..0x6c].copy_from_slice(&MINIMUM.to_le_bytes());
        body[0x6c] = 0xa1;
        body[0x6d..0x71].copy_from_slice(&MINIMUM.to_le_bytes());
        body[0x71..0x75].copy_from_slice(&[0x0f, 0x47, 0x45, 0xfc]);
        let init = &mut text[0x100..0x110];
        init[..3].copy_from_slice(&[0x6a, zero as u8, 0x68]);
        init[3..7].copy_from_slice(&0x404000u32.to_le_bytes());
        init[7] = 0xe8;
        init[8..12].copy_from_slice(&(0x401900i32 - (INIT + 12) as i32).to_le_bytes());
        init[12..].copy_from_slice(&[0x83, 0xc4, 0x08, 0xc3]);
        let mut data = [0; 8];
        data[..4].copy_from_slice(&threshold.to_le_bytes());
        data[4..].copy_from_slice(&minimum.to_le_bytes());
        let mut bytes = synthetic_image(&[(TEXT, 0x1000, &text), (DATA, 0x3000, &data)]);
        bytes[..2].copy_from_slice(b"MZ");
        bytes[0x84..0x86].copy_from_slice(&0x14cu16.to_le_bytes());
        bytes[0xd0..0xd4].copy_from_slice(&0x4000u32.to_le_bytes());
        bytes[0x19c..0x1a0].copy_from_slice(&0x6000_0020u32.to_le_bytes());
        bytes[0x1c4..0x1c8].copy_from_slice(&0xc000_0040u32.to_le_bytes());
        let binding = RestoreLifeBinding {
            origin: AssetIdentity {
                edition: "synthetic".into(),
                version: "synthetic".into(),
                source: "synthetic client".into(),
            },
            image_sha256: sha256(&bytes),
            body: SourceWindow {
                va: BODY,
                len: 0x75,
                digest: sha256(&text[..0x75]),
            },
            zero_initializer: SourceWindow {
                va: INIT,
                len: 16,
                digest: sha256(&text[0x100..]),
            },
            threshold: SourceWindow {
                va: THRESHOLD,
                len: 4,
                digest: sha256(&data[..4]),
            },
            minimum: SourceWindow {
                va: MINIMUM,
                len: 4,
                digest: sha256(&data[4..]),
            },
            subtract_va: 0x401800,
            convert_va: 0x401900,
        };
        (bytes, binding)
    }

    #[test]
    fn authored_threshold_and_floor_change_restore_amounts() {
        let (bytes, binding) = image(0, 0.125, 0.75);
        let policy = RestoreLifePolicy::read(
            &bytes,
            binding.clone(),
            RuleInputBudget {
                image_bytes: 1 << 20,
                profile_bytes: 1024,
                sections: 4,
            },
        )
        .unwrap();
        assert_eq!(policy.life(0.0625, 50.0), 50.0);
        assert_eq!(policy.life(0.25, 50.0), 0.75);
        assert_eq!(policy.life(-20.0, 50.0), 0.75);
        assert_eq!(policy.life(100.0, 50.0), 50.0);
        let (changed, _) = image(0, 0.25, 1.25);
        assert!(
            RestoreLifePolicy::read(
                &changed,
                binding,
                RuleInputBudget {
                    image_bytes: 1 << 20,
                    profile_bytes: 1024,
                    sections: 4
                }
            )
            .is_err()
        );
        let (changed, binding) = image(0, 0.25, 1.25);
        let changed_policy = RestoreLifePolicy::read(
            &changed,
            binding,
            RuleInputBudget {
                image_bytes: 1 << 20,
                profile_bytes: 1024,
                sections: 4,
            },
        )
        .unwrap();
        assert_eq!(changed_policy.life(0.2, 50.0), 50.0);
        assert_eq!(changed_policy.life(0.5, 50.0), 1.25);
        let (shifted, binding) = image(1, 0.125, 0.75);
        let shifted_policy = RestoreLifePolicy::read(
            &shifted,
            binding,
            RuleInputBudget {
                image_bytes: 1 << 20,
                profile_bytes: 1024,
                sections: 4,
            },
        )
        .unwrap();
        assert_eq!(shifted_policy.life(1.0625, 50.0), 50.0);
        assert_eq!(shifted_policy.life(0.0625, 50.0), 0.75);
    }

    #[test]
    fn restore_windows_consume_the_declared_profile_budget() {
        let (bytes, binding) = image(0, 0.125, 0.75);
        let needed = [
            &binding.body,
            &binding.zero_initializer,
            &binding.threshold,
            &binding.minimum,
        ]
        .iter()
        .map(|window| window.len as usize)
        .sum::<usize>();
        let budget = |profile_bytes| RuleInputBudget {
            image_bytes: 1 << 20,
            profile_bytes,
            sections: 4,
        };
        assert!(RestoreLifePolicy::read(&bytes, binding.clone(), budget(needed)).is_ok());
        assert!(
            RestoreLifePolicy::read(&bytes, binding, budget(needed - 1))
                .unwrap_err()
                .contains("restore windows exceed input budget")
        );
    }
}
