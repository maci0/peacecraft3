// Browser display observations and local user preferences; no remote storage.
const key = 'peacecraft3.menu-settings.v1';
function bounded(text, limit) {
  if (!Number.isSafeInteger(limit) || limit < 0) throw new TypeError('Settings limit must be a nonnegative integer');
  if (typeof text !== 'string' || text.length > limit || new TextEncoder().encode(text).byteLength > limit)
    throw new RangeError('Settings exceed the preference byte limit');
  return text;
}
globalThis.peacecraft3Settings = {
  displayModes() {
    const modes = [];
    const add = (width, height) => {
      if (Number.isInteger(width) && Number.isInteger(height) && width > 0 && height > 0 &&
          width <= 0xffffffff && height <= 0xffffffff && !modes.some(mode => mode[0] === width && mode[1] === height))
        modes.push([width, height]);
    };
    const ratio = globalThis.devicePixelRatio;
    if (Number.isFinite(ratio) && ratio > 0) {
      // The preview's content box remains measurable when a selected canvas mode
      // changes. Do not treat the canvas's temporary startup bitmap as a mode.
      const stage = document.querySelector('#stage');
      if (stage) add(Math.round(stage.clientWidth * ratio), Math.round(stage.clientHeight * ratio));
      if (globalThis.screen) add(Math.round(screen.width * ratio), Math.round(screen.height * ratio));
    }
    return JSON.stringify(modes);
  },
  resumeParentSizing() {
    const canvas = document.querySelector('#launcher-canvas');
    if (!canvas) throw new Error('Menu canvas is unavailable');
    canvas.style.width = canvas.style.height = '100%';
    fitPreview();
  },
  load(limit) {
    const text = localStorage.getItem(key);
    return text === null ? undefined : bounded(text, limit);
  },
  save(text, limit) {
    bounded(text, limit);
    if (typeof globalThis.peacecraft3Audio?.setUserGains !== 'function')
      throw new Error('Menu audio settings are unavailable');
    localStorage.setItem(key, text);
  },
};

// Preserve the requested render resolution while fitting its preview into the page.
// CSS transforms leave the canvas layout box (and winit's resize observations)
// unchanged. Transformed picking is checked by the browser interaction gate.
function fitPreview() {
  const canvas = document.querySelector('#launcher-canvas');
  const stage = document.querySelector('#stage');
  if (!canvas || !stage) return;
  const width = canvas.clientWidth, height = canvas.clientHeight;
  const availableWidth = stage.clientWidth, availableHeight = stage.clientHeight;
  if (![width, height, availableWidth, availableHeight].every(value => Number.isFinite(value) && value > 0)) return;
  const actual = stage.dataset.previewSize === 'actual';
  const scale = actual ? 1 : Math.min(1, availableWidth / width, availableHeight / height);
  const x = actual ? 0 : (availableWidth - width * scale) / 2;
  const y = actual ? 0 : (availableHeight - height * scale) / 2;
  canvas.style.transform = `translate(${x}px, ${y}px) scale(${scale})`;
}
if (typeof ResizeObserver === 'function') {
  const observer = new ResizeObserver(fitPreview);
  for (const selector of ['#stage', '#launcher-canvas']) {
    const element = document.querySelector(selector);
    if (element) observer.observe(element);
  }
}

const previewSize = document.querySelector('#preview-size');
const previewStage = document.querySelector('#stage');
if (previewSize && previewStage) {
  previewSize.addEventListener('click', () => {
    const actual = previewStage.dataset.previewSize !== 'actual';
    previewStage.dataset.previewSize = actual ? 'actual' : 'fit';
    previewSize.setAttribute('aria-pressed', String(actual));
    previewStage.scrollLeft = previewStage.scrollTop = 0;
    fitPreview();
  });
}

if (previewStage) {
  previewStage.addEventListener('wheel', event => {
    if (previewStage.dataset.previewSize !== 'actual' || event.ctrlKey ||
        ![0, 1, 2].includes(event.deltaMode)) return;
    const line = event.deltaMode === 1 ? Number.parseFloat(getComputedStyle(previewStage).lineHeight) ||
      Number.parseFloat(getComputedStyle(previewStage).fontSize) : 1;
    const xUnit = event.deltaMode === 2 ? previewStage.clientWidth : line;
    const yUnit = event.deltaMode === 2 ? previewStage.clientHeight : line;
    if (![event.deltaX, event.deltaY, xUnit, yUnit].every(Number.isFinite)) return;
    previewStage.scrollLeft += event.deltaX * xUnit;
    previewStage.scrollTop += event.deltaY * yUnit;
    event.preventDefault();
    event.stopPropagation();
  }, { capture: true, passive: false });
}
