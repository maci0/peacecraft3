//! Source preparation follows executed paths, never a chapter-specific asset list.
use pcx_assets::Resolver;
use pcx_play::{
    day_night::{Assets, Budget, ModelInput},
    scripts::{AssetIdentity, ScriptAsset},
};

pub(crate) fn day_night(
    reader: &Resolver,
    edition: &str,
    paths: [&str; 2],
    budget: Budget,
) -> Result<Assets, String> {
    if paths.iter().any(|path| path.len() > budget.path_bytes) || edition.len() > budget.path_bytes
    {
        return Err("SetDayNightModels: source path/identity budget exceeded".into());
    }
    let paths = if paths[0] == paths[1] {
        &paths[..1]
    } else {
        &paths[..]
    };
    if paths.len() > budget.models {
        return Err("SetDayNightModels: source model count budget exceeded".into());
    }
    let mut remaining = budget.total_bytes;
    let mut loaded = Vec::new();
    for path in paths {
        let member = pcx_mdl::binary_model_path(path);
        let limit = budget.model_bytes.min(remaining);
        let asset = reader
            .resolve_bounded(&member, limit as u64)
            .map_err(|error| format!("SetDayNightModels {path} -> {member}: {error}"))?;
        remaining = remaining.checked_sub(asset.bytes.len()).ok_or_else(|| {
            format!("SetDayNightModels {path}: aggregate source byte budget exceeded")
        })?;
        let (label, origin) = identity(&asset, edition, budget.path_bytes)
            .map_err(|error| format!("SetDayNightModels {path}: {error}"))?;
        loaded.push((asset, label, origin));
    }
    let inputs: Vec<_> = paths
        .iter()
        .zip(&loaded)
        .map(|(path, (asset, label, origin))| ModelInput {
            script_path: path,
            asset: ScriptAsset {
                label,
                bytes: &asset.bytes,
            },
            origin: origin.clone(),
        })
        .collect();
    Assets::read(&inputs, budget).map_err(|error| format!("SetDayNightModels: {error}"))
}

pub(crate) fn file_sound(
    reader: &Resolver,
    edition: &str,
    member: &str,
    budget: pcx_play::file_sounds::Budget,
    mp3: pcx_play::file_sounds::Mp3Budget,
) -> Result<pcx_play::file_sounds::Assets, String> {
    // Application metadata cap also bounds adapter identity formatting before copies.
    let metadata = budget.metadata_bytes.min(16 << 10);
    if member.len() > metadata || edition.len() > metadata || budget.resources == 0 {
        return Err("CreateSound: source path/identity/count budget exceeded".into());
    }
    let asset = reader
        .resolve_bounded(member, budget.resource_bytes.min(budget.total_bytes) as u64)
        .map_err(|error| format!("CreateSound {member}: {error}"))?;
    let (label, origin) = identity(&asset, edition, metadata)
        .map_err(|error| format!("CreateSound {member}: {error}"))?;
    pcx_play::file_sounds::Assets::read_encoded(
        &[pcx_play::file_sounds::SoundInput {
            member,
            asset: ScriptAsset {
                label: &label,
                bytes: &asset.bytes,
            },
            origin,
        }],
        budget,
        mp3,
    )
    .map_err(|error| format!("CreateSound {member}: {error}"))
}

fn identity(
    asset: &pcx_assets::ResolvedAsset,
    edition: &str,
    limit: usize,
) -> Result<(String, AssetIdentity), String> {
    let source = &asset.provenance.source;
    let bytes = [&source.id, &source.origin, &asset.provenance.locator]
        .iter()
        .try_fold(0usize, |size, value| size.checked_add(value.len()))
        .and_then(|size| size.checked_add(source.version.as_ref().map_or(0, String::len)));
    if edition.len() > limit || bytes.is_none_or(|bytes| bytes > limit) {
        return Err("source identity budget exceeded".into());
    }
    Ok((
        format!("{}::{}", source.origin, asset.provenance.locator),
        AssetIdentity {
            edition: edition.to_owned(),
            version: source
                .version
                .clone()
                .unwrap_or_else(|| "unversioned source".into()),
            source: serde_json::to_string(&asset.provenance).map_err(|error| error.to_string())?,
        },
    ))
}
