import assert from 'node:assert/strict';
import {mkdtemp, readFile, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {test} from 'node:test';
import {checkedGeometry, physicalPoint, checkPointerPosition, checkHeldButton, createBrowserAdapter} from './browser-adapter.mjs';
const observed = {box:{x:20,y:0,width:80,height:60},bitmap:[80,60],client:[80,60],viewport:[200,150],screen:[200,150],dpr:1,insets:[0,0,0,0,0,0,0,0],visual:{scale:1,offsetX:0,offsetY:0,pageX:0,pageY:320}};
const actual = checkedGeometry(observed,[80,60]);
assert.deepEqual(actual,{origin:{x:20,y:0},clip:{x:20,y:320,width:80,height:60,scale:1}});
assert.deepEqual(physicalPoint(actual.origin,[80,60],79,59),{x:99,y:59});
for(const [x,y] of [[-1,0],[80,0],[0,60],[0.1,0],[NaN,0]]) assert.throws(()=>physicalPoint(actual.origin,[80,60],x,y),{code:'INPUT_OUTSIDE_CANVAS'});
for(const change of [{screen:undefined},{screen:[0,150]},{dpr:2},{bitmap:[81,60]},{client:[79,60]},{box:{...observed.box,x:20.5}},{box:{...observed.box,x:150}},{insets:[1]},{visual:{...observed.visual,scale:2}},{visual:{...observed.visual,pageY:320.5}}]) assert.throws(()=>checkedGeometry({...observed,...change},[80,60]));
checkPointerPosition(undefined,'move',{x:20,y:30});
checkPointerPosition({x:20,y:30},'down',{x:20,y:30});
checkPointerPosition({x:20,y:30},'up',{x:20,y:30});
assert.throws(()=>checkPointerPosition(undefined,'down',{x:20,y:30}),{code:'POINTER_POSITION_MISMATCH'});
assert.throws(()=>checkPointerPosition({x:20,y:30},'up',{x:21,y:30}),{code:'POINTER_POSITION_MISMATCH'});
checkHeldButton(new Set(),'move',undefined);
checkHeldButton(new Set(),'down','left');
checkHeldButton(new Set(['left']),'up','left');
assert.throws(()=>checkHeldButton(new Set(['left']),'down','left'),{code:'BUTTON_STATE_MISMATCH'});
assert.throws(()=>checkHeldButton(new Set(),'up','left'),{code:'BUTTON_STATE_MISMATCH'});
console.log('Pure geometry: document crop versus viewport input, physical bounds, scale, clipping, padding, alignment pass. No browser adapter integration is asserted.');

test('capture samples geometry and state together using two renderer evaluations', async t => {
  const output = await mkdtemp(join(tmpdir(), 'pcx-reference-observation-'));
  t.after(() => rm(output, {recursive: true}));
  let evaluations = 0, phase = 'before';
  const bytes = Buffer.alloc(24);
  bytes.set([137,80,78,71,13,10,26,10]);
  bytes.writeUInt32BE(80, 16); bytes.writeUInt32BE(60, 20);
  const adapter = createBrowserAdapter({width: 80, height: 60,
    page: {
      async evaluate(_fn, includeState) {
        evaluations++;
        assert.equal(includeState, true);
        return {observed: {...observed}, state: {phase}};
      },
      async screenshot() {assert.fail('Canvas capture must not prepare DOM fonts or styles');},
    },
    cdp: {async send(method, options) {
      if (method === 'Page.getLayoutMetrics') return {cssVisualViewport: observed.visual};
      if (method === 'Emulation.setDeviceMetricsOverride') {
        assert.deepEqual(options, {width:200,height:150,screenWidth:200,screenHeight:150,deviceScaleFactor:1,mobile:false,dontSetVisibleSize:true});
        return {};
      }
      assert.equal(method, 'Page.captureScreenshot');
      assert.deepEqual(options, {format:'png',clip:actual.clip,captureBeyondViewport:false});
      phase = 'after'; return {data:bytes.toString('base64')};
    }},
  });
  const capture = await adapter.capture(join(output, 'sample.png'));
  assert.equal(evaluations, 2);
  assert.deepEqual(capture.state_before, {phase: 'before'});
  assert.deepEqual(capture.state_after, {phase: 'after'});
  assert.equal(capture.state_changed_during_capture, true);
  assert.deepEqual(capture.geometry.clip, actual.clip);
});

test('capture has a finite separate budget and refuses reuse after timeout', async t => {
  t.mock.timers.enable({apis: ['setTimeout']});
  let captureStarted = false, rejected = false;
  const adapter = createBrowserAdapter({width:80,height:60,
    page: {async evaluate() {return {observed: {...observed}, state: null};}},
    cdp: {async send(method) {
      if (method === 'Page.getLayoutMetrics') return {cssVisualViewport: observed.visual};
      if (method === 'Emulation.setDeviceMetricsOverride') return {};
      assert.equal(method, 'Page.captureScreenshot'); captureStarted = true;
      return new Promise(() => {});
    }},
  });
  const pending = adapter.capture('/unused-timeout.png').catch(error => {rejected = true; throw error;});
  for (let n=0;n<20 && !captureStarted;n++) await Promise.resolve();
  assert(captureStarted);
  t.mock.timers.tick(30001); await Promise.resolve(); assert.equal(rejected,false);
  t.mock.timers.tick(59999);
  await assert.rejects(pending, {code:'OPERATION_TIMEOUT'});
  await assert.rejects(adapter.capture('/unused-retry.png'), {code:'ADAPTER_FAILED'});
});

test('capture refuses changed screen bounds even when canvas geometry matches', async t => {
  const output = await mkdtemp(join(tmpdir(), 'pcx-reference-screen-'));
  t.after(() => rm(output, {recursive:true}));
  const bytes = Buffer.alloc(24); bytes.set([137,80,78,71,13,10,26,10]);
  bytes.writeUInt32BE(80,16); bytes.writeUInt32BE(60,20);
  let captured = false;
  const adapter = createBrowserAdapter({width:80,height:60,
    page: {async evaluate() {return {observed:{...observed,screen:captured ? [201,150] : observed.screen},state:null};}},
    cdp: {async send(method) {
      if (method === 'Page.getLayoutMetrics') return {cssVisualViewport:observed.visual};
      if (method === 'Emulation.setDeviceMetricsOverride') return {};
      assert.equal(method,'Page.captureScreenshot'); captured = true;
      return {data:bytes.toString('base64')};
    }},
  });
  await assert.rejects(adapter.capture(join(output,'changed.png')), {code:'SCREEN_CHANGED'});
  await assert.rejects(adapter.capture(join(output,'retry.png')), {code:'ADAPTER_FAILED'});
});

test('real browser adapter preserves scrolled synthetic canvas crop and explicit input events',
  {skip: !process.env.PCX_PLAYWRIGHT_MODULE}, async t => {
    const {chromium} = await import(pathToFileURL(process.env.PCX_PLAYWRIGHT_MODULE).href);
    const output = await mkdtemp(join(tmpdir(), 'pcx-reference-adapter-'));
    const browser = await chromium.launch({executablePath: process.env.CHROMIUM ?? '/usr/bin/chromium', args: ['--no-sandbox']});
    t.after(async () => {await browser.close(); await rm(output, {recursive: true});});
    const page = await browser.newPage({viewport: {width: 200, height: 150}, deviceScaleFactor: 1});
    await page.setContent('<style>body{margin:0;background:red}header{height:320px}canvas{display:block;margin-left:20px;width:80px;height:60px;outline:none}footer{height:700px}</style><header></header><canvas id="launcher-canvas" width="80" height="60" tabindex="0"></canvas><footer></footer>');
    await page.evaluate(() => {
      const canvas = document.querySelector('canvas'), context = canvas.getContext('2d');
      context.fillStyle = 'rgb(0,255,0)'; context.fillRect(0, 0, 80, 60);
      canvas.scrollIntoView({block: 'start', behavior: 'instant'});
      globalThis.peacecraft3 = {menu_state: () => JSON.stringify({screen: 'Synthetic'})};
      globalThis.events = [];
      for (const type of ['pointermove', 'pointerdown', 'pointerup', 'keydown', 'keyup'])
        canvas.addEventListener(type, event => events.push({type, key: event.key, code: event.code, shift: event.shiftKey, x: event.offsetX, y: event.offsetY}));
    });
    const adapter = createBrowserAdapter({page, cdp: await page.context().newCDPSession(page), width: 80, height: 60});
    await adapter.focus();
    for (const phase of ['move', 'down', 'up'])
      await adapter.input({kind: 'pointer', phase, x: 5, y: 7, ...(phase === 'move' ? {} : {button: 'left'})});
    for (const phase of ['down', 'up']) await adapter.input({kind: 'key', phase, key: 'o'});
    const events = await page.evaluate(() => globalThis.events);
    assert.deepEqual(events.map(event => event.type), ['pointermove', 'pointerdown', 'pointerup', 'keydown', 'keyup']);
    assert(events.slice(0, 3).every(event => event.x === 5 && event.y === 7));
    assert(events.slice(3).every(event => event.key === 'o'));
    await page.evaluate(() => {globalThis.events = [];});
    for (const event of [{phase:'down',key:'Shift'}, {phase:'down',key:'o'}, {phase:'up',key:'o'},
      {phase:'down',key:'1'}, {phase:'up',key:'1'}, {phase:'up',key:'Shift'}])
      await adapter.input({kind:'key',...event});
    const shifted = await page.evaluate(() => globalThis.events);
    assert.deepEqual(shifted.map(event => event.key), ['Shift','O','O','!','!','Shift']);
    assert.deepEqual(shifted.map(event => event.code), ['ShiftLeft','KeyO','KeyO','Digit1','Digit1','ShiftLeft']);
    assert.deepEqual(shifted.map(event => event.shift), [true,true,true,true,true,false]);
    const screenBefore = await page.evaluate(() => [screen.width, screen.height, devicePixelRatio]);
    const path = join(output, 'canvas.png');
    const capture = await adapter.capture(path);
    assert.deepEqual(await page.evaluate(() => [screen.width, screen.height, devicePixelRatio]), screenBefore);
    assert.equal(capture.geometry.clip.y, 320);
    assert.deepEqual(capture.state_before, {screen: 'Synthetic'});
    assert.deepEqual(capture.state_after, {screen: 'Synthetic'});
    assert.equal(capture.state_changed_during_capture, false);
    const pixelsMatch = await page.evaluate(async base64 => {
      const image = await createImageBitmap(await (await fetch('data:image/png;base64,' + base64)).blob());
      const canvas = new OffscreenCanvas(image.width, image.height), context = canvas.getContext('2d');
      context.drawImage(image, 0, 0);
      const bytes = context.getImageData(0, 0, image.width, image.height).data;
      let incorrect = 0, first;
      for (let at = 0; at < bytes.length; at += 4) {
        if (![0, 255, 0, 255].every((value, offset) => bytes[at + offset] === value)) {
          incorrect++; first ??= {pixel: at / 4, rgba: [...bytes.slice(at, at + 4)]};
        }
      }
      return {extent: [image.width, image.height], incorrect, first};
    }, (await readFile(path)).toString('base64'));
    assert.deepEqual(pixelsMatch.extent, [80, 60]);
    assert.equal(pixelsMatch.incorrect, 0, JSON.stringify(pixelsMatch));
    const session = await page.context().newCDPSession(page);
    const changing = createBrowserAdapter({page, cdp: {
      async send(method, options) {
        const result = await session.send(method, options);
        if (method === 'Page.captureScreenshot')
          await page.evaluate(() => {globalThis.peacecraft3.menu_state = () => JSON.stringify({screen: 'Changed'});});
        return result;
      },
    }, width: 80, height: 60});
    const changed = await changing.capture(join(output, 'changed.png'));
    assert.deepEqual(changed.state_before, {screen: 'Synthetic'});
    assert.deepEqual(changed.state_after, {screen: 'Changed'});
    assert.equal(changed.state_changed_during_capture, true);
    await assert.rejects(adapter.capture(path), {code: 'EEXIST'});
  });
