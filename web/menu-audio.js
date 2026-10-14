// Original TFT audio. Browsers require a gesture before audible playback.
const localOnly = document.documentElement.dataset.localAssets === 'true' || new URLSearchParams(location.search).has('local');
const music = localOnly ? new Audio() : new Audio('./assets/menu-music.mp3');
music.loop = true;
const clickSound = localOnly ? new Audio() : new Audio('./assets/menu-click.wav');
if (!localOnly) clickSound.volume = 100 / 127; // Existing staged-file policy; local imports use authored settings.
let enabled = !localOnly;
let unlocked = false;
let clicks = 0;
let introActive = false;
let localMode = localOnly;
let clickNoDuplicates = true;
let localUrls = [];
let localSources = { music: null, click: null };
let generation = 0;

function resetLocalAssets() {
  generation++;
  music.pause(); clickSound.pause();
  music.removeAttribute('src'); clickSound.removeAttribute('src');
  music.load(); clickSound.load();
  for (const url of localUrls) URL.revokeObjectURL(url);
  localUrls = []; localSources = { music: null, click: null };
  localMode = true; unlocked = false;
}
async function audioType(blob) {
  if (!(blob instanceof Blob)) throw new TypeError('local audio must be a Blob');
  const b = new Uint8Array(await blob.slice(0, 12).arrayBuffer());
  const text = String.fromCharCode(...b);
  if (text.startsWith('RIFF') && text.slice(8) === 'WAVE') return 'audio/wav';
  if (text.startsWith('ID3') || (b[0] === 255 && (b[1] & 224) === 224 && (b[1] & 6) === 2)) return 'audio/mpeg';
  throw new Error('UNSUPPORTED_LOCAL_AUDIO: expected WAV or MP3 content');
}
async function setLocalAssets({ music: musicSource = null, click: clickSource = null }) {
  const request = ++generation;
  const sources = [];
  for (const [name, descriptor, player] of [['music', musicSource, music], ['click', clickSource, clickSound]]) {
    if (!descriptor) continue;
    if (!descriptor.path || !descriptor.provenance) throw new Error(`${name}: local source requires canonical path and provenance`);
    const type = await audioType(descriptor.blob);
    if (!player.canPlayType(type)) throw new Error(`${descriptor.path}: browser does not support ${type}`);
    if (name === 'click' && (!Number.isFinite(descriptor.settings?.volume) || descriptor.settings.volume < 0 || descriptor.settings.volume > 1 || typeof descriptor.settings.noDuplicates !== 'boolean'))
      throw new Error(`${descriptor.path}: missing/unsupported authored click settings`);
    sources.push([name, descriptor, player, type]);
  }
  if (request !== generation) throw new Error('local audio replacement superseded');
  resetLocalAssets();
  for (const [name, descriptor, player, type] of sources) {
    const url = URL.createObjectURL(new Blob([descriptor.blob], { type }));
    localUrls.push(url); player.src = url; player.muted = !enabled;
    localSources[name] = { path: descriptor.path, provenance: descriptor.provenance };
    if (name === 'click') { player.volume = descriptor.settings.volume; clickNoDuplicates = descriptor.settings.noDuplicates; }
    // Existing launcher music loop/volume policy is preserved, not native setting proof.
    player.load();
  }
  if (musicSource && enabled && !introActive) unlock();
  return { ...localSources, musicVolumePolicy: 'existing browser policy; original user volume unverified' };
}
window.addEventListener('pagehide', resetLocalAssets);
const report = error => console.warn('Menu audio:', error.message);

function unlock() {
  if (unlocked || !enabled || introActive || (localMode && !localSources.music)) return;
  unlocked = true;
  music.play().catch(error => { unlocked = false; report(error); });
}
document.addEventListener('pointerdown', unlock, { capture: true });
document.addEventListener('keydown', unlock, { capture: true });

globalThis.peacecraft3Audio = {
  setLocalAssets,
  resetLocalAssets,
  pauseForIntro() { introActive = true; music.pause(); clickSound.pause(); },
  resumeAfterIntro() { introActive = false; unlocked = false; music.currentTime = 0; unlock(); },
  click() {
    if (!enabled || introActive || (localMode && !localSources.click)) return;
    unlock();
    // The original GlueScreenClick is NODUPLICATES.
    if (clickNoDuplicates && !clickSound.paused && !clickSound.ended) return;
    clickSound.currentTime = 0;
    clicks++;
    clickSound.play().catch(report);
  },
  // A model event sound; the app resolves its WAV bytes and volume from the
  // install's AnimLookups.slk and AnimSounds.slk.
  event(bytes, volume) {
    if (!enabled || introActive) return;
    const url = URL.createObjectURL(new Blob([bytes], { type: 'audio/wav' }));
    const sound = new Audio(url);
    sound.volume = volume;
    sound.addEventListener('ended', () => URL.revokeObjectURL(url), { once: true });
    sound.play().catch(error => { URL.revokeObjectURL(url); report(error); });
  },
  status() {
    return { enabled, unlocked, clicks, introActive, localMode, localSources, localUrlCount: localUrls.length, musicVolumePolicy: 'existing browser policy; original user volume unverified', musicTime: music.currentTime, musicPaused: music.paused,
      loop: music.loop, clickTime: clickSound.currentTime, clickPaused: clickSound.paused,
      clickVolume: clickSound.volume, muted: music.muted && clickSound.muted };
  },
};

const toggle = document.querySelector('#sound');
toggle.textContent = enabled ? 'Mute sound' : 'Enable sound';
toggle.setAttribute('aria-pressed', String(!enabled));
toggle.addEventListener('click', () => {
  enabled = !enabled;
  music.muted = clickSound.muted = !enabled;
  toggle.textContent = enabled ? 'Mute sound' : 'Enable sound';
  toggle.setAttribute('aria-pressed', String(!enabled));
  document.dispatchEvent(new CustomEvent('peacecraft3-sound-changed', { detail: { enabled } }));
  if (enabled) unlock();
});
