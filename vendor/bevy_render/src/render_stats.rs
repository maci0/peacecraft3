//! Process-wide render submission counters. Local patch: see `vendor/BEVY_PATCHES.md`.
//!
//! Plain relaxed counters of the work the renderer submits, so tests can compare draw calls,
//! bind-group switches and resource creation between runs without timing anything.
use core::sync::atomic::{AtomicU64, Ordering};

/// Totals since process start. Subtract two snapshots to count one interval.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderStats {
    /// `draw`, `draw_indexed` and the single indirect draws issued through a tracked pass.
    pub draw_calls: u64,
    /// `multi_draw_*` calls (each issues many draws on the GPU).
    pub multi_draw_calls: u64,
    /// Pipelines actually set (redundant sets are skipped by the tracked pass).
    pub pipelines_set: u64,
    /// Bind groups actually set (redundant sets are skipped by the tracked pass).
    pub bind_groups_set: u64,
    /// `RenderDevice::create_bind_group` calls.
    pub bind_groups_created: u64,
    /// `RenderDevice::create_buffer` and `create_buffer_with_data` calls.
    pub buffers_created: u64,
    /// Bytes requested by those buffer creations.
    pub buffer_bytes_created: u64,
    /// `RenderDevice::create_texture` and `create_texture_with_data` calls.
    pub textures_created: u64,
}

impl core::ops::Sub for RenderStats {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self {
            draw_calls: self.draw_calls - other.draw_calls,
            multi_draw_calls: self.multi_draw_calls - other.multi_draw_calls,
            pipelines_set: self.pipelines_set - other.pipelines_set,
            bind_groups_set: self.bind_groups_set - other.bind_groups_set,
            bind_groups_created: self.bind_groups_created - other.bind_groups_created,
            buffers_created: self.buffers_created - other.buffers_created,
            buffer_bytes_created: self.buffer_bytes_created - other.buffer_bytes_created,
            textures_created: self.textures_created - other.textures_created,
        }
    }
}

static DRAW_CALLS: AtomicU64 = AtomicU64::new(0);
static MULTI_DRAW_CALLS: AtomicU64 = AtomicU64::new(0);
static PIPELINES_SET: AtomicU64 = AtomicU64::new(0);
static BIND_GROUPS_SET: AtomicU64 = AtomicU64::new(0);
static BIND_GROUPS_CREATED: AtomicU64 = AtomicU64::new(0);
static BUFFERS_CREATED: AtomicU64 = AtomicU64::new(0);
static BUFFER_BYTES_CREATED: AtomicU64 = AtomicU64::new(0);
static TEXTURES_CREATED: AtomicU64 = AtomicU64::new(0);

/// Reads every counter.
pub fn snapshot() -> RenderStats {
    RenderStats {
        draw_calls: DRAW_CALLS.load(Ordering::Relaxed),
        multi_draw_calls: MULTI_DRAW_CALLS.load(Ordering::Relaxed),
        pipelines_set: PIPELINES_SET.load(Ordering::Relaxed),
        bind_groups_set: BIND_GROUPS_SET.load(Ordering::Relaxed),
        bind_groups_created: BIND_GROUPS_CREATED.load(Ordering::Relaxed),
        buffers_created: BUFFERS_CREATED.load(Ordering::Relaxed),
        buffer_bytes_created: BUFFER_BYTES_CREATED.load(Ordering::Relaxed),
        textures_created: TEXTURES_CREATED.load(Ordering::Relaxed),
    }
}

pub(crate) fn draw() {
    DRAW_CALLS.fetch_add(1, Ordering::Relaxed);
}

pub(crate) fn multi_draw() {
    MULTI_DRAW_CALLS.fetch_add(1, Ordering::Relaxed);
}

pub(crate) fn pipeline_set() {
    PIPELINES_SET.fetch_add(1, Ordering::Relaxed);
}

pub(crate) fn bind_group_set() {
    BIND_GROUPS_SET.fetch_add(1, Ordering::Relaxed);
}

pub(crate) fn bind_group_created() {
    BIND_GROUPS_CREATED.fetch_add(1, Ordering::Relaxed);
}

pub(crate) fn buffer_created(bytes: u64) {
    BUFFERS_CREATED.fetch_add(1, Ordering::Relaxed);
    BUFFER_BYTES_CREATED.fetch_add(bytes, Ordering::Relaxed);
}

pub(crate) fn texture_created() {
    TEXTURES_CREATED.fetch_add(1, Ordering::Relaxed);
}
