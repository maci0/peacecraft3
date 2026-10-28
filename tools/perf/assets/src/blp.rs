//! BLP decode: JPEG and palette content, every mip.
use pcx_blp::{Header, decode_jpeg, decode_mip};

use crate::{
    Context, Failure, Outcome, Prepared, Workload,
    digest::Fnv,
    input::{self, extract_all},
};

pub const WORKLOADS: [Workload; 2] = [
    Workload {
        name: "blp-jpeg-v1",
        summary: "decode every mip of fixed JPEG-content textures",
        prepare: jpeg,
    },
    Workload {
        name: "blp-palette-v1",
        summary: "decode every mip of fixed palette-content textures",
        prepare: palette,
    },
];

/// content 0: JPEG scan data behind a shared header.
const JPEG_TEXTURES: [&str; 13] = [
    "Units\\Human\\HeroPaladin\\Heropaladin.blp",
    "TerrainArt\\Ashenvale\\Ashen_Dirt.blp",
    "TerrainArt\\Ashenvale\\Ashen_Grass.blp",
    "TerrainArt\\Barrens\\Barrens_Rock.blp",
    "TerrainArt\\BlackCitadel\\Citadel_LargeBricks.blp",
    "ReplaceableTextures\\Cliff\\Cliff0.blp",
    "ReplaceableTextures\\CommandButtons\\BTNShadowStrike.blp",
    "ReplaceableTextures\\CommandButtons\\BTNWandOfShadowSight.blp",
    "ReplaceableTextures\\Cliff\\Cliff1.blp",
    "UI\\Glues\\Loading\\Backgrounds\\Campaigns\\AshenvaleLocationIcons.blp",
    "UI\\Glues\\Loading\\Backgrounds\\Campaigns\\Ashenvale-TopLeft.blp",
    "UI\\Glues\\Loading\\Backgrounds\\Campaigns\\Barrens-BotRight.blp",
    "Doodads\\Northrend\\Water\\Whale\\killerwhale.blp",
];

/// content 1: palette indices with an optional alpha plane.
const PALETTE_TEXTURES: [&str; 10] = [
    "ReplaceableTextures\\Selection\\SelectionCircleLarge.blp",
    "ReplaceableTextures\\Selection\\SelectionCircleMed.blp",
    "ReplaceableTextures\\Selection\\SpellAreaOfEffect.blp",
    "ReplaceableTextures\\Selection\\SpellAreaOfEffect512.blp",
    "ReplaceableTextures\\Shadows\\BuildingShadowLarge.blp",
    "ReplaceableTextures\\Shadows\\Shadow.blp",
    "ReplaceableTextures\\Shadows\\ShadowArmory.blp",
    "ReplaceableTextures\\Shadows\\ShadowBlackCitidel.blp",
    "ReplaceableTextures\\Selection\\SelectionCircleSmall.blp",
    "UI\\Glues\\BattleNet\\BattlenetChatGlue\\CenterPanel01.blp",
];

fn prepare(ctx: &Context, paths: &[&'static str], content: u32) -> Result<Prepared, Failure> {
    let mut set = input::open_set_with_codec(&ctx.install)?;
    let mut input_digest = Fnv::default();
    let files = extract_all(&mut set, paths, &mut input_digest)?;
    let mut misfiled = Vec::new();
    for (path, bytes) in &files {
        let header = Header::parse(bytes).map_err(|e| format!("{path}: {e}"))?;
        if header.content != content {
            misfiled.push(format!("{path} (content {})", header.content));
        }
    }
    if !misfiled.is_empty() {
        return Err(format!("not content-{content}: {}", misfiled.join(", ")));
    }
    Ok(Prepared {
        input: input_digest,
        run: Box::new(move || {
            let mut decoded = Vec::new();
            let mut pixels = 0u64;
            for (path, bytes) in &files {
                let header = Header::parse(bytes).map_err(|e| format!("{path}: {e}"))?;
                for mip in 0..header.mip_count() {
                    let rgba = if content == 0 {
                        decode_jpeg(bytes, &header, mip)
                    } else {
                        decode_mip(bytes, &header, mip)
                    }
                    .map_err(|e| format!("{path} mip {mip}: {e}"))?;
                    pixels += rgba.len() as u64 / 4;
                    decoded.push(rgba);
                }
            }
            Ok(Outcome {
                work: vec![("mips", decoded.len() as u64), ("pixels", pixels)],
                fold: Box::new(move |out| decoded.iter().for_each(|rgba| out.bytes(rgba))),
            })
        }),
    })
}

fn jpeg(ctx: &Context) -> Result<Prepared, Failure> {
    prepare(ctx, &JPEG_TEXTURES, 0)
}

fn palette(ctx: &Context) -> Result<Prepared, Failure> {
    prepare(ctx, &PALETTE_TEXTURES, 1)
}
