// Original TFT audio. Browsers require a gesture before audible playback.
const music = new Audio('./assets/menu-music.mp3');
music.loop = true;
const clickSound = new Audio('./assets/menu-click.wav');
clickSound.volume = 100 / 127; // GlueScreenClick in UISounds.slk.
let enabled = true;
let unlocked = false;
let clicks = 0;
let introActive = false;
const report = error => console.warn('Menu audio:', error.message);

function unlock() {
  if (unlocked || !enabled || introActive) return;
  unlocked = true;
  music.play().catch(error => { unlocked = false; report(error); });
}
document.addEventListener('pointerdown', unlock, { capture: true });
document.addEventListener('keydown', unlock, { capture: true });

globalThis.peacecraft3Audio = {
  pauseForIntro() { introActive = true; music.pause(); clickSound.pause(); },
  resumeAfterIntro() { introActive = false; unlocked = false; music.currentTime = 0; unlock(); },
  click() {
    if (!enabled || introActive) return;
    unlock();
    // The original GlueScreenClick is NODUPLICATES.
    if (!clickSound.paused && !clickSound.ended) return;
    clickSound.currentTime = 0;
    clicks++;
    clickSound.play().catch(report);
  },
  status() {
    return { enabled, unlocked, clicks, introActive, musicTime: music.currentTime, musicPaused: music.paused,
      loop: music.loop, clickTime: clickSound.currentTime, clickPaused: clickSound.paused,
      clickVolume: clickSound.volume, muted: music.muted && clickSound.muted };
  },
};

const toggle = document.querySelector('#sound');
toggle.addEventListener('click', () => {
  enabled = !enabled;
  music.muted = clickSound.muted = !enabled;
  toggle.textContent = enabled ? 'Mute sound' : 'Enable sound';
  toggle.setAttribute('aria-pressed', String(!enabled));
  document.dispatchEvent(new CustomEvent('peacecraft3-sound-changed', { detail: { enabled } }));
  if (enabled) unlock();
});
