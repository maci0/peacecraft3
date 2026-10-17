// Original TFT audio. Browsers require a gesture before audible playback.
const scriptContexts = new Set();
const BrowserAudioContext = globalThis.AudioContext;
// Bevy's CPAL context is created before interaction and has no public resume handle.
if (BrowserAudioContext) {
  globalThis.AudioContext = class extends BrowserAudioContext {
    constructor(...args) {
      super(...args);
      scriptContexts.add(this);
      this.addEventListener('statechange', () => {
        if (this.state === 'closed') scriptContexts.delete(this);
      });
    }
  };
}
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

const musicBaseVolume = music.volume;
let clickBaseVolume = clickSound.volume;
let userMusicGain = 1;
let userEffectsGain = 1;
let userMusicEnabled = true;
let userEffectsEnabled = true;
const eventSounds = new Map();
function applyUserGains() {
  music.volume = musicBaseVolume * (userMusicEnabled ? userMusicGain : 0);
  clickSound.volume = clickBaseVolume * (userEffectsEnabled ? userEffectsGain : 0);
  for (const [sound, { base }] of eventSounds) sound.volume = base * (userEffectsEnabled ? userEffectsGain : 0);
}
function setUserGains(musicGain, effectsGain, musicEnabled, effectsEnabled) {
  if (![musicGain, effectsGain].every(value => Number.isFinite(value) && value >= 0 && value <= 1) ||
      typeof musicEnabled !== 'boolean' || typeof effectsEnabled !== 'boolean')
    throw new TypeError('Menu user gains require finite 0..1 values and boolean channel switches');
  userMusicGain = musicGain; userEffectsGain = effectsGain;
  userMusicEnabled = musicEnabled; userEffectsEnabled = effectsEnabled;
  applyUserGains();
}

function stopEvents() {
  for (const [sound, { release }] of eventSounds) {
    sound.pause(); sound.removeAttribute('src'); sound.load(); release();
  }
}
function resetLocalAssets() {
  generation++;
  stopEvents();
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
    if (name === 'click') { clickBaseVolume = descriptor.settings.volume; clickNoDuplicates = descriptor.settings.noDuplicates; }
    applyUserGains();
    // Existing launcher music loop/volume policy is preserved, not native setting proof.
    player.load();
  }
  if (musicSource && enabled && !introActive) unlock();
  return { ...localSources, musicVolumePolicy: 'existing browser policy; original user volume unverified' };
}
window.addEventListener('pagehide', resetLocalAssets);
const report = error => console.warn('Menu audio:', error.message);

function unlock() {
  for (const context of scriptContexts) {
    if (context.state === 'suspended') context.resume().catch(report);
  }
  if (unlocked || !enabled || introActive || (localMode && !localSources.music)) return;
  unlocked = true;
  music.play().catch(error => { unlocked = false; report(error); });
}
document.addEventListener('pointerdown', unlock, { capture: true });
document.addEventListener('keydown', unlock, { capture: true });

globalThis.peacecraft3Audio = {
  setLocalAssets,
  setUserGains,
  resetLocalAssets,
  stopEvents,
  pauseForIntro() { introActive = true; music.pause(); clickSound.pause(); stopEvents(); },
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
    if (!Number.isFinite(volume) || volume < 0 || volume > 1) throw new TypeError('Event audio requires a finite source base volume');
    const url = URL.createObjectURL(new Blob([bytes], { type: 'audio/wav' }));
    const sound = new Audio(url);
    const release = () => {
      if (!eventSounds.delete(sound)) return false;
      URL.revokeObjectURL(url); return true;
    };
    eventSounds.set(sound, { base: volume, release });
    sound.volume = volume * (userEffectsEnabled ? userEffectsGain : 0);
    sound.muted = !enabled;
    sound.addEventListener('ended', release, { once: true });
    sound.play().catch(error => { if (release()) report(error); });
  },
  status() {
    return { enabled, unlocked, clicks, introActive, localMode, localSources, localUrlCount: localUrls.length, eventSoundCount: eventSounds.size,
      musicBaseVolume, clickBaseVolume, userMusicGain, userEffectsGain, userMusicEnabled, userEffectsEnabled, musicVolume: music.volume, musicVolumePolicy: 'existing browser policy; original user volume unverified', musicTime: music.currentTime, musicPaused: music.paused,
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
  for (const sound of eventSounds.keys()) sound.muted = !enabled;
  toggle.textContent = enabled ? 'Mute sound' : 'Enable sound';
  toggle.setAttribute('aria-pressed', String(!enabled));
  document.dispatchEvent(new CustomEvent('peacecraft3-sound-changed', { detail: { enabled } }));
  if (enabled) unlock();
});
