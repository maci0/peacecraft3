//! Enumeration-family display modes read from one selected client image.
//!
//! The supported profile is the admission compare, the following flag and kind
//! stores, the refresh-center replacement, the selection score and the two
//! label formats. A complete selector-2 popup caller also supplies pair families
//! and one appended full-monitor record. Operand values come from the image.

use crate::neutral_player::{NeutralError, RuleInputBudget};
use crate::neutral_rule_input::Pe;
use crate::scripts::AssetIdentity;

const STORE_WINDOW: usize = 96;
/// Resource ceiling for supplied host records and emitted popup rows.
pub const MAX_HOST_RECORDS: usize = 4096;
// Resource ceiling for pairs in one referenced table, not a measured table size.
const MAX_PAIRS: usize = 128;
/// `WS_POPUP` in the Win32 window-style definitions.
const WS_POPUP: u32 = 0x8000_0000;
/// `WS_EX_APPWINDOW` in the Win32 extended-style definitions.
const WS_EX_APPWINDOW: u32 = 0x0004_0000;

/// Explicit caller limits for one image. Immediate values are not limits.
#[derive(Clone, Copy)]
pub struct DiscoveryBudget {
    pub image: RuleInputBudget,
    /// Maximum inspected bytes of one directly referenced label, including NUL.
    pub label_bytes: usize,
    /// Successful profile-site matches before discovery stops.
    pub candidate_limit: usize,
    /// Scanned code, inspected labels and selected pair-table bytes. Includes
    /// the second code-range walk and each bounded prologue search.
    pub work_bytes: usize,
}

#[derive(Debug, thiserror::Error)]
#[error("Options display modes {source_context}: {problem}")]
pub struct Error {
    pub source_context: String,
    pub problem: Problem,
}

