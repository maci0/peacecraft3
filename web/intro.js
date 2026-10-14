// Original local cinematics. Movie data stays on this machine/browser.
let played = false;
let active = false;
let finished = false;
let reason = null;
let section = null;
let video = null;
let previousCanvasHidden = false;
let loadTimer = null;

function finish(why) {
  if (!active) return;
  active = false;
  finished = true;
  reason = why;
  clearTimeout(loadTimer);
  video.pause();
  video.removeAttribute('src');
  video.load();
  section.hidden = true;
  const canvas = document.querySelector('#launcher-canvas');
  if (canvas) {
    canvas.hidden = previousCanvasHidden;
    if (!canvas.hidden) canvas.focus({ preventScroll: true });
  }
  globalThis.peacecraft3Audio?.resumeAfterIntro?.();
  document.dispatchEvent(new CustomEvent('peacecraft3-intro-finished', { detail: why }));
}

function mount() {
  const stage = document.querySelector('#stage');
  if (!stage) return false;
  section = document.createElement('section');
  section.className = 'intro-player';
  section.setAttribute('aria-label', 'Original Warcraft III opening cinematic');
  video = document.createElement('video');
  video.className = 'intro-video';
  video.playsInline = true;
  video.muted = globalThis.peacecraft3Audio?.status?.().enabled === false;
  video.preload = 'auto';
  video.setAttribute('aria-label', 'Opening cinematic');
  const skip = document.createElement('button');
  skip.type = 'button';
  skip.className = 'intro-skip';
  skip.textContent = 'Skip cinematic (Esc)';
  skip.addEventListener('click', () => finish('skipped'));
  video.addEventListener('ended', () => finish('ended'));
  video.addEventListener('error', () => finish('unavailable'));
  video.addEventListener('playing', () => clearTimeout(loadTimer));
  section.append(video, skip);
  stage.append(section);
  return true;
}

document.addEventListener('keydown', event => {
  if (active && event.key === 'Escape') { event.preventDefault(); finish('skipped'); }
});
document.addEventListener('peacecraft3-sound-changed', event => {
  if (video) video.muted = !event.detail.enabled;
});

globalThis.peacecraft3Intro = {
  play(expansion) {
    if (played) return false;
    played = true;
    active = true;
    finished = false;
    reason = null;
    if (document.documentElement.dataset.localAssets === 'true' || new URLSearchParams(location.search).has('local')) {
      active = false;
      finished = true;
      reason = 'unavailable';
      return false;
    }
    if (!mount()) { active = false; finished = true; reason = 'unavailable'; return false; }
    const canvas = document.querySelector('#launcher-canvas');
    if (canvas) { previousCanvasHidden = canvas.hidden; canvas.hidden = true; }
    globalThis.peacecraft3Audio?.pauseForIntro?.();
    // This is a converted copy of the user's original movie, never a downloaded substitute.
    video.src = expansion ? './assets/intro-tft.mp4' : './assets/intro-roc.mp4';
    loadTimer = setTimeout(() => finish('unavailable'), 10000);
    video.play().catch(() => finish('unavailable'));
    section.querySelector('button').focus({ preventScroll: true });
    return true;
  },
  take_finished() { const result = finished; finished = false; return result; },
  status() { return { played, active, reason, time: video?.currentTime ?? 0, duration: video?.duration ?? 0 }; },
};
