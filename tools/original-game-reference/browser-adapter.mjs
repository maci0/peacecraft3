import {writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';

export class BrowserAdapterError extends Error {
  constructor(code, message, details) {
    super(message);
    this.name = 'BrowserAdapterError';
    this.code = code;
    this.details = details;
  }
}
const requireValue = (condition, code, message, details) => {
  if (!condition) throw new BrowserAdapterError(code, message, details);
};
const near = (a, b) => Number.isFinite(a) && Math.abs(a - b) < 0.001;

export function checkedGeometry(observed, extent) {
  requireValue(extent.length === 2 && extent.every(n => Number.isSafeInteger(n) && n > 0), 'INVALID_EXTENT', 'Expected positive physical pixel extent');
  const [width, height] = extent;
  const {box, bitmap, client, dpr, visual, viewport, insets} = observed;
  requireValue(dpr === 1 && visual.scale === 1 && visual.offsetX === 0 && visual.offsetY === 0, 'SCALE_MISMATCH', 'Replay requires actual DPR1 without page zoom or visual viewport displacement', observed);
  requireValue(bitmap[0] === width && bitmap[1] === height && client[0] === width && client[1] === height && near(box.width, width) && near(box.height, height) && insets.every(value => value === 0), 'CANVAS_EXTENT_MISMATCH', 'Canvas content, CSS box and bitmap must match the requested extent', observed);
  requireValue(near(box.x, Math.round(box.x)) && near(box.y, Math.round(box.y)) && box.x >= 0 && box.y >= 0 && box.x + width <= viewport[0] && box.y + height <= viewport[1], 'CANVAS_CLIPPED', 'Canvas must be pixel aligned and wholly inside the viewport', observed);
  requireValue([visual.pageX, visual.pageY].every(Number.isFinite) && near(visual.pageX, Math.round(visual.pageX)) && near(visual.pageY, Math.round(visual.pageY)), 'PAGE_ORIGIN_MISMATCH', 'Document origin must be pixel aligned', observed);
  const origin = {x: Math.round(box.x), y: Math.round(box.y)};
  return {
    origin,
    clip: {x: origin.x + Math.round(visual.pageX), y: origin.y + Math.round(visual.pageY), width, height, scale: 1},
  };
}

export function physicalPoint(origin, extent, x, y) {
  requireValue(Number.isSafeInteger(x) && Number.isSafeInteger(y) && x >= 0 && y >= 0 && x < extent[0] && y < extent[1], 'INPUT_OUTSIDE_CANVAS', 'Pointer coordinates must name a physical pixel inside the canvas', {x, y});
  return {x: origin.x + x, y: origin.y + y};
}

export function checkPointerPosition(previous, phase, point) {
  requireValue(phase === 'move' || (previous && previous.x === point.x && previous.y === point.y), 'POINTER_POSITION_MISMATCH', 'Down/up require a preceding explicit move to the same point');
}

export function checkHeldButton(held, phase, button) {
  if (phase !== 'move') requireValue(held.has(button) === (phase === 'up'), 'BUTTON_STATE_MISMATCH', 'Pointer down/up must form a balanced button sequence', {phase, button});
}

export function physicalKey(key) {
  if (typeof key === 'string' && /^[a-z]$/.test(key)) return 'Key' + key.toUpperCase();
  if (typeof key === 'string' && /^[0-9]$/.test(key)) return 'Digit' + key;
  return {Shift: 'ShiftLeft', Control: 'ControlLeft', Alt: 'AltLeft'}[key] ?? key;
}

// Own the existing page/session; the caller owns launch, scheduling and cleanup.
export function createBrowserAdapter({page, cdp, width, height}) {
  const extent = [width, height];
  const timeoutMs = 30000;
  requireValue(Number.isSafeInteger(timeoutMs) && timeoutMs > 0, 'INVALID_TIMEOUT', 'Operation timeout must be positive');
  let failed = false;
  let pointer;
  const heldButtons = new Set();
  async function bounded(label, operation) {
    requireValue(!failed, 'ADAPTER_FAILED', 'A previous operation failed; do not retry replay inputs');
    let timer;
    try {
      return await Promise.race([
        operation(),
        new Promise((_, reject) => {timer = setTimeout(() => reject(new BrowserAdapterError('OPERATION_TIMEOUT', `${label} exceeded its operation bound`)), timeoutMs);}),
      ]);
    } catch (error) {
      failed = true;
      throw error;
    } finally {clearTimeout(timer);}
  }
  async function geometry() {
    return bounded('geometry', async () => {
      const observed = await page.evaluate(() => {
        const canvas = document.querySelector('#launcher-canvas');
        if (!canvas) return null;
        const box = canvas.getBoundingClientRect(), style = getComputedStyle(canvas);
        return {box: {x: box.x, y: box.y, width: box.width, height: box.height}, bitmap: [canvas.width, canvas.height], client: [canvas.clientWidth, canvas.clientHeight], viewport: [innerWidth, innerHeight], dpr: devicePixelRatio,
          insets: ['borderLeftWidth', 'borderRightWidth', 'borderTopWidth', 'borderBottomWidth', 'paddingLeft', 'paddingRight', 'paddingTop', 'paddingBottom'].map(name => parseFloat(style[name]))};
      });
      requireValue(observed, 'CANVAS_MISSING', 'Launcher canvas is absent');
      const metrics = await cdp.send('Page.getLayoutMetrics');
      observed.visual = metrics.cssVisualViewport ?? metrics.visualViewport;
      return {...checkedGeometry(observed, extent), observed};
    });
  }
  async function state() {
    return bounded('state', () => page.evaluate(() => {
      const raw = globalThis.peacecraft3?.menu_state?.();
      return raw ? JSON.parse(raw) : null;
    }));
  }
  return {
    state,
    async focus() {
      return bounded('focus', async () => {
        await page.bringToFront();
        const focused = await page.evaluate(() => {
          const canvas = document.querySelector('#launcher-canvas');
          canvas?.focus({preventScroll: true});
          return !!canvas && document.activeElement === canvas && document.hasFocus();
        });
        requireValue(focused, 'FOCUS_REFUSED', 'Canvas/page focus could not be established');
      });
    },
    async input(event) {
      // after_ms belongs to the shared controller so both targets use one schedule.
      requireValue(['pointer', 'key'].includes(event.kind), 'INVALID_EVENT', 'Expected pointer or key event');
      if (event.kind === 'pointer') {
        requireValue(['move', 'down', 'up'].includes(event.phase), 'INVALID_EVENT', 'Invalid pointer phase');
        const {origin} = await geometry();
        const point = physicalPoint(origin, extent, event.x, event.y);
        requireValue(event.phase === 'move' || ['left', 'middle', 'right'].includes(event.button), 'INVALID_EVENT', 'Invalid pointer button');
        checkPointerPosition(pointer, event.phase, point);
        checkHeldButton(heldButtons, event.phase, event.button);
        return bounded('pointer', async () => {
          if (event.phase === 'move') await page.mouse.move(point.x, point.y);
          else await page.mouse[event.phase]({button: event.button});
          pointer = point;
          if (event.phase === 'down') heldButtons.add(event.button);
          if (event.phase === 'up') heldButtons.delete(event.button);
        });
      }
      requireValue(['down', 'up'].includes(event.phase) && typeof event.key === 'string' && event.key.length > 0, 'INVALID_EVENT', 'Invalid key event');
      return bounded('key', () => page.keyboard[event.phase](physicalKey(event.key)));
    },
    async capture(path) {
      const geometryBefore = await geometry();
      const stateBefore = await state();
      return bounded('capture', async () => {
        // Playwright restores screen emulation after Chromium's clipped capture.
        const clip = {...geometryBefore.origin, width, height};
        const bytes = await page.screenshot({type: 'png', clip, caret: 'initial'});
        requireValue(bytes.length >= 24 && bytes.subarray(0, 8).equals(Buffer.from([137,80,78,71,13,10,26,10])) && bytes.readUInt32BE(16) === extent[0] && bytes.readUInt32BE(20) === extent[1], 'CAPTURE_EXTENT_MISMATCH', 'Captured PNG does not match requested physical extent');
        const geometryAfter = await geometry();
        const stateAfter = await state();
        requireValue(JSON.stringify(geometryBefore.clip) === JSON.stringify(geometryAfter.clip) && JSON.stringify(geometryBefore.origin) === JSON.stringify(geometryAfter.origin), 'GEOMETRY_CHANGED', 'Canvas geometry changed during capture');
        await writeFile(path, bytes, {flag: 'wx'});
        return {path, sha256: createHash('sha256').update(bytes).digest('hex'), geometry: geometryBefore,
          state_before: stateBefore, state_after: stateAfter,
          state_changed_during_capture: JSON.stringify(stateBefore) !== JSON.stringify(stateAfter)};
      });
    },
  };
}