#[derive(Debug, thiserror::Error)]
pub enum Problem {
    #[error("input or selection budget exceeded")]
    Budget,
    #[error("missing source identity")]
    Identity,
    #[error("invalid PE image: {0}")]
    Image(#[source] NeutralError),
    #[error("no complete supported display-mode profile")]
    None,
    #[error("multiple supported display-mode profiles")]
    Ambiguous,
    #[error("display-mode operand is not usable")]
    Operand,
    #[error("display-mode label is not usable")]
    Label,
    #[error("display-mode arithmetic overflowed")]
    Overflow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostRecord {
    pub width: u32,
    pub height: u32,
    pub depth: Option<u32>,
    pub refresh_millihertz: Option<u32>,
    pub adapter: u32,
    pub mode_index: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub label: String,
    pub width: u32,
    pub height: u32,
    pub depth: Option<u32>,
    pub refresh_millihertz: Option<u32>,
    pub adapter: u32,
    pub mode_index: u32,
    pub kind: i32,
    pub flag: u8,
    /// Pass index of the monitor bound that produced a popup row.
    pub monitor_ordinal: Option<u32>,
}

/// One monitor's full rectangle for this listing pass.
/// `ordinal` is that pass index. It is not a session adapter token and it is
/// not a stable host identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MonitorBound {
    pub ordinal: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Query {
    pub width: i32,
    pub height: i32,
    pub depth: i32,
    pub refresh: i32,
    pub adapter: u32,
    pub kind: i32,
    pub flag: u8,
}

#[derive(Clone, Debug)]
pub struct Policy {
    origin: AssetIdentity,
    depth_a: u32,
    depth_b: u32,
    min_width: u32,
    kind: i32,
    flag: u8,
    center_hz: i32,
    sentinel: i32,
    wildcard: i32,
    adapter_penalty: u32,
    kind_penalty: u32,
    flag_penalty: u32,
    depth_penalty: u32,
    refresh_penalty: u32,
    depth_kind: i32,
    shift: u32,
    quant_max: u32,
    label_with_depth: String,
    label_size_only: String,
    label_limit: usize,
    popup: Option<PopupProfile>,
}

struct FilterSite {
    depth_a: u32,
    depth_b: u32,
    min_width: u32,
    flag: u8,
    kind: i32,
}

struct ScoreSite {
    sentinel: i32,
    wildcard: i32,
    adapter_penalty: u32,
    kind_penalty: u32,
    flag_penalty: u32,
    shift: u32,
    quant_max: u32,
    depth_kind: i32,
    depth_penalty: u32,
    refresh_penalty: u32,
}

#[derive(PartialEq, Eq)]
struct LabelSite {
    with_depth: String,
    size_only: String,
    limit: usize,
}

struct Cur<'a> {
    bytes: &'a [u8],
    index: usize,
}

impl<'a> Cur<'a> {
    fn rest(&self) -> &'a [u8] {
        &self.bytes[self.index..]
    }
    fn take(&mut self, len: usize) -> Option<&'a [u8]> {
        let out = self.rest().get(..len)?;
        self.index += len;
        Some(out)
    }
    fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    fn i32(&mut self) -> Option<i32> {
        Some(i32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    fn expect(&mut self, prefix: &[u8]) -> Option<()> {
        if self.rest().starts_with(prefix) {
            self.index += prefix.len();
            Some(())
        } else {
            None
        }
    }
    fn rel8(&mut self, opcode: u8) -> Option<isize> {
        if self.rest().first().copied() != Some(opcode) {
            return None;
        }
        self.index += 1;
        let rel = self.u8()? as i8;
        Some(self.index as isize + isize::from(rel))
    }
}

impl Policy {
    pub fn discover(
        image: &[u8],
        origin: AssetIdentity,
        budget: DiscoveryBudget,
    ) -> Result<Self, Error> {
        let source_context = format!(
            "{} | {} | {}",
            origin.edition, origin.version, origin.source
        );
        let fail = |problem| Error {
            source_context: source_context.clone(),
            problem,
        };
        if image.len() > budget.image.image_bytes
            || budget.label_bytes == 0
            || budget.candidate_limit == 0
            || budget.work_bytes == 0
            || budget.image.profile_bytes == 0
        {
            return Err(fail(Problem::Budget));
        }
        if [&origin.edition, &origin.version, &origin.source]
            .iter()
            .any(|text| text.trim().is_empty())
        {
            return Err(fail(Problem::Identity));
        }
        let pe = Pe::parse(image, budget.image).map_err(|error| fail(Problem::Image(error)))?;
        let mut work = 0usize;
        let mut selected = 0usize;
        let mut candidates = 0usize;
        let mut filter = None;
        let mut center = None;
        let mut score = None;
        let mut label = None;
        let mut bounds = Vec::new();
        for (code_va, code) in pe.code_ranges() {
            charge(&mut work, code.len(), budget.work_bytes).map_err(fail)?;
            for offset in 0..code.len() {
                let tail = &code[offset..];
                let fail_at = |problem, family| Error {
                    source_context: format!(
                        "{source_context}; {family} site {:#x}",
                        u64::from(code_va) + offset as u64
                    ),
                    problem,
                };
                if let Some(site) = match_filter(tail) {
                    admit(&mut candidates, budget.candidate_limit, filter.is_some())
                        .map_err(|problem| fail_at(problem, "enumeration"))?;
                    filter = Some(site);
                }
                if let Some(site) = match_refresh(tail) {
                    admit(&mut candidates, budget.candidate_limit, center.is_some())
                        .map_err(|problem| fail_at(problem, "refresh"))?;
                    center = Some(site);
                }
                if let Some(site) = match_score(tail) {
                    admit(&mut candidates, budget.candidate_limit, score.is_some())
                        .map_err(|problem| fail_at(problem, "selection score"))?;
                    score = Some(site);
                }
                match match_label(tail, &pe, budget, &mut work, &mut selected) {
                    Ok(Some(site)) => {
                        admit(
                            &mut candidates,
                            budget.candidate_limit,
                            label.as_ref().is_some_and(|previous| previous != &site),
                        )
                        .map_err(|problem| fail_at(problem, "label"))?;
                        label = Some(site);
                    }
                    Ok(None) => {}
                    Err(problem) => return Err(fail_at(problem, "label")),
                }
                let hit = match_bounds(code, offset, &mut work, budget)
                    .map_err(|problem| fail_at(problem, "popup bounds"))?;
                if let Some(hit) = hit {
                    let Some(entry) = u32::try_from(hit.entry)
                        .ok()
                        .and_then(|at| code_va.checked_add(at))
                    else {
                        continue;
                    };
                    admit(&mut candidates, budget.candidate_limit, false)
                        .map_err(|problem| fail_at(problem, "popup bounds"))?;
                    bounds
                        .try_reserve(1)
                        .map_err(|_| fail_at(Problem::Budget, "popup bounds"))?;
                    bounds.push(BoundsHit {
                        entry,
                        selector: hit.selector,
                    });
                }
            }
        }
        let popup = link_popup(&pe, budget, &mut work, &mut selected, &bounds).map_err(fail)?;
        let (Some(filter), Some(center_hz), Some(score), Some(label)) =
            (filter, center, score, label)
        else {
            return Err(fail(Problem::None));
        };
        Ok(Self {
            origin,
            depth_a: filter.depth_a,
            depth_b: filter.depth_b,
            min_width: filter.min_width,
            kind: filter.kind,
            flag: filter.flag,
            center_hz,
            sentinel: score.sentinel,
            wildcard: score.wildcard,
            adapter_penalty: score.adapter_penalty,
            kind_penalty: score.kind_penalty,
            flag_penalty: score.flag_penalty,
            depth_penalty: score.depth_penalty,
            refresh_penalty: score.refresh_penalty,
            depth_kind: score.depth_kind,
            shift: score.shift,
            quant_max: score.quant_max,
            label_with_depth: label.with_depth,
            label_size_only: label.size_only,
            label_limit: label.limit,
            popup,
        })
    }

    pub fn has_popup_profile(&self) -> bool {
        self.popup.is_some()
    }

    /// Shared kind of the flag-0 popup rows, including the appended full-monitor
    /// record when its flag is 0. This is a recreation presentation choice for
    /// render-extent selection. The active-kind compare and the adapter-owner
    /// field stay unread.
    pub fn popup_presentation_kind(&self) -> Option<i32> {
        let popup = self.popup.as_ref()?;
        let mut kind = None;
        for family in popup.families.iter().filter(|family| family.flag == 0) {
            match kind {
                None => kind = Some(family.kind),
                Some(previous) if previous == family.kind => {}
                Some(_) => return None,
            }
        }
        if let Some(full) = popup.full.filter(|full| full.flag == 0) {
            match kind {
                None => kind = Some(full.kind),
                Some(previous) if previous == full.kind => {}
                Some(_) => return None,
            }
        }
        kind
    }

    pub fn origin(&self) -> &AssetIdentity {
        &self.origin
    }
    pub fn kind(&self) -> i32 {
        self.kind
    }
    pub fn flag(&self) -> u8 {
        self.flag
    }
    pub fn sentinel(&self) -> i32 {
        self.sentinel
    }
    pub fn wildcard(&self) -> i32 {
        self.wildcard
    }

    /// Filters and groups host records. `required_flag` is the absent-checkbox
    /// value when supplied. An empty input or a flag mismatch yields no rows.
    pub fn rows(
        &self,
        records: &[HostRecord],
        required_flag: Option<u8>,
    ) -> Result<Vec<Row>, Error> {
        if required_flag.is_some_and(|flag| flag != self.flag) || records.is_empty() {
            return Ok(Vec::new());
        }
        if records.len() > MAX_HOST_RECORDS {
            return Err(self.err(Problem::Budget));
        }
        let mut groups: Vec<Group> = Vec::new();
        groups
            .try_reserve(records.len())
            .map_err(|_| self.err(Problem::Budget))?;
        for record in records {
            if record.width == 0
                || record.height == 0
                || record.width < self.min_width
                || record
                    .depth
                    .is_some_and(|depth| depth != self.depth_a && depth != self.depth_b)
            {
                continue;
            }
            if record.width > i32::MAX as u32 || record.height > i32::MAX as u32 {
                return Err(self.err(Problem::Overflow));
            }
            if i32::try_from(record.adapter).is_err() {
                return Err(self.err(Problem::Overflow));
            }
            let refresh = record.refresh_millihertz.filter(|hz| *hz != 0);
            let distance = refresh
                .map(|hz| self.refresh_distance(hz))
                .transpose()
                .map_err(|problem| self.err(problem))?;
            let key = (
                record.width,
                record.height,
                record.depth,
                record.adapter,
                self.kind,
                self.flag,
            );
            if let Some(group) = groups.iter_mut().find(|group| group.key == key) {
                let replace = match (distance, group.distance) {
                    (Some(new), Some(old)) => new < old,
                    (Some(_), None) => true,
                    _ => false,
                };
                if replace {
                    group.refresh = refresh;
                    group.distance = distance;
                    group.mode_index = record.mode_index;
                }
            } else {
                groups.push(Group {
                    key,
                    refresh,
                    distance,
                    mode_index: record.mode_index,
                });
            }
        }
        let mut rows = Vec::new();
        rows.try_reserve_exact(groups.len())
            .map_err(|_| self.err(Problem::Budget))?;
        for group in groups {
            let (width, height, depth, adapter, kind, flag) = group.key;
            let with_depth = depth.is_some_and(|value| value != 0);
            let template = if with_depth {
                self.label_with_depth.as_str()
            } else {
                self.label_size_only.as_str()
            };
            let mut values = vec![
                signed(width).map_err(|problem| self.err(problem))?,
                signed(height).map_err(|problem| self.err(problem))?,
            ];
            if with_depth {
                values.push(signed(depth.unwrap_or(0)).map_err(|problem| self.err(problem))?);
            }
            let label = format_label(template, &values, self.label_limit)
                .map_err(|problem| self.err(problem))?;
            rows.push(Row {
                label,
                width,
                height,
                depth,
                refresh_millihertz: group.refresh,
                adapter,
                mode_index: group.mode_index,
                kind,
                flag,
                monitor_ordinal: None,
            });
        }
        Ok(rows)
    }

    /// Selector-2 pair families in caller order, then the appended record when
    /// its capture-to-record shape validated. That record is the supplied full
    /// monitor bound. `required_flag` keeps the absent-checkbox value when set.
    /// A non-positive monitor adds no rows. One bound is not applied to any
    /// other source family. Table offsets identify session rows, not exclusive
    /// host modes. Equal source keys keep their first occurrence.
    pub fn popup_rows(
        &self,
        monitors: &[MonitorBound],
        required_flag: Option<u8>,
        session_adapter: u32,
    ) -> Result<Vec<Row>, Error> {
        let Some(popup) = &self.popup else {
            return Ok(Vec::new());
        };
        if monitors.len() > MAX_HOST_RECORDS {
            return Err(self.err(Problem::Budget));
        }
        i32::try_from(session_adapter).map_err(|_| self.err(Problem::Overflow))?;
        let mut rows = Vec::new();
        for monitor in monitors {
            if monitor.width == 0 || monitor.height == 0 {
                continue;
            }
            let width = signed(monitor.width).map_err(|problem| self.err(problem))?;
            let height = signed(monitor.height).map_err(|problem| self.err(problem))?;
            for family in &popup.families {
                if required_flag.is_some_and(|flag| flag != family.flag) {
                    continue;
                }
                for (index, pair) in family.pairs.iter().enumerate() {
                    if pair.0 > width || pair.1 > height {
                        continue;
                    }
                    let mode_index =
                        u32::try_from(index).map_err(|_| self.err(Problem::Overflow))?;
                    self.push_labeled(
                        &mut rows,
                        LabeledParts {
                            width: pair.0,
                            height: pair.1,
                            adapter: session_adapter,
                            mode_index,
                            kind: family.kind,
                            flag: family.flag,
                            monitor_ordinal: Some(monitor.ordinal),
                        },
                    )?;
                }
            }
            if let Some(full) = popup.full
                && !required_flag.is_some_and(|flag| flag != full.flag)
            {
                self.push_labeled(
                    &mut rows,
                    LabeledParts {
                        width,
                        height,
                        adapter: session_adapter,
                        mode_index: 0,
                        kind: full.kind,
                        flag: full.flag,
                        monitor_ordinal: Some(monitor.ordinal),
                    },
                )?;
            }
        }
        Ok(rows)
    }

    fn push_labeled(&self, rows: &mut Vec<Row>, parts: LabeledParts) -> Result<(), Error> {
        let width = u32::try_from(parts.width).map_err(|_| self.err(Problem::Overflow))?;
        let height = u32::try_from(parts.height).map_err(|_| self.err(Problem::Overflow))?;
        // The converter groups dimensions/depth/adapter/kind/flag, excluding mode index.
        if rows.iter().any(|row| {
            row.width == width
                && row.height == height
                && row.depth.is_none()
                && row.adapter == parts.adapter
                && row.kind == parts.kind
                && row.flag == parts.flag
                && row.monitor_ordinal == parts.monitor_ordinal
        }) {
            return Ok(());
        }
        if rows.len() >= MAX_HOST_RECORDS {
            return Err(self.err(Problem::Budget));
        }
        rows.try_reserve(1).map_err(|_| self.err(Problem::Budget))?;
        rows.push(self.labeled_row(parts)?);
        Ok(())
    }

    fn labeled_row(&self, parts: LabeledParts) -> Result<Row, Error> {
        let width = u32::try_from(parts.width).map_err(|_| self.err(Problem::Overflow))?;
        let height = u32::try_from(parts.height).map_err(|_| self.err(Problem::Overflow))?;
        let label = format_label(
            self.label_size_only.as_str(),
            &[
                signed(width).map_err(|problem| self.err(problem))?,
                signed(height).map_err(|problem| self.err(problem))?,
            ],
            self.label_limit,
        )
        .map_err(|problem| self.err(problem))?;
        Ok(Row {
            label,
            width,
            height,
            depth: None,
            refresh_millihertz: None,
            adapter: parts.adapter,
            mode_index: parts.mode_index,
            kind: parts.kind,
            flag: parts.flag,
            monitor_ordinal: parts.monitor_ordinal,
        })
    }

    /// Lowest unsigned score wins. Equal scores keep the earlier row.
    /// A query adapter equal to the resolver sentinel is refused.
    pub fn select(&self, rows: &[Row], query: Query) -> Result<Option<usize>, Error> {
        if rows.is_empty() {
            return Ok(None);
        }
        let query_adapter =
            i32::try_from(query.adapter).map_err(|_| self.err(Problem::Overflow))?;
        if query_adapter == self.sentinel {
            return Err(self.err(Problem::Operand));
        }
        let mut best: Option<(usize, u32)> = None;
        for (index, row) in rows.iter().enumerate() {
            let score = self
                .score(row, &query, query_adapter)
                .map_err(|problem| self.err(problem))?;
            match best {
                Some((_, previous)) if score >= previous => {}
                _ => best = Some((index, score)),
            }
        }
        Ok(best.map(|(index, _)| index))
    }

    fn err(&self, problem: Problem) -> Error {
        Error {
            source_context: format!(
                "{} | {} | {}",
                self.origin.edition, self.origin.version, self.origin.source
            ),
            problem,
        }
    }

    fn refresh_distance(&self, millihertz: u32) -> Result<i64, Problem> {
        let center = i64::from(self.center_hz)
            .checked_mul(1_000)
            .ok_or(Problem::Overflow)?;
        i64::from(millihertz)
            .checked_sub(center)
            .ok_or(Problem::Overflow)?
            .checked_abs()
            .ok_or(Problem::Overflow)
    }

    fn score(&self, row: &Row, query: &Query, query_adapter: i32) -> Result<u32, Problem> {
        let mut score = 0u32;
        let row_adapter = i32::try_from(row.adapter).map_err(|_| Problem::Overflow)?;
        if row_adapter != self.wildcard
            && query_adapter != self.wildcard
            && row_adapter != query_adapter
        {
            score = self.adapter_penalty;
        }
        if row.kind != query.kind {
            score = score
                .checked_add(self.kind_penalty)
                .ok_or(Problem::Overflow)?;
        }
        if row.flag != query.flag {
            score = score
                .checked_add(self.flag_penalty)
                .ok_or(Problem::Overflow)?;
        }
        score = score
            .checked_add(self.dimension_penalty(row, query)?)
            .ok_or(Problem::Overflow)?;
        if row.kind == self.depth_kind {
            let depth = match row.depth {
                None => 0,
                Some(value) => i32::try_from(value).map_err(|_| Problem::Overflow)?,
            };
            if depth != query.depth {
                score = score
                    .checked_add(self.depth_penalty)
                    .ok_or(Problem::Overflow)?;
            }
            let refresh = scored_refresh(row.refresh_millihertz);
            if refresh != 0 && query.refresh != 0 && refresh != query.refresh {
                score = score
                    .checked_add(self.refresh_penalty)
                    .ok_or(Problem::Overflow)?;
            }
        }
        Ok(score)
    }

    fn dimension_penalty(&self, row: &Row, query: &Query) -> Result<u32, Problem> {
        let width = i32::try_from(row.width).map_err(|_| Problem::Overflow)?;
        let height = i32::try_from(row.height).map_err(|_| Problem::Overflow)?;
        let delta = i64::from(width)
            .abs_diff(i64::from(query.width))
            .checked_add(i64::from(height).abs_diff(i64::from(query.height)))
            .ok_or(Problem::Overflow)?;
        if delta > 0x7fff_ffff {
            return Err(Problem::Overflow);
        }
        if delta == 0 {
            return Ok(0);
        }
        let stepped = u32::try_from(delta)
            .map_err(|_| Problem::Overflow)?
            .checked_shr(self.shift)
            .ok_or(Problem::Overflow)?
            .checked_add(1)
            .ok_or(Problem::Overflow)?;
        stepped
            .min(self.quant_max)
            .checked_shl(self.shift)
            .ok_or(Problem::Overflow)
    }
}

struct Group {
    key: (u32, u32, Option<u32>, u32, i32, u8),
    refresh: Option<u32>,
    distance: Option<i64>,
    mode_index: u32,
}

fn scored_refresh(refresh: Option<u32>) -> i32 {
    match refresh {
        Some(millihertz) if millihertz % 1000 == 0 => i32::try_from(millihertz / 1000).unwrap_or(0),
        _ => 0,
    }
}

fn signed(value: u32) -> Result<i32, Problem> {
    i32::try_from(value).map_err(|_| Problem::Overflow)
}

fn charge(total: &mut usize, bytes: usize, limit: usize) -> Result<(), Problem> {
    *total = total
        .checked_add(bytes)
        .filter(|sum| *sum <= limit)
        .ok_or(Problem::Budget)?;
    Ok(())
}

fn admit(count: &mut usize, limit: usize, already: bool) -> Result<(), Problem> {
    *count = count
        .checked_add(1)
        .filter(|sum| *sum <= limit)
        .ok_or(Problem::Budget)?;
    if already {
        Err(Problem::Ambiguous)
    } else {
        Ok(())
    }
}

fn positive_i32(value: i32) -> Option<u32> {
    u32::try_from(value).ok().filter(|value| *value > 0)
}

fn cmp_imm(cursor: &mut Cur<'_>, short: u8, long: u8) -> Option<i32> {
    if cursor.rest().starts_with(&[0x83, short]) {
        cursor.index += 2;
        Some(i32::from(cursor.u8()? as i8))
    } else if cursor.rest().starts_with(&[0x81, long]) {
        cursor.index += 2;
        cursor.i32()
    } else {
        None
    }
}

fn sub_eax(cursor: &mut Cur<'_>) -> Option<i32> {
    if cursor.rest().starts_with(&[0x83, 0xe8]) {
        cursor.index += 2;
        let value = i32::from(cursor.u8()? as i8);
        (value > 0).then_some(value)
    } else if cursor.rest().first() == Some(&0x2d) {
        cursor.index += 1;
        let value = cursor.i32()?;
        (value > 0).then_some(value)
    } else {
        None
    }
}

fn add_esi(cursor: &mut Cur<'_>) -> Option<u32> {
    if cursor.rest().starts_with(&[0x83, 0xc6]) {
        cursor.index += 2;
        positive_i32(i32::from(cursor.u8()? as i8))
    } else if cursor.rest().starts_with(&[0x81, 0xc6]) {
        cursor.index += 2;
        let value = cursor.u32()?;
        (value != 0).then_some(value)
    } else {
        None
    }
}

fn lea_esi(cursor: &mut Cur<'_>) -> Option<u32> {
    if cursor.rest().starts_with(&[0x8d, 0x46]) {
        cursor.index += 2;
        positive_i32(i32::from(cursor.u8()? as i8))
    } else if cursor.rest().starts_with(&[0x8d, 0x86]) {
        cursor.index += 2;
        positive_i32(cursor.i32()?)
    } else {
        None
    }
}

fn match_filter(bytes: &[u8]) -> Option<FilterSite> {
    let mut cursor = Cur { bytes, index: 0 };
    cursor.expect(&[0x8b, 0x4d, 0xc8])?;
    let depth_a = positive_i32(cmp_imm(&mut cursor, 0xf9, 0xf9)?)?;
    let je_land = cursor.rel8(0x74)?;
    let depth_b = positive_i32(cmp_imm(&mut cursor, 0xf9, 0xf9)?)?;
    if depth_a == depth_b {
        return None;
    }
    let jne_land = cursor.rel8(0x75)?;
    if je_land != cursor.index as isize {
        return None;
    }
    cursor.expect(&[0x8b, 0x55, 0xcc])?;
    let min_width = positive_i32(cmp_imm(&mut cursor, 0xfa, 0xfa)?)?;
    let jb_land = cursor.rel8(0x72)?;
    if jb_land != jne_land {
        return None;
    }
    let window = cursor.rest().get(..cursor.rest().len().min(STORE_WINDOW))?;
    let (flag, kind) = flag_and_kind(window)?;
    Some(FilterSite {
        depth_a,
        depth_b,
        min_width,
        flag,
        kind,
    })
}

fn flag_and_kind(window: &[u8]) -> Option<(u8, i32)> {
    let flag_prefix = [0xc6, 0x85, 0xb0, 0xfd, 0xff, 0xff];
    let kind_prefix = [0xc7, 0x85, 0xa4, 0xfd, 0xff, 0xff];
    let mut flag = None;
    let mut kind = None;
    let mut index = 0;
    while index < window.len() {
        if window[index..].starts_with(&flag_prefix) && index + 7 <= window.len() {
            if flag.is_some() {
                return None;
            }
            flag = Some((index, window[index + 6]));
            index += 7;
            continue;
        }
        if window[index..].starts_with(&kind_prefix) && index + 10 <= window.len() {
            if kind.is_some() {
                return None;
            }
            let value = i32::from_le_bytes(window[index + 6..index + 10].try_into().ok()?);
            kind = Some((index, value));
            index += 10;
            continue;
        }
        index += 1;
    }
    let (flag_at, flag) = flag?;
    let (kind_at, kind) = kind?;
    (flag_at < kind_at).then_some((flag, kind))
}

fn match_refresh(bytes: &[u8]) -> Option<i32> {
    let mut cursor = Cur { bytes, index: 0 };
    cursor.expect(&[0x8b, 0x43, 0x20])?;
    let center = sub_eax(&mut cursor)?;
    cursor.expect(&[0x99, 0x8b, 0xc8, 0x8b, 0x46, 0x20, 0x33, 0xca])?;
    if sub_eax(&mut cursor)? != center {
        return None;
    }
    cursor.expect(&[0x2b, 0xca, 0x99, 0x33, 0xc2, 0x2b, 0xc2, 0x3b, 0xc1])?;
    cursor.rel8(0x7d)?;
    Some(center)
}

fn match_score(bytes: &[u8]) -> Option<ScoreSite> {
    let mut cursor = Cur { bytes, index: 0 };
    cursor.expect(&[0x3d])?;
    let sentinel = cursor.i32()?;
    let skip_call = cursor.rel8(0x75)?;
    cursor.expect(&[0xe8])?;
    cursor.take(4)?;
    if skip_call != cursor.index as isize {
        return None;
    }
    cursor.expect(&[0x8b, 0x0f, 0x81, 0xf9])?;
    let wildcard = cursor.i32()?;
    let skip_adapter_ecx = cursor.rel8(0x74)?;
    cursor.expect(&[0x3d])?;
    if cursor.i32()? != wildcard {
        return None;
    }
    let skip_adapter_eax = cursor.rel8(0x74)?;
    cursor.expect(&[0x3b, 0xc8, 0xba])?;
    let adapter_penalty = cursor.u32()?;
    cursor.expect(&[0x0f, 0x45, 0xf2])?;
    if skip_adapter_ecx != cursor.index as isize || skip_adapter_eax != cursor.index as isize {
        return None;
    }
    cursor.expect(&[0x8b, 0x4f, 0x18, 0x3b, 0x4d, 0xf0])?;
    let skip_kind = cursor.rel8(0x74)?;
    cursor.expect(&[0x81, 0xc6])?;
    let kind_penalty = cursor.u32()?;
    if kind_penalty == 0 || skip_kind != cursor.index as isize {
        return None;
    }
    cursor.expect(&[0x8a, 0x47, 0x24, 0x3a, 0x45, 0xfc])?;
    let skip_flag = cursor.rel8(0x74)?;
    cursor.expect(&[0x81, 0xc6])?;
    let flag_penalty = cursor.u32()?;
    if flag_penalty == 0 || skip_flag != cursor.index as isize {
        return None;
    }
    cursor.expect(&[
        0x8b, 0x47, 0x0c, 0x2b, 0x45, 0xe4, 0x99, 0x53, 0x8b, 0xd8, 0x8b, 0x47, 0x08, 0x2b, 0x45,
        0xe0, 0x33, 0xda, 0x2b, 0xda, 0x99, 0x33, 0xc2, 0x2b, 0xc2, 0x03, 0xd8, 0x85, 0xdb,
    ])?;
    let skip_quant = cursor.rel8(0x7e)?;
    cursor.expect(&[0x8b, 0xc3, 0x99, 0x83, 0xe2])?;
    let mask = u32::from(cursor.u8()?);
    cursor.expect(&[0x03, 0xc2, 0xba])?;
    let quant_max = cursor.u32()?;
    cursor.expect(&[0xc1, 0xf8])?;
    let shift = u32::from(cursor.u8()?);
    cursor.expect(&[0x40, 0x3b, 0xc2, 0x0f, 0x4f, 0xc2, 0xc1, 0xe0])?;
    let shift_back = u32::from(cursor.u8()?);
    cursor.expect(&[0x03, 0xf0])?;
    if skip_quant != cursor.index as isize {
        return None;
    }
    cursor.expect(&[0x5b])?;
    if shift != shift_back || shift >= 32 || quant_max == 0 || quant_max > i32::MAX as u32 {
        return None;
    }
    let expected_mask = 1u32.checked_shl(shift)?.wrapping_sub(1);
    if mask != expected_mask {
        return None;
    }
    let depth_kind = if cursor.rest().starts_with(&[0x83, 0xe9]) {
        cursor.index += 2;
        i32::from(cursor.u8()? as i8)
    } else if cursor.rest().starts_with(&[0x81, 0xe9]) {
        cursor.index += 2;
        cursor.i32()?
    } else {
        return None;
    };
    let skip_depth = cursor.rel8(0x75)?;
    cursor.expect(&[0x8b, 0x47, 0x1c, 0x3b, 0x45, 0xf4])?;
    let skip_depth_add = cursor.rel8(0x74)?;
    let depth_penalty = add_esi(&mut cursor)?;
    if skip_depth_add != cursor.index as isize {
        return None;
    }
    cursor.expect(&[0x8b, 0x4f, 0x20, 0x85, 0xc9])?;
    let skip_candidate_refresh = cursor.rel8(0x74)?;
    cursor.expect(&[0x8b, 0x45, 0xf8, 0x85, 0xc0])?;
    let skip_query_refresh = cursor.rel8(0x74)?;
    cursor.expect(&[0x3b, 0xc8])?;
    let refresh_penalty = lea_esi(&mut cursor)?;
    let unequal_refresh = cursor.rel8(0x75)?;
    let return_score = cursor.index as isize;
    cursor.expect(&[0x8b, 0xc6])?;
    if skip_depth != return_score
        || skip_candidate_refresh != return_score
        || skip_query_refresh != return_score
        || unequal_refresh != cursor.index as isize
    {
        return None;
    }
    Some(ScoreSite {
        sentinel,
        wildcard,
        adapter_penalty,
        kind_penalty,
        flag_penalty,
        shift,
        quant_max,
        depth_kind,
        depth_penalty,
        refresh_penalty,
    })
}

fn match_label(
    bytes: &[u8],
    pe: &Pe<'_>,
    budget: DiscoveryBudget,
    work: &mut usize,
    selected: &mut usize,
) -> Result<Option<LabelSite>, Problem> {
    let Some(pushes) = label_pushes(bytes) else {
        return Ok(None);
    };
    let Some(with_depth) = cstring(pe, pushes.with_depth, budget, work, selected)? else {
        return Ok(None);
    };
    let Some(size_only) = cstring(pe, pushes.size_only, budget, work, selected)? else {
        return Ok(None);
    };
    if !valid_template(&with_depth, 3) || !valid_template(&size_only, 2) {
        return Ok(None);
    }
    let Ok(with_depth) = String::from_utf8(with_depth) else {
        return Ok(None);
    };
    let Ok(size_only) = String::from_utf8(size_only) else {
        return Ok(None);
    };
    Ok(Some(LabelSite {
        with_depth,
        size_only,
        limit: pushes.limit,
    }))
}

struct LabelPushes {
    with_depth: u32,
    size_only: u32,
    limit: usize,
}

fn label_pushes(bytes: &[u8]) -> Option<LabelPushes> {
    let mut cursor = Cur { bytes, index: 0 };
    cursor.expect(&[0x8b, 0x48, 0x1c, 0x85, 0xc9])?;
    let short_path = cursor.rel8(0x74)?;
    cursor.expect(&[
        0x51, 0xff, 0x70, 0x0c, 0xff, 0x70, 0x08, 0x8d, 0x45, 0xbc, 0x68,
    ])?;
    let with_depth = cursor.u32()?;
    cursor.expect(&[0x6a])?;
    let limit = cursor.u8()?;
    cursor.expect(&[0x50, 0xe8])?;
    cursor.take(4)?;
    cursor.expect(&[0x83, 0xc4, 0x18])?;
    let after_long = cursor.rel8(0xeb)?;
    if short_path != cursor.index as isize || limit == 0 {
        return None;
    }
    cursor.expect(&[0xff, 0x70, 0x0c, 0xff, 0x70, 0x08, 0x8d, 0x45, 0xbc, 0x68])?;
    let size_only = cursor.u32()?;
    cursor.expect(&[0x6a])?;
    let short_limit = cursor.u8()?;
    if short_limit != limit {
        return None;
    }
    cursor.expect(&[0x50, 0xe8])?;
    cursor.take(4)?;
    cursor.expect(&[0x83, 0xc4, 0x14])?;
    if after_long != cursor.index as isize {
        return None;
    }
    Some(LabelPushes {
        with_depth,
        size_only,
        limit: usize::from(limit),
    })
}

fn cstring(
    pe: &Pe<'_>,
    va: u32,
    budget: DiscoveryBudget,
    work: &mut usize,
    selected: &mut usize,
) -> Result<Option<Vec<u8>>, Problem> {
    let Ok(tail) = pe.read_from(va, false) else {
        return Ok(None);
    };
    for (offset, byte) in tail.iter().take(budget.label_bytes).enumerate() {
        charge(selected, 1, budget.image.profile_bytes)?;
        charge(work, 1, budget.work_bytes)?;
        if *byte == 0 {
            return Ok(Some(tail[..offset].to_vec()));
        }
    }
    if tail.len() >= budget.label_bytes {
        Err(Problem::Budget)
    } else {
        Ok(None)
    }
}

fn valid_template(bytes: &[u8], arity: usize) -> bool {
    let mut index = 0;
    let mut count = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if bytes.get(index + 1) != Some(&b'd') {
                return false;
            }
            count += 1;
            index += 2;
        } else if (0x20..0x7f).contains(&bytes[index]) {
            index += 1;
        } else {
            return false;
        }
    }
    count == arity && !bytes.is_empty()
}

