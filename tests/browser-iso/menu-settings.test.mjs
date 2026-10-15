import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import vm from 'node:vm';

const source = await readFile(new URL('../../web/menu-settings.js', import.meta.url), 'utf8');
function preview(width, height, availableWidth, availableHeight) {
  const canvas = { width, height, clientWidth: width, clientHeight: height, style: {} };
  const stage = { clientWidth: availableWidth, clientHeight: availableHeight, dataset: {}, scrollLeft: 0, scrollTop: 0,
    listeners: {}, addEventListener(type, callback, options) { assert.equal(options.capture, true); assert.equal(options.passive, false); this.listeners[type] = callback; } };
  let click;
  const button = { attributes: {}, addEventListener: (type, callback) => { assert.equal(type, 'click'); click = callback; },
    setAttribute(name, value) { this.attributes[name] = value; } };
  let update;
  const observed = [];
  const context = { document: { querySelector: selector => ({ '#stage': stage, '#launcher-canvas': canvas, '#preview-size': button })[selector] ?? null },
    ResizeObserver: class { constructor(callback) { update = callback; } observe(element) { observed.push(element); } } };
  vm.runInNewContext(source, context);
  update();
  return { canvas, stage, observed, update, button, click };
}
test('preview fits a selected resolution without changing its layout or backing dimensions', () => {
  const { canvas, observed } = preview(1280, 720, 360, 480);
  assert.equal(canvas.style.transform, 'translate(0px, 138.75px) scale(0.28125)');
  assert.deepEqual([canvas.width, canvas.height, canvas.clientWidth, canvas.clientHeight], [1280, 720, 1280, 720]);
  assert.equal(observed.length, 2);
});
test('preview follows container and requested canvas changes independently', () => {
  const { canvas, stage, update } = preview(640, 480, 960, 720);
  assert.equal(canvas.style.transform, 'translate(160px, 120px) scale(1)');
  stage.clientWidth = 320; stage.clientHeight = 240; update();
  assert.equal(canvas.style.transform, 'translate(0px, 0px) scale(0.5)');
  canvas.clientWidth = 800; canvas.clientHeight = 600; update();
  assert.equal(canvas.style.transform, 'translate(0px, 0px) scale(0.4)');
});
test('temporarily zero-sized preview retains its last valid transform', () => {
  const { canvas, stage, update } = preview(640, 480, 320, 240);
  const previous = canvas.style.transform;
  stage.clientWidth = 0; update();
  assert.equal(canvas.style.transform, previous);
});

test('display choices derive from the preview container and screen, never a startup bitmap', () => {
  const canvas = { width: 300, height: 150 };
  const stage = { clientWidth: 744, clientHeight: 418, addEventListener() {} };
  const context = { document: { querySelector: selector => ({ '#stage': stage, '#launcher-canvas': canvas })[selector] ?? null }, devicePixelRatio: 1,
    screen: { width: 1366, height: 768 } };
  vm.runInNewContext(source, context);
  const modes = () => JSON.parse(context.peacecraft3Settings.displayModes());
  assert.deepEqual(modes(), [[744, 418], [1366, 768]]);
  canvas.width = 1366; canvas.height = 768;
  assert.deepEqual(modes(), [[744, 418], [1366, 768]]);
  stage.clientWidth = 358; stage.clientHeight = 464;
  context.devicePixelRatio = 2;
  assert.deepEqual(modes(), [[716, 928], [2732, 1536]]);
});

test('actual-size preview stays readable without modifying selected render or layout size', () => {
  const { canvas, stage, button, click, update } = preview(1280, 720, 360, 480);
  click();
  assert.equal(stage.dataset.previewSize, 'actual');
  assert.equal(button.attributes['aria-pressed'], 'true');
  assert.equal(canvas.style.transform, 'translate(0px, 0px) scale(1)');
  stage.clientWidth = 320; stage.clientHeight = 400; update();
  assert.equal(canvas.style.transform, 'translate(0px, 0px) scale(1)');
  assert.deepEqual([canvas.width, canvas.height, canvas.clientWidth, canvas.clientHeight], [1280, 720, 1280, 720]);
  stage.scrollLeft = 700; stage.scrollTop = 250;
  click();
  assert.equal(stage.dataset.previewSize, 'fit');
  assert.equal(button.attributes['aria-pressed'], 'false');
  assert.deepEqual([stage.scrollLeft, stage.scrollTop], [0, 0]);
  assert.equal(canvas.style.transform, 'translate(0px, 110px) scale(0.25)');
});

test('actual-size wheel routing leaves fit-mode and modified input untouched', () => {
  const { stage, click } = preview(1280, 720, 360, 480);
  const wheel = (deltaMode = 0, ctrlKey = false) => {
    let handled = 0;
    stage.listeners.wheel({ deltaMode, ctrlKey, deltaX: 40, deltaY: 90,
      preventDefault() { handled++; }, stopPropagation() { handled++; } });
    return handled;
  };
  assert.equal(wheel(), 0);
  click();
  assert.equal(wheel(), 2);
  assert.deepEqual([stage.scrollLeft, stage.scrollTop], [40, 90]);
  assert.equal(wheel(0, true), 0);
  assert.equal(wheel(9), 0);
  assert.equal(wheel(2), 2);
  assert.deepEqual([stage.scrollLeft, stage.scrollTop], [40 + 40 * 360, 90 + 90 * 480]);
  click();
  assert.equal(wheel(), 0);
});
