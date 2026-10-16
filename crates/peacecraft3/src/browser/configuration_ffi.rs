//! Owned metadata-only JS replies. No asset bytes or gameplay activation cross this boundary.
use super::{
    browser_budget, committed_archive_selection, enqueue_map_configuration_json,
    poll_map_configuration,
};
use pcx_assets::SourceError;
use peacecraft3::{
    browser_archive_source::ArchiveSelection,
    browser_configuration::{InboxError, Poll},
    browser_configuration_json::Error,
    map_requests::{MapOutcome, RequestFailure},
};
use wasm_bindgen::prelude::*;

// Preserve the existing budget guard's original JS Error, alongside concise project diagnostics.
struct Argument {
    field: &'static str,
    error: JsError,
}
fn integer(value: &JsValue, field: &'static str) -> Result<usize, Argument> {
    browser_budget(value, field).map_err(|error| Argument { field, error })
}
fn identifier(value: &JsValue, field: &'static str) -> Result<u32, Argument> {
    let value = integer(value, field)?;
    u32::try_from(value).map_err(|_| Argument {
        field,
        error: JsError::new("identifier exceeds u32"),
    })
}
fn source_kind(error: &SourceError) -> &'static str {
    match error {
        SourceError::Unavailable { .. } => "source-unavailable",
        SourceError::InvalidInput { .. } => "source-invalid-input",
        SourceError::Truncated { .. } => "source-truncated",
        SourceError::Ambiguous { .. } => "source-ambiguous",
        SourceError::Limit { .. } => "source-limit",
        SourceError::BudgetUnsupported { .. } => "source-budget-unsupported",
        SourceError::MpqExtraction(_) => "source-mpq-extraction",
        SourceError::CascRead(_) => "source-casc-read",
        SourceError::Io(_) => "source-io",
    }
}
fn queue_kind(error: &InboxError) -> &'static str {
    match error {
        InboxError::Busy => "queue-busy",
        InboxError::Unknown { .. } => "queue-unknown",
        InboxError::Poisoned => "queue-poisoned",
        InboxError::Exhausted => "queue-exhausted",
    }
}
fn decoder_kind(error: &Error) -> &'static str {
    match error {
        Error::Budget { .. } => "metadata-budget",
        Error::Syntax(_) => "metadata-syntax",
        Error::Schema(_) => "metadata-schema",
        Error::Policy { .. } => "metadata-policy",
        Error::Revision(_) => "binding-revision",
        Error::Mode(_) => "mode",
        Error::Path { .. } => "path",
        Error::Table { .. } => "catalog-table",
        Error::Source { source, .. } => source_kind(source),
        Error::Resolver(_) => "resolver",
        Error::Queue(error) => queue_kind(error),
    }
}
fn request_kind(error: &RequestFailure) -> &'static str {
    match error {
        RequestFailure::Binding(_) => "map-binding",
        RequestFailure::MissingBindings => "map-missing-bindings",
        RequestFailure::StaleBindings { .. } => "map-stale-bindings",
        RequestFailure::IdentityMismatch => "map-identity-mismatch",
        RequestFailure::MemberMismatch { .. } => "map-member-mismatch",
        RequestFailure::CatalogMapMismatch => "map-catalog-mismatch",
        RequestFailure::FileBudget { .. } => "map-file-budget",
        RequestFailure::Container(_) => "map-container",
        RequestFailure::Admission(_) => "map-admission",
    }
}
enum Inspection {
    Selected(ArchiveSelection),
    Argument(Argument),
    Source(SourceError),
}
#[wasm_bindgen]
pub struct MapArchiveInspection {
    state: Inspection,
}
#[wasm_bindgen]
impl MapArchiveInspection {
    pub fn status(&self) -> String {
        match self.state {
            Inspection::Selected(_) => "selected",
            _ => "failed",
        }
        .into()
    }
    pub fn kind(&self) -> String {
        match &self.state {
            Inspection::Selected(_) => "archive-selected",
            Inspection::Argument(_) => "argument",
            Inspection::Source(error) => source_kind(error),
        }
        .into()
    }
    pub fn diagnostic(&self) -> String {
        match &self.state {
            Inspection::Selected(_) => {
                "archive metadata selected; source version and member policy remain caller-supplied"
                    .into()
            }
            Inspection::Argument(error) => format!(
                "{} must be an explicit finite integer within range",
                error.field
            ),
            Inspection::Source(error) => error.to_string(),
        }
    }
    pub fn index(&self) -> Option<u32> {
        match &self.state {
            Inspection::Selected(s) => Some(s.index as u32),
            _ => None,
        }
    }
    pub fn committed_revision(&self) -> Option<u32> {
        match &self.state {
            Inspection::Selected(s) => Some(s.committed_revision),
            _ => None,
        }
    }
    pub fn archive_name(&self) -> Option<String> {
        match &self.state {
            Inspection::Selected(s) => Some(s.archive_name.clone()),
            _ => None,
        }
    }
    /// Decimal u64 avoids loss through JavaScript's Number.
    pub fn archive_bytes(&self) -> Option<String> {
        match &self.state {
            Inspection::Selected(s) => Some(s.archive_bytes.to_string()),
            _ => None,
        }
    }
    /// The original JS guard Error remains owned; this returns a JS reference without consuming it.
    pub fn argument_error(&self) -> Option<JsValue> {
        match &self.state {
            Inspection::Argument(a) => Some(a.error.clone().into()),
            _ => None,
        }
    }
}
#[wasm_bindgen]
pub fn inspect_map_archive(index: JsValue, committed_revision: JsValue) -> MapArchiveInspection {
    let result = integer(&index, "archive_index").and_then(|index| {
        identifier(&committed_revision, "committed_revision").map(|revision| (index, revision))
    });
    MapArchiveInspection {
        state: match result {
            Err(error) => Inspection::Argument(error),
            Ok((index, revision)) => match committed_archive_selection(index, revision) {
                Ok(selected) => Inspection::Selected(selected),
                Err(error) => Inspection::Source(error),
            },
        },
    }
}
enum Cause {
    Argument(Argument),
    Decoder(Error),
    Queue(InboxError),
}
enum Reply {
    Submitted(u32),
    Pending(u32),
    Completed(u32, Box<MapOutcome>),
    Failed(Option<u32>, Cause),
}
#[wasm_bindgen]
pub struct MapConfigurationResult {
    state: Reply,
}
#[wasm_bindgen]
impl MapConfigurationResult {
    pub fn id(&self) -> Option<u32> {
        match &self.state {
            Reply::Submitted(id) | Reply::Pending(id) | Reply::Completed(id, _) => Some(*id),
            Reply::Failed(id, _) => *id,
        }
    }
    pub fn status(&self) -> String {
        match self.state {
            Reply::Submitted(_) => "submitted",
            Reply::Pending(_) => "pending",
            Reply::Completed(_, _) => "completed",
            Reply::Failed(_, _) => "failed",
        }
        .into()
    }
    pub fn kind(&self) -> String {
        match &self.state {
            Reply::Submitted(_) => "configuration-submitted",
            Reply::Pending(_) => "configuration-pending",
            Reply::Completed(_, outcome) => match &outcome.result {
                Ok(()) => "map-admitted",
                Err(error) => request_kind(error),
            },
            Reply::Failed(_, Cause::Argument(_)) => "argument",
            Reply::Failed(_, Cause::Decoder(error)) => decoder_kind(error),
            Reply::Failed(_, Cause::Queue(error)) => queue_kind(error),
        }
        .into()
    }
    pub fn diagnostic(&self) -> String {
        match &self.state {
            Reply::Submitted(_) => {
                "configuration queued for the existing application consumer".into()
            }
            Reply::Pending(_) => "configuration pending or in flight".into(),
            Reply::Completed(_, outcome) => match &outcome.result {
                Ok(()) => "map parsed and admitted; gameplay Host execution has not started".into(),
                Err(error) => error.to_string(),
            },
            Reply::Failed(_, Cause::Argument(error)) => format!(
                "{} must be an explicit finite integer within range",
                error.field
            ),
            Reply::Failed(_, Cause::Decoder(error)) => error.to_string(),
            Reply::Failed(_, Cause::Queue(error)) => error.to_string(),
        }
    }
    pub fn label(&self) -> Option<String> {
        match &self.state {
            Reply::Completed(_, o) => Some(o.label.clone()),
            _ => None,
        }
    }
    pub fn binding_revision(&self) -> Option<String> {
        match &self.state {
            Reply::Completed(_, o) => o.binding_revision.map(|n| n.to_string()),
            _ => None,
        }
    }
    pub fn identity_source(&self) -> Option<String> {
        match &self.state {
            Reply::Completed(_, o) => o.identity.as_ref().map(|i| i.source.clone()),
            _ => None,
        }
    }
    pub fn identity_edition(&self) -> Option<String> {
        match &self.state {
            Reply::Completed(_, o) => o.identity.as_ref().map(|i| i.edition.clone()),
            _ => None,
        }
    }
    pub fn identity_version(&self) -> Option<String> {
        match &self.state {
            Reply::Completed(_, o) => o.identity.as_ref().map(|i| i.version.clone()),
            _ => None,
        }
    }
    pub fn argument_error(&self) -> Option<JsValue> {
        match &self.state {
            Reply::Failed(_, Cause::Argument(a)) => Some(a.error.clone().into()),
            _ => None,
        }
    }
}
/// String has already crossed wasm-bindgen's allocation boundary; the explicit UTF-8 cap precedes serde.
#[wasm_bindgen]
pub fn submit_map_configuration(
    text: String,
    max_metadata_bytes: JsValue,
) -> MapConfigurationResult {
    let state = match integer(&max_metadata_bytes, "max_metadata_bytes") {
        Err(error) => Reply::Failed(None, Cause::Argument(error)),
        Ok(limit) => match enqueue_map_configuration_json(&text, limit) {
            Ok(id) => Reply::Submitted(id),
            Err(error) => Reply::Failed(None, Cause::Decoder(error)),
        },
    };
    MapConfigurationResult { state }
}
/// A completed poll acknowledges the inbox; the returned handle owns the complete original outcome.
#[wasm_bindgen]
pub fn poll_map_configuration_result(id: JsValue) -> MapConfigurationResult {
    let state = match identifier(&id, "request_id") {
        Err(error) => Reply::Failed(None, Cause::Argument(error)),
        Ok(id) => match poll_map_configuration(id) {
            Ok(Poll::Pending) => Reply::Pending(id),
            Ok(Poll::Completed(outcome)) => Reply::Completed(id, outcome),
            Err(error) => Reply::Failed(Some(id), Cause::Queue(error)),
        },
    };
    MapConfigurationResult { state }
}

#[cfg(test)]
#[path = "../../tests/support/browser_configuration_ffi_cases.rs"]
mod tests;