fn format_label(template: &str, values: &[i32], limit: usize) -> Result<String, Problem> {
    let bytes = template.as_bytes();
    let mut out = String::new();
    let mut index = 0;
    let mut used = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let Some(value) = values.get(used) else {
                return Err(Problem::Label);
            };
            out.push_str(&value.to_string());
            used += 1;
            index += 2;
        } else {
            out.push(char::from(bytes[index]));
            index += 1;
        }
    }
    if used != values.len() || out.len().checked_add(1).is_none_or(|len| len > limit) {
        return Err(Problem::Label);
    }
    Ok(out)
}

struct LabeledParts {
    width: i32,
    height: i32,
    adapter: u32,
    mode_index: u32,
    kind: i32,
    flag: u8,
    monitor_ordinal: Option<u32>,
}

struct BoundsMatch {
    entry: usize,
    selector: u8,
}

struct BoundsHit {
    entry: u32,
    selector: u8,
}

struct ParsedFamily {
    start: u32,
    end: u32,
    kind: i32,
    flag: u8,
}

struct ParsedCaller {
    families: [ParsedFamily; 2],
    full: Option<FullRecord>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FullRecord {
    kind: i32,
    flag: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PopupFamily {
    pairs: Vec<(i32, i32)>,
    kind: i32,
    flag: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PopupProfile {
    families: Vec<PopupFamily>,
    full: Option<FullRecord>,
}

fn match_bounds(
    code: &[u8],
    offset: usize,
    work: &mut usize,
    budget: DiscoveryBudget,
) -> Result<Option<BoundsMatch>, Problem> {
    let Some(selector) = popup_selector(code, offset) else {
        return Ok(None);
    };
    let Some(entry) = bounds_entry(code, offset, work, budget)? else {
        return Ok(None);
    };
    Ok(Some(BoundsMatch { entry, selector }))
}

fn popup_selector(code: &[u8], offset: usize) -> Option<u8> {
    let bytes = code.get(offset..)?;
    let mut cursor = Cur { bytes, index: 0 };
    cursor.expect(&[0x8b, 0x75, 0x08, 0x33, 0xc9, 0x8b, 0xc6, 0x33, 0xd2])?;
    cursor.expect(&[0x83, 0xe8, 0x01])?;
    let style1_at = cursor.rel8(0x74)?;
    cursor.expect(&[0x83, 0xe8, 0x01])?;
    let style2_at = cursor.rel8(0x74)?;
    cursor.expect(&[0x83, 0xe8, 0x01])?;
    let past_at = cursor.rel8(0x75)?;
    cursor.expect(&[0xba])?;
    let style3 = cursor.u32()?;
    let join_from_3 = cursor.rel8(0xeb)?;
    if cursor.index as isize != style2_at {
        return None;
    }
    cursor.expect(&[0xba])?;
    let style2 = cursor.u32()?;
    let join_from_2 = cursor.rel8(0xeb)?;
    if cursor.index as isize != style1_at {
        return None;
    }
    cursor.expect(&[0xba])?;
    let style1 = cursor.u32()?;
    if cursor.index as isize != join_from_3 || cursor.index as isize != join_from_2 {
        return None;
    }
    cursor.expect(&[0xb9])?;
    let exstyle = cursor.u32()?;
    if cursor.index as isize != past_at || exstyle != WS_EX_APPWINDOW {
        return None;
    }
    cursor.expect(&[
        0x51, 0x6a, 0x00, 0x52, 0x8d, 0x45, 0xec, 0x0f, 0x57, 0xc0, 0x50, 0x0f, 0x11, 0x45, 0xec,
        0xff, 0x15,
    ])?;
    cursor.take(4)?;
    let rest = cursor.rest().get(..96)?;
    // `mov esi, [ebp+0xc]` sits between `cmp esi, imm8` and `sete` and leaves
    // flags unchanged, so the immediate is still the work-rectangle selector.
    let sete = rest.windows(8).position(|chunk| {
        chunk[0] == 0x83
            && chunk[1] == 0xfe
            && chunk[3] == 0x8b
            && chunk[4] == 0x75
            && chunk[5] == 0x0c
            && chunk[6] == 0x0f
            && chunk[7] == 0x94
    })?;
    let work = rest[sete + 2];
    let styles = [style1, style2, style3];
    let mut selector = None;
    for (index, style) in styles.iter().copied().enumerate() {
        if style == WS_POPUP {
            if selector.is_some() {
                return None;
            }
            selector = u8::try_from(index + 1).ok();
        }
    }
    let selector = selector?;
    if work == selector {
        return None;
    }
    Some(selector)
}

/// `push ebp; mov ebp, esp`, either `89 /r` or `8B /r`.
fn is_frame_prologue(code: &[u8], index: usize) -> bool {
    code.get(index..index.saturating_add(3))
        .is_some_and(|bytes| bytes == [0x55, 0x89, 0xe5] || bytes == [0x55, 0x8b, 0xec])
}

fn bounds_entry(
    code: &[u8],
    pattern: usize,
    work: &mut usize,
    budget: DiscoveryBudget,
) -> Result<Option<usize>, Problem> {
    let begin = pattern.saturating_sub(40);
    charge(work, pattern - begin, budget.work_bytes)?;
    let mut found = None;
    let mut index = begin;
    while index + 3 <= pattern {
        if is_frame_prologue(code, index) {
            found = Some(index);
        }
        index += 1;
    }
    Ok(found)
}

fn link_popup(
    pe: &Pe<'_>,
    budget: DiscoveryBudget,
    work: &mut usize,
    selected: &mut usize,
    bounds: &[BoundsHit],
) -> Result<Option<PopupProfile>, Problem> {
    if bounds.is_empty() {
        return Ok(None);
    }
    let mut found = None;
    for (code_va, code) in pe.code_ranges() {
        charge(work, code.len(), budget.work_bytes)?;
        for parsed in caller_groups(code, code_va, bounds, budget, work)? {
            let profile = load_popup(pe, budget, work, selected, &parsed)?;
            match &found {
                None => found = Some(profile),
                Some(previous) if previous == &profile => {}
                Some(_) => return Err(Problem::Ambiguous),
            }
        }
    }
    Ok(found)
}

fn caller_groups(
    code: &[u8],
    code_va: u32,
    bounds: &[BoundsHit],
    budget: DiscoveryBudget,
    work: &mut usize,
) -> Result<Vec<ParsedCaller>, Problem> {
    let call_cap = budget.candidate_limit.saturating_mul(3);
    let mut calls = Vec::new();
    let mut index = 0usize;
    while index + 5 <= code.len() {
        if code[index] == 0xe8
            && index >= 3
            && code[index - 3] == 0x56
            && code[index - 2] == 0x6a
            && let Some(target) = call_target(code, code_va, index)
            && bounds
                .iter()
                .any(|hit| hit.entry == target && hit.selector == code[index - 1])
        {
            if calls.len() >= call_cap {
                return Err(Problem::Budget);
            }
            calls.try_reserve(1).map_err(|_| Problem::Budget)?;
            calls.push(index);
        }
        index += 1;
    }
    let mut grouped: Vec<(usize, Vec<usize>)> = Vec::new();
    for call in calls {
        let Some(entry_at) = function_entry_before(code, call, work, budget)? else {
            continue;
        };
        if let Some(group) = grouped.iter_mut().find(|group| group.0 == entry_at) {
            group.1.try_reserve(1).map_err(|_| Problem::Budget)?;
            group.1.push(call);
        } else {
            if grouped.len() >= budget.candidate_limit {
                return Err(Problem::Budget);
            }
            let mut group_calls = Vec::new();
            group_calls.try_reserve(1).map_err(|_| Problem::Budget)?;
            group_calls.push(call);
            grouped.try_reserve(1).map_err(|_| Problem::Budget)?;
            grouped.push((entry_at, group_calls));
        }
    }
    let mut parsed = Vec::new();
    for (span_start, group) in grouped {
        if let Some(caller) = parse_calls(code, span_start, &group) {
            parsed.try_reserve(1).map_err(|_| Problem::Budget)?;
            parsed.push(caller);
        }
    }
    Ok(parsed)
}

fn parse_calls(code: &[u8], span_start: usize, calls: &[usize]) -> Option<ParsedCaller> {
    if calls.len() != 3 || calls.iter().any(|call| *call < span_start) {
        return None;
    }
    if !is_capture(code, calls[0]) || table_family(code, calls[0]).is_some() {
        return None;
    }
    let mut families = [None, None];
    let mut later_end = None;
    for (slot, &call) in calls[1..].iter().enumerate() {
        let captured = is_capture(code, call);
        let table = table_family(code, call);
        if captured || table.is_none() {
            return None;
        }
        let table = table?;
        later_end = Some(table.pattern_end);
        families[slot] = Some(ParsedFamily {
            start: table.start,
            end: table.end,
            kind: table.kind,
            flag: table.flag,
        });
    }
    Some(ParsedCaller {
        families: [families[0].take()?, families[1].take()?],
        full: full_record(code, later_end?),
    })
}

fn call_target(code: &[u8], code_va: u32, call: usize) -> Option<u32> {
    let rel = i32::from_le_bytes(code.get(call + 1..call + 5)?.try_into().ok()?);
    let next = code_va.checked_add(u32::try_from(call + 5).ok()?)?;
    Some(next.wrapping_add(rel as u32))
}

fn is_capture(code: &[u8], call: usize) -> bool {
    code.get(call + 5..)
        .and_then(|tail| tail.strip_prefix(&[0x83, 0xc4, 0x08, 0x89, 0x55, 0x08]))
        .is_some_and(|width| width.starts_with(&[0x89, 0xc3]) || width.starts_with(&[0x8b, 0xd8]))
}

struct TableFamily {
    start: u32,
    end: u32,
    kind: i32,
    flag: u8,
    pattern_end: usize,
}

fn table_family(code: &[u8], call: usize) -> Option<TableFamily> {
    let push = call.checked_sub(3)?;
    let window_from = push.saturating_sub(16);
    let region = code.get(window_from..push)?;
    let bf_rel = region.iter().rposition(|byte| *byte == 0xbf)?;
    let bf = window_from + bf_rel;
    if bf + 5 > push || !is_nop_gap(code.get(bf + 5..push)?) {
        return None;
    }
    let start = u32::from_le_bytes(code.get(bf + 1..bf + 5)?.try_into().ok()?);
    let scan_end = call.checked_add(5)?.checked_add(0x120)?.min(code.len());
    let tail = code.get(call + 5..scan_end)?;
    let add = tail
        .windows(9)
        .position(|chunk| chunk.starts_with(&[0x83, 0xc7, 0x08, 0x81, 0xff]))?;
    let end = u32::from_le_bytes(tail.get(add + 5..add + 9)?.try_into().ok()?);
    let record = tail.get(..add)?;
    if one_record_word(record, 0xcc)? != 0 || one_record_word(record, 0xd0)? != 0 {
        return None;
    }
    let (kind, flag) = one_kind_flag(record)?;
    Some(TableFamily {
        start,
        end,
        kind,
        flag,
        pattern_end: call + 5 + add + 9,
    })
}

fn is_nop_gap(gap: &[u8]) -> bool {
    let mut index = 0;
    while index < gap.len() {
        if gap[index] == 0x90 {
            index += 1;
            continue;
        }
        if gap[index..].starts_with(&[0x0f, 0x1f, 0x00]) {
            index += 3;
            continue;
        }
        if gap[index..].starts_with(&[0x0f, 0x1f, 0x40, 0x00]) {
            index += 4;
            continue;
        }
        if gap[index..].starts_with(&[0x0f, 0x1f, 0x44, 0x00, 0x00]) {
            index += 5;
            continue;
        }
        if gap[index..].starts_with(&[0x66, 0x0f, 0x1f, 0x84, 0x00, 0x00, 0x00, 0x00, 0x00]) {
            index += 9;
            continue;
        }
        return false;
    }
    true
}

fn one_kind_flag(window: &[u8]) -> Option<(i32, u8)> {
    let mut found = None;
    let mut index = 0;
    while index + 7 <= window.len() {
        if window[index..].starts_with(&[0xc7, 0x45, 0xc8]) {
            if found.is_some() {
                return None;
            }
            let kind = i32::from_le_bytes(window.get(index + 3..index + 7)?.try_into().ok()?);
            let flag_at = index + 7;
            if !window.get(flag_at..)?.starts_with(&[0xc6, 0x45, 0xd4]) {
                return None;
            }
            let flag = *window.get(flag_at + 3)?;
            found = Some((kind, flag));
            index = flag_at + 4;
            continue;
        }
        index += 1;
    }
    found
}

fn one_record_word(window: &[u8], slot: u8) -> Option<u32> {
    let mut stores = window
        .windows(7)
        .filter(|bytes| bytes.starts_with(&[0xc7, 0x45, slot]));
    let store = stores.next()?;
    if stores.next().is_some() {
        return None;
    }
    Some(u32::from_le_bytes(store[3..].try_into().ok()?))
}

/// The appended record reloads the captured edx from `[ebp+8]` into eax and
/// stores ebx (captured eax) with that eax into both dimension pairs. A shape
/// that does not validate omits the record.
fn full_record(code: &[u8], from: usize) -> Option<FullRecord> {
    let window = code
        .get(from..)?
        .get(..16.min(code.len().saturating_sub(from)))?;
    let start = window
        .windows(3)
        .position(|chunk| chunk == [0x8b, 0x45, 0x08])?;
    let mut cursor = Cur {
        bytes: code.get(from + start..)?,
        index: 0,
    };
    cursor.expect(&[0x8b, 0x45, 0x08, 0xb9])?;
    cursor.take(4)?;
    cursor.expect(&[0x89, 0x75, 0xb0, 0xc7, 0x45, 0xb4])?;
    if cursor.u32()? != 0 {
        return None;
    }
    cursor.expect(&[
        0x89, 0x5d, 0xb8, 0x89, 0x45, 0xbc, 0x89, 0x5d, 0xc0, 0x89, 0x45, 0xc4,
    ])?;
    cursor.expect(&[0xc7, 0x45, 0xcc])?;
    if cursor.u32()? != 0 {
        return None;
    }
    cursor.expect(&[0xc7, 0x45, 0xd0])?;
    if cursor.u32()? != 0 {
        return None;
    }
    cursor.expect(&[0xc7, 0x45, 0xc8])?;
    let kind = cursor.i32()?;
    cursor.expect(&[0xc6, 0x45, 0xd4])?;
    let flag = cursor.u8()?;
    cursor.expect(&[0xe8])?;
    cursor.take(4)?;
    cursor.expect(&[
        0x0f, 0x10, 0x45, 0xb0, 0x46, 0x0f, 0x11, 0x00, 0x0f, 0x10, 0x45, 0xc0, 0x0f, 0x11, 0x40,
        0x10, 0xf3, 0x0f, 0x7e, 0x45, 0xd0, 0x66, 0x0f, 0xd6, 0x40, 0x20, 0x3b, 0x75, 0xf4,
    ])?;
    Some(FullRecord { kind, flag })
}

fn function_entry_before(
    code: &[u8],
    call: usize,
    work: &mut usize,
    budget: DiscoveryBudget,
) -> Result<Option<usize>, Problem> {
    let begin = call.saturating_sub(0x800);
    charge(work, call - begin, budget.work_bytes)?;
    let mut found = None;
    let mut index = begin;
    while index + 3 <= call {
        if is_frame_prologue(code, index) {
            found = Some(index);
        }
        index += 1;
    }
    Ok(found)
}

fn load_popup(
    pe: &Pe<'_>,
    budget: DiscoveryBudget,
    work: &mut usize,
    selected: &mut usize,
    parsed: &ParsedCaller,
) -> Result<PopupProfile, Problem> {
    let mut families = Vec::new();
    families
        .try_reserve(parsed.families.len())
        .map_err(|_| Problem::Budget)?;
    for family in &parsed.families {
        let count = table_count(family.start, family.end).ok_or(Problem::Operand)?;
        let bytes = count.checked_mul(8).ok_or(Problem::Operand)?;
        charge(work, bytes, budget.work_bytes)?;
        charge(selected, bytes, budget.image.profile_bytes)?;
        let pairs = read_pairs(pe, family.start, count)?;
        families.push(PopupFamily {
            pairs,
            kind: family.kind,
            flag: family.flag,
        });
    }
    Ok(PopupProfile {
        families,
        full: parsed.full,
    })
}

fn table_count(start: u32, end: u32) -> Option<usize> {
    let span = end.checked_sub(start)?;
    if span == 0 || !span.is_multiple_of(8) {
        return None;
    }
    let count = usize::try_from(span / 8).ok()?;
    (1..=MAX_PAIRS).contains(&count).then_some(count)
}

fn read_pairs(pe: &Pe<'_>, va: u32, count: usize) -> Result<Vec<(i32, i32)>, Problem> {
    let len = count.checked_mul(8).ok_or(Problem::Operand)?;
    let tail = pe.read_from(va, false).map_err(|_| Problem::Operand)?;
    let bytes = tail.get(..len).ok_or(Problem::Operand)?;
    let mut pairs = Vec::new();
    pairs
        .try_reserve_exact(count)
        .map_err(|_| Problem::Budget)?;
    for chunk in bytes.as_chunks::<8>().0 {
        let width = i32::from_le_bytes(chunk[..4].try_into().map_err(|_| Problem::Operand)?);
        let height = i32::from_le_bytes(chunk[4..].try_into().map_err(|_| Problem::Operand)?);
        if width <= 0 || height <= 0 {
            return Err(Problem::Operand);
        }
        pairs.push((width, height));
    }
    if pairs.len() == count {
        Ok(pairs)
    } else {
        Err(Problem::Operand)
    }
}

#[cfg(test)]
#[path = "options_modes_tests.rs"]
mod tests;
