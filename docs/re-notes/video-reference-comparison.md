# Original movie and menu reference comparison

Checked 2026-10-13. Target: The Frozen Throne 1.29b. This is a bounded reference pass, not a claim that the current menu is visually faithful.

## YouTube sources and playback limit

| Source | Identity and useful scope | Inspection status |
| --- | --- | --- |
| [Blizzard: TFT intro](https://www.youtube.com/watch?v=iqB3KeiBLuw) | Official Blizzard upload, published 2009-09-25. Cinematic reference; not a capture of the 1.29b startup flow. | Playback blocked by YouTube's sign-in/bot confirmation. No video frames or timing verified from this upload. |
| [RoC intro](https://www.youtube.com/watch?v=tib0GcRYmBU) | Separate base-game cinematic candidate. Do not substitute for TFT's IntroX movie. | Page fetch failed; playback unverified. |
| [SkyboxEye: TFT main menu](https://www.youtube.com/watch?v=TVSuvfryo3E) | Original-style background ambience candidate, published 2021-03-02. Exact client version unknown. | Paseo browser showed sign-in/bot confirmation; no frames or motion inspected. |
| [SkyboxEye: TFT widescreen main menu](https://www.youtube.com/watch?v=WY19S_AsFCo) | Background composition candidate. Exact client version and modifications unknown. | Fetch failed/throttled; playback unverified. |
| [World of Longplays: TFT, part 1](https://www.youtube.com/watch?v=d6FpUNudV94) | Published 2012-03-19, played by Zuljaras. Pre-Reforged gameplay/UI candidate; necessarily predates 1.29. | Paseo browser showed sign-in/bot confirmation. Whether its opening includes the main-menu entrance is unverified. |

The browser checked the actual watch pages, not just search descriptions. No login challenge was bypassed and no video-author contact was made. Thumbnail imagery is not evidence of animation, interaction or exact menu placement. Reforged, modified backgrounds and UI-free ambience cannot establish original 1.29b control geometry.

## Visually inspected still references

The [archived Blizzard 2004 promotional screenshot](https://www.mobygames.com/game/9685/warcraft-iii-the-frozen-throne/promo/group-6894/image-81339/) shows the frozen logo at upper left, blue button interiors inside a dark chain-hung panel, a separate lower exit panel, snow, water and aurora. It says **Cancel**, so it is an early promotional UI reference, not the target build's layout authority.

A [classic screenshot linked from Riot Pixels](https://riotpixels.com/forums/topic/4296-%D1%81%D0%B5%D1%80%D0%B8%D1%8F-warcraft/) visibly identifies **1.28.5.7680**. Its displayed [image](https://images.noob-club.ru/news/2018/02/n4ub7ozox7.jpg) was inspected at 1920×1080. The approximately 1440×1080 inner menu viewport is bounded by black side bars and an authored border. Measurements below are approximate manual observations within that inner viewport, not pixel-perfect 1.29b runtime measurements.

| Label | Observed center Y / 1080 | Center from shipped FDF using 0.6 vertical extent |
| --- | --- | --- |
| Single Player | ≈257 / 1080 = 0.238 | 0.237708 |
| Battle.net | ≈360 / 1080 = 0.333 | 0.336042 |
| Local Area Network | ≈469 / 1080 = 0.434 | 0.434375 |
| Options | ≈576 / 1080 = 0.533 | 0.532708 |
| Credits | ≈681 / 1080 = 0.631 | 0.631042 |
| Quit | ≈931 / 1080 = 0.862 | 0.863333 |

The corresponding primary evidence is the shipped expansion `MainMenu.fdf` and `StandardTemplates.fdf`, inspected locally. The first text-control center is `(0.110625 + 0.0165 + 0.031/2)/0.6`. Each following upper control advances `(0.064 - 0.005)/0.6`: the positive `+0.005` anchor below the preceding `BOTTOMRIGHT` means an overlap, not an extra downward gap. Quit is separately bottom-anchored: `(0.6 - 0.05 - 0.064 + 0.0165 + 0.031/2)/0.6`.

The first two text controls have width 0.149 and reserve space for edition/realm icons. The next three inherit width 0.179. Quit's outer backdrop width is 0.24 and its text control width is 0.168. Labels are centered inside their **text controls**, not inside the larger asymmetric decorative backdrop. These measurements strongly corroborate the 0.8×0.6 convention for the corresponding classic UI, and the original 1.29b frame renderer now independently establishes this extent; see [runtime evidence](glue-coordinate-runtime.md). Live widescreen flags and exact rasterization remain unmeasured. Relative layout should preserve this parent/child geometry rather than use fixed pixel coordinates.

## Comparison with the current WebGL TFT render

The inspected `.scratch/relative-scene-tft-later.png` is the actual local reconstruction, 988×892. It has six uniformly stacked, isolated black buttons, including Quit immediately after Credits. It lacks the logo, blue control surfaces, edition/realm icons, hanging panel and chains visible in the classic reference. The lower foreground is black, and the scene shows conspicuous texture streaking and pale flat shading. These are observable gaps; they do not establish which material, transform or wrapping defect causes each one.

The FDF comparison supplies concrete corrections independent of video playback: use five upper controls and a separate bottom Quit control; preserve control-relative text centering and icon space; interpret the signed stacking anchors correctly. The screenshot also supports checking fog, particles, wrapping and authored sprite layers rather than replacing original artwork.

## Owned TFT cinematic frames

`wc3/game/app/Movies/IntroX.mpq` is a locally supplied **RIFF AVI movie**, not an MPQ archive. `ffprobe` reports MPEG-4 video, 800×368 at 24 fps, MP3 audio and 188.834844 seconds container duration. Its exact installation/build provenance is not established by this inspection; it must not be called a verified 1.29b-native encoding.

Frames were decoded with FFmpeg with `-ss` **after** the input for the exact time probes. Visually inspected samples include 20 s (Illidan close-up during a bright flash), 40 s (Illidan against the stormy sky), 80 s (creature's eye), 100 s (underwater silhouette), and 120/150 s (Illidan and the opposing creature near rocky terrain). This confirms actual TFT cinematic imagery, distinct from the animated ice-tower menu. Black frames were also observed at the sampled start/end times; isolated black samples do not prove playback failure or the precise final fade duration. No original startup skip interaction or synchronized audio playback was observed here.

The decoded image aspect ratio is 800/368 ≈2.174. A 4:3 viewport needs a centered movie at approximately 61.3% of viewport height, with black above/below, rather than stretching or cropping. In any viewport, preserve its aspect ratio with containment. The movie should own presentation and audio until it ends or the user skips it; then begin the original menu entrance. That sequencing is the requested behavior, not a claim that the native startup dispatcher has been traced.

The TFT background model's measured sequence inventory is `Stand` 0..33333, `Stand Alternate` 95333..128667 and `Morph Alternate` 167600..169533. It has no `Birth` sequence. An opening panel animation cannot be sourced from a nonexistent background `Birth`; the separate authored glue sprite layers need inspection. The logo's own fade is also separate from the movie and background.

## Retained evidence and remaining work

Analysis-only images and probe logs live under ignored `.scratch/video-reference/`; original movie frames and screenshots are not included in the repository. Reproducible input digests:

- IntroX AVI: SHA-256 `fc1689eeff44b657bb0e93a950bf00d7e712a5c869a81f7aba958fb50b61acef`.
- Expansion MainMenu FDF: `a189a1a952b7340ab11a183898ada5d601287f1b5e4b8a1042ab8380c6e9193e`.
- StandardTemplates FDF: `4605f089d26ecb1ef5bcc0d3068004fc61ef5687b1523905987dbfcea8a93ad9`.
- Version-labelled classic still: `91b7e3a469cce738d3b1b8b320906aa375c6cc79fc1949e8b198cd9fa3d723c3`.
- Current TFT render inspected: `b7dc25b290e7da9f9e7dae7139e67d9b55de0a36dce274ccb6e3234afd0ec515`.

Still needed: accessible original-client footage with UI, actual 1.29b entry/exit motion and audio comparison, native movie skip behavior, and post-fix render comparison. The YouTube video comparison remains partial because playback was blocked; no timestamps from those uploads are invented.
