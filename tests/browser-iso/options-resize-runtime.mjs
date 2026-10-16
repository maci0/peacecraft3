// Real local browser gate; --self-check performs no browser, network, or ISO reads.
import assert from 'node:assert/strict';
import {readFile, writeFile, mkdir, realpath} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {resolve, relative, sep, dirname} from 'node:path';
import {pathToFileURL} from 'node:url';

const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const equal = (a, b) => JSON.stringify(a) === JSON.stringify(b);
function sliderValue(slider, value) {
  assert(slider && ['min','max','step','value'].every(k=>Number.isFinite(slider[k])), 'Missing runtime slider metadata');
  assert(slider.max>slider.min && slider.step>0, 'Invalid authored slider range');
  return Math.min(slider.max,Math.max(slider.min,slider.min+Math.round((value-slider.min)/slider.step)*slider.step));
}
function dimensions(value) {
  assert(Array.isArray(value) && value.length === 2 && value.every(n => Number.isInteger(n) && n > 0 && n <= 0xffffffff), 'Invalid dimensions');
  return value;
}
function distinctChoice(controls, old) {
  const choices = controls.filter(c => c.name.startsWith('ResolutionChoice:') && !c.disabled).map(c => {
    const match = /^ResolutionChoice:(\d+)x(\d+)$/.exec(c.name);
    assert(match, 'Malformed resolution diagnostic');
    return {name: c.name, size: dimensions(match.slice(1).map(Number))};
  });
  assert.equal(new Set(choices.map(c => c.name)).size, choices.length, 'Duplicate choice');
  const choice = choices.find(c => !equal(c.size, old));
  assert(choice, 'Host exposes no distinct selectable portable render size');
  return choice;
}
function physicalRect(rect, bounds, physical) {
  dimensions(physical);
  assert(bounds && rect?.length === 4 && rect.every(Number.isFinite) && rect[2] > 0 && rect[3] > 0, 'Invalid runtime rectangle');
  return {x: bounds.x + rect[0] / physical[0] * bounds.width, y: bounds.y + rect[1] / physical[1] * bounds.height,
    width: rect[2] / physical[0] * bounds.width, height: rect[3] / physical[1] * bounds.height};
}
function receiptFiles(receipt, origin) {
  assert.equal(Object.keys(receipt.files ?? {}).length, 24, 'Require the 24-file package receipt');
  return new Map(Object.entries(receipt.files).map(([name, item]) => {
    assert(!name.startsWith('/') && !name.split('/').some(v => v === '..' || v === '.') && !/[?#\\]/.test(name));
    assert(!/\.(iso|mpq|fdf|blp|ttf|wav|mp3)$/i.test(name) && !name.startsWith('assets/'), 'Original asset in package receipt');
    assert(Number.isSafeInteger(item.bytes) && item.bytes >= 0 && /^[0-9a-f]{64}$/.test(item.sha256));
    const url = new URL(name, origin);
    assert.equal(url.origin, origin.origin);
    return [url.href, item];
  }));
}
function allowedRequest(url, method, body, files) { return method === 'GET' && !body && files.has(url); }
function saveWitness(witness, target) {
  assert.deepEqual(witness.target, target, 'Saved candidate changed');
  assert.deepEqual(witness.bitmap, target, 'Storage ran before canvas bitmap reached target');
  assert.deepEqual(witness.clientPhysical, target, 'Storage ran before client extent reached target');
}
// Current-update press/release qualify the actual batch; their targets are the oracle.
function fastBatchFrame(s, physical) {
  return s?.picking?.press?.length>0 && s.picking.release?.length>0 &&
    s.pointer?.physical?.every((v,i)=>Math.abs(v-physical[i])<2);
}
function fastBatchOracle(s, name) {
  assert.equal(s.picking.recordLimit,16);assert.deepEqual(s.lastAction?.frames,[name]);assert.equal(s.lastAction.truncated,false);
  let sender;
  for(const kind of ['press','release','click']) {
    const events=s.picking[kind];assert(Array.isArray(events)&&events.length===1,kind+' must contain exactly one sender');
    const event=events[0];assert.equal(typeof event.entity,'string');assert(event.entity.length>0,'Missing picking entity');
    if(sender===undefined)sender=event.entity;else assert.equal(event.entity,sender,'Press/release/click sender changed');
    // PostUpdate owner lookup can lose the despawned tab; the exact A action above remains.
    assert(event.owner===name||event.owner===null,kind+' targeted another logical control');
  }
  assert.equal(s.lastAction.screen.startsWith('Options('),true);
  assert.deepEqual(s.picking.pressing??[],[]);assert.deepEqual(s.picking.dragging??[],[]);
}

function selfCheck() {
  const controls = [{name:'ResolutionChoice:731x419'}, {name:'ResolutionChoice:997x613'}];
  assert.deepEqual(distinctChoice(controls, [731,419]).size, [997,613]);
  assert.throws(() => distinctChoice(controls.slice(0,1), [731,419]));
  for (const name of ['ResolutionChoice:0x1','ResolutionChoice:2x3junk','ResolutionChoice:4294967296x1'])
    assert.throws(() => distinctChoice([{name}], [1,1]));
  assert.throws(() => distinctChoice([...controls, controls[0]], [731,419]));
  assert.deepEqual(physicalRect([10,20,30,40], {x:7,y:8,width:200,height:100}, [100,100]), {x:27,y:28,width:60,height:40});
  assert.throws(() => physicalRect([1,2,0,4], {x:0,y:0,width:1,height:1}, [1,1]));
  const files = new Map([['http://localhost:8/pkg/x.js', {}]]);
  assert(allowedRequest('http://localhost:8/pkg/x.js','GET',false,files));
  for (const [url, method, body] of [['http://localhost:8/pkg/x.js','POST',false],['http://localhost:8/pkg/x.js','GET',true],['http://elsewhere/x','GET',false],['http://localhost:8/pkg/x.js?x','GET',false]])
    assert(!allowedRequest(url,method,body,files));
  saveWitness({target:[731,419],bitmap:[731,419],clientPhysical:[731,419]}, [731,419]);
  for (const field of ['target','bitmap','clientPhysical']) {
    const witness = {target:[731,419],bitmap:[731,419],clientPhysical:[731,419]}; witness[field] = [997,613];
    assert.throws(() => saveWitness(witness,[731,419]));
  }
  assert.throws(() => receiptFiles({files:{}}, new URL('http://localhost/')));
  assert.equal(sliderValue({min:-30,max:110,step:7,value:1},40),40);
  assert.equal(sliderValue({min:-30,max:110,step:7,value:1},500),110);
  assert.throws(()=>sliderValue({min:0,max:1,step:0,value:0},0));
  const frame={screen:'Options(Sound)',pointer:{physical:[31,47]},picking:{recordLimit:16,press:[{entity:'synthetic-E',owner:'A'}],release:[{entity:'synthetic-E',owner:'A'}],click:[{entity:'synthetic-E',owner:'A'}],pressing:[],dragging:[]},lastAction:{frames:['A'],screen:'Options(Video)',truncated:false}};
  assert(fastBatchFrame(frame,[31,47]));fastBatchOracle(frame,'A');
  for(const kind of ['press','release','click']){const bad=structuredClone(frame);bad.picking[kind][0].owner='B';assert(fastBatchFrame(bad,[31,47]));assert.throws(()=>fastBatchOracle(bad,'A'));}
  for(const kind of ['press','release']){const split=structuredClone(frame);split.picking[kind]=[];assert(!fastBatchFrame(split,[31,47]));}
  assert(!fastBatchFrame(frame,[97,83]));
  const duplicate=structuredClone(frame);duplicate.lastAction.frames.push('A');assert.throws(()=>fastBatchOracle(duplicate,'A'));
  for(let mask=0;mask<8;mask++){const gone=structuredClone(frame);['press','release','click'].forEach((k,i)=>{if(mask&(1<<i))gone.picking[k][0].owner=null;});fastBatchOracle(gone,'A');}
  for(const kind of ['press','release','click']) {
    const different=structuredClone(frame);different.picking[kind][0].entity='synthetic-other';assert.throws(()=>fastBatchOracle(different,'A'));
    for(const entity of [undefined,null,'',7]){const bad=structuredClone(frame);bad.picking[kind][0].entity=entity;assert.throws(()=>fastBatchOracle(bad,'A'));}
    for(const owner of [undefined,'B']){const bad=structuredClone(frame);bad.picking[kind][0].owner=owner;assert.throws(()=>fastBatchOracle(bad,'A'));}
    const extra=structuredClone(frame);extra.picking[kind].push({...extra.picking[kind][0]});assert.throws(()=>fastBatchOracle(extra,'A'));
    const missing=structuredClone(frame);missing.picking[kind]=[];assert.throws(()=>fastBatchOracle(missing,'A'));
  }
  for(const frames of [[],['B'],['A','B'],['A','A'],null]){const bad=structuredClone(frame);bad.lastAction.frames=frames;assert.throws(()=>fastBatchOracle(bad,'A'));}
  const truncated=structuredClone(frame);truncated.lastAction.truncated=true;assert.throws(()=>fastBatchOracle(truncated,'A'));

  console.log(JSON.stringify({ok:true,selfCheck:true,browserLaunched:false,networkRequests:0,originalReads:0}));
}

async function run() {
  const applyProbe=process.argv.includes('--apply-probe');
  const fastClick=process.argv.includes('--fast-click');
  assert(!fastClick||!applyProbe,'--fast-click cannot be combined with diagnostic-only --apply-probe');
  const sliderRun=process.argv.includes('--slider')||applyProbe;
  const env = process.env, root = await realpath(env.PCX_REPO_ROOT ?? process.cwd());
  assert.equal(env.PCX_OPTIONS_WASM_READY, '1', 'Require an explicitly prepared fresh release');
  for (const key of ['PCX_OPTIONS_ORIGIN','PCX_OPTIONS_BUILD_RECEIPT','PCX_OPTIONS_PREPARED','PCX_OPTIONS_RUN_OUTPUT']) assert(env[key], `Missing ${key}`);
  const origin = new URL(env.PCX_OPTIONS_ORIGIN);
  assert.equal(origin.protocol,'http:'); assert(['localhost','127.0.0.1','[::1]'].includes(origin.hostname));
  assert(!origin.username && !origin.password && !origin.search && !origin.hash);
  assert(origin.pathname.endsWith('/') || origin.pathname.endsWith('/index.html'));
  const packageBase = new URL('./',origin);
  const receiptBytes = await readFile(env.PCX_OPTIONS_BUILD_RECEIPT), files = receiptFiles(JSON.parse(receiptBytes),packageBase);
  assert(files.has(new URL('index.html',packageBase).href));
  const fixtureBytes = await readFile(env.PCX_OPTIONS_PREPARED), fixture = JSON.parse(fixtureBytes);
  assert.equal(typeof fixture.isoPath,'string'); // Only metadata is read here; File import stays browser-local.
  const output = resolve(env.PCX_OPTIONS_RUN_OUTPUT), parent = await realpath(dirname(output));
  const scratch = await realpath(resolve(root,'.scratch'));
  const rel = relative(scratch,parent); assert(rel === '' || (!rel.startsWith('..'+sep) && rel !== '..' && !rel.startsWith(sep)), 'Output must be inside ignored .scratch');
  await mkdir(output); // A fresh directory is mandatory; never overwrite a prior run.
  const save = (name,data) => writeFile(resolve(output,name),JSON.stringify(data,null,2)+'\n',{flag:'wx'});
  const packageChecks = [];
  for (const [url, item] of files) {
    const response = await fetch(url,{redirect:'error'}); assert.equal(response.status,200);
    const bytes = Buffer.from(await response.arrayBuffer());
    assert.equal(bytes.length,item.bytes,url); assert.equal(hash(bytes),item.sha256,url);
    packageChecks.push({url,bytes:bytes.length,sha256:hash(bytes)});
  }
  const settingsURL = new URL('menu-settings.js',packageBase).href; assert(files.has(settingsURL));
  const settingsSource = await (await fetch(settingsURL,{redirect:'error'})).text();
  assert.equal(hash(settingsSource),files.get(settingsURL).sha256);
  const preferenceKey = settingsSource.match(/const key = '([^']+)';/)?.[1]; assert(preferenceKey);
  const {chromium} = await import(pathToFileURL(env.PCX_PLAYWRIGHT_MODULE ?? resolve(root,'.scratch/browser-check/node_modules/playwright-core/index.mjs')));
  // CDP emulation alone does not scale Chromium's device-pixel ResizeObserver box.
  const browser = await chromium.launch({executablePath:env.PCX_CHROMIUM ?? '/usr/bin/chromium',args:['--no-sandbox','--enable-unsafe-swiftshader',...((sliderRun||fastClick)?['--force-device-scale-factor=2']:[])]});
  const requests=[],responses=[],blocked=[],errors=[],sockets=[],logs=[],steps=[],captures=[];
  let page, failure, lastState, passed=false;
  async function poll(check, why) {
    const end = Date.now()+120000;
    while (Date.now()<end) {
      assert.deepEqual(errors,[]); assert.deepEqual(sockets,[]);
      assert(!logs.some(m => /Caught rendering error|Quitting application due to|Attachments have differing sizes/.test(m.text)), 'Renderer stopped');
      if (await check()) return; await new Promise(r => setTimeout(r,100));
    }
    throw Error(why);
  }
  const snapshot = async () => {const s=await page.evaluate(() => { const text=peacecraft3.menu_state(); return text ? JSON.parse(text) : null; });if(s)lastState=s;return s;};
  async function state(predicate, why) { let s; await poll(async()=>{s=await snapshot(); return s && predicate(s);},why); return s; }
  const settled = screen => state(s=>s.menuVisible && !s.transitionActive && s.screen===screen,'Did not settle: '+screen);
  const extent = () => page.locator('#launcher-canvas').evaluate(c=>({bitmap:[c.width,c.height],clientPhysical:[Math.round(c.clientWidth*devicePixelRatio),Math.round(c.clientHeight*devicePixelRatio)]}));
  const prefs = () => page.evaluate(key=>localStorage.getItem(key),preferenceKey);
  const audio = () => page.evaluate(()=>{const a=peacecraft3Audio.status(); return Object.fromEntries(['musicBaseVolume','clickBaseVolume','userMusicGain','userEffectsGain','userMusicEnabled','userEffectsEnabled','musicVolume','clickVolume'].map(k=>[k,a[k]]));});
  async function targetExtent(target) { await poll(async()=>{const e=await extent();return equal(e.bitmap,target)&&equal(e.clientPhysical,target);},'Canvas did not reach '+target); }
  async function controlPoint(name) {
    const s = await state(s=>s.menuVisible && !s.transitionActive && s.controls?.some(c=>c.name===name && !c.disabled),'No enabled control: '+name);
    const c = s.controls.find(c=>c.name===name);
    await page.evaluate(rect=>{
      const stage=document.querySelector('#stage'),canvas=document.querySelector('#launcher-canvas');
      if(stage.dataset.previewSize!=='actual')return;
      const sb=stage.getBoundingClientRect(),cb=canvas.getBoundingClientRect();
      const x=cb.left+(rect[0]+rect[2]/2)/canvas.width*cb.width,y=cb.top+(rect[1]+rect[3]/2)/canvas.height*cb.height;
      if(x<sb.left||x>sb.left+stage.clientWidth)stage.scrollLeft+=x-(sb.left+stage.clientWidth/2);
      if(y<sb.top||y>sb.top+stage.clientHeight)stage.scrollTop+=y-(sb.top+stage.clientHeight/2);
    },c.physicalCanvasRect);
    let bounds=await page.locator('#launcher-canvas').boundingBox(), size=(await extent()).bitmap;
    let r=physicalRect(c.physicalCanvasRect,bounds,size),view=page.viewportSize();
    if(r.y+r.height/2<0||r.y+r.height/2>view.height){await page.evaluate(y=>scrollBy(0,y),r.y+r.height/2-view.height/2);bounds=await page.locator('#launcher-canvas').boundingBox();r=physicalRect(c.physicalCanvasRect,bounds,size);}
    const x=r.x+r.width/2,y=r.y+r.height/2; assert(x>=0&&x<view.width&&y>=0&&y<view.height,'Runtime control outside viewport');
    const expected=[c.physicalCanvasRect[0]+c.physicalCanvasRect[2]/2,c.physicalCanvasRect[1]+c.physicalCanvasRect[3]/2];
    return {s,c,r,x,y,expected,bounds,size};
  }
  async function click(name) {
    const {s,r,x,y,expected,bounds,size}=await controlPoint(name);
    await page.mouse.move(x,y);
    const hovered=await state(s=>s.pointer?.physical?.every((v,i)=>Math.abs(v-expected[i])<2)&&s.controls.some(c=>c.name===name&&c.hovered===true),'UI hover did not reach '+name);
    const input={action:'pointer',name,screen:s.screen,rect:r,canvasBounds:bounds,bitmap:size,pointer:hovered.pointer,viewport:hovered.uiViewportPixels};
    steps.push(input);console.log(JSON.stringify(input));
    await page.mouse.down();const pressed=await state(s=>s.controls.some(c=>c.name===name&&c.pressed),'Press did not reach '+name);await page.mouse.up();
    const released=await state(s=>!s.controls.some(c=>c.name===name&&c.pressed),'Release did not reach '+name);
    steps.push({action:'dispatch',name,press:pressed.picking,release:released.picking,lastAction:released.lastAction,native:await page.evaluate(()=>__pcxNativeInput.slice(-16))});
    return r;
  }
  async function openOptions() { await click('OptionsButton'); const s=await state(s=>s.menuVisible&&!s.transitionActive&&s.screen.startsWith('Options('),'Options did not open'); if(s.screen!=='Options(Video)')await click('VideoButton'); await settled('Options(Video)'); }
  async function chooseOther(old) { await click('ResolutionMenu');const s=await state(s=>s.controls.some(c=>c.name.startsWith('ResolutionChoice:')),'No resolution choices');const choice=distinctChoice(s.controls,old);await click(choice.name);await state(s=>equal(s.draft.resolution,choice.size),'Draft not selected');return choice.size; }
  async function capture(name) {
    await page.evaluate(async()=>{await new Promise(requestAnimationFrame);await new Promise(requestAnimationFrame);});
    const path=resolve(output,name+'.png');
    if(sliderRun)await page.locator('#launcher-canvas').screenshot({path});else await page.screenshot({path,fullPage:true});
    const s=await snapshot();
    if(sliderRun&&s.screen.startsWith('Options('))for(const name of ['GameplayButtonText','VideoButtonText','SoundButtonText']) {
      const label=s.labels.find(l=>l.name===name);
      assert(label?.filled&&label.text.length>0,'Source tab text was not retained in diagnostics: '+name);
    }
    captures.push({name,path,sha256:hash(await readFile(path)),state:s,extent:await extent()});
  }
  async function checkFastClicks() {
    await openOptions();await settled('Options(Video)');
    const client=await page.context().newCDPSession(page),cases=[];
    try {
      for(const item of [
        {name:'fresh-A',a:'SoundButton',b:'VideoButton',screen:'Options(Sound)',finalB:false},
        {name:'A-click-final-B',a:'VideoButton',b:'SoundButton',screen:'Options(Video)',finalB:true},
      ]) {
        const b=await controlPoint(item.b);await page.mouse.move(b.x,b.y);
        const before=await state(s=>s.pointer?.physical?.every((v,i)=>Math.abs(v-b.expected[i])<2)&&
          s.controls.some(c=>c.name===item.b&&c.hovered)&&!s.controls.some(c=>c.name===item.a&&c.hovered)&&
          ['press','release','click'].every(k=>s.picking?.[k]?.length===0)&&!s.controls.some(c=>c.pressed),'Idle hover B was not acknowledged');
        assert(!before.lastAction?.frames?.includes(item.a),'Baseline action already equals the new target');
        const a=await controlPoint(item.a),final= item.finalB?b:a;
        assert.deepEqual(a.bounds,b.bounds,'Resolving tab bounds scrolled the canvas between targets');
        assert(!equal(a.expected,b.expected),'Fast-input controls overlap');
        await page.evaluate(points=>{
          for(const {x,y} of points)if(document.elementFromPoint(x,y)?.id!=='launcher-canvas')throw Error('Runtime control center is occluded by DOM');
          const gate={active:true,records:[],count:0,truncated:false,raf:0,last:null};globalThis.__pcxFastFrames=gate;
          globalThis.__pcxNativeInput.length=0;
          // Rearm the existing read-only diagnostic every browser frame; never override rAF/update.
          peacecraft3.menu_state();
          const tick=()=>{if(!gate.active)return;const raw=peacecraft3.menu_state();if(raw&&raw!==gate.last){
            gate.last=raw;const s=JSON.parse(raw);gate.count++;
            if(gate.records.length===64){gate.records.shift();gate.truncated=true;}
            gate.records.push({screen:s.screen,pointer:s.pointer,picking:s.picking,lastAction:s.lastAction});
          }gate.raf=requestAnimationFrame(tick);};gate.raf=requestAnimationFrame(tick);
        },[a,b]);
        const events=[{type:'mouseMoved',x:a.x,y:a.y,button:'none',buttons:0},
          {type:'mousePressed',x:a.x,y:a.y,button:'left',buttons:1,clickCount:1},
          {type:'mouseReleased',x:a.x,y:a.y,button:'left',buttons:0,clickCount:1}];
        if(item.finalB)events.push({type:'mouseMoved',x:b.x,y:b.y,button:'none',buttons:0});
        let witness,recording,native;
        try {
          // One ordered CDP transport batch; actual snapshot qualification detects any frame split.
          await Promise.all(events.map(event=>client.send('Input.dispatchMouseEvent',{...event,pointerType:'mouse'})));
          await poll(async()=>{recording=await page.evaluate(()=>{const g=__pcxFastFrames;return {records:g.records,count:g.count,truncated:g.truncated};});witness=recording.records.find(s=>fastBatchFrame(s,final.expected));return Boolean(witness);},'UNQUALIFIED '+item.name+': no single current-update press+release with final pointer; no same-frame claim');
          fastBatchOracle(witness,item.a);
          native=await page.evaluate(()=>__pcxNativeInput.filter(e=>['pointerdown','pointerup'].includes(e.type)));
          assert.deepEqual(native.map(e=>[e.type,e.trusted,e.button]),[['pointerdown',true,0],['pointerup',true,0]],'Require one trusted native press/release');
          assert(native.every(e=>e.target==='launcher-canvas'&&Math.abs(e.x-a.x)<=1&&Math.abs(e.y-a.y)<=1),'Native press/release did not occur at actual A canvas point');
          await settled(item.screen);
          cases.push({case:item,before,targets:{a,b},events,witness,recording,native});
        } finally {
          const proof=await page.evaluate(()=>{const g=globalThis.__pcxFastFrames;if(!g)return null;g.active=false;cancelAnimationFrame(g.raf);return {records:g.records,count:g.count,truncated:g.truncated,native:__pcxNativeInput};});
          await save('fast-click-'+item.name+'.json',{case:item,before,targets:{a,b},events,witness,proof});
        }
      }
      await capture('fast-click-qualified');await click('CancelButton');await settled('Main');
      assert.equal(await prefs(),null,'Fast tab input unexpectedly saved settings');
      await save('fast-click-observations.json',{cases,qualified:true,trustedNativeInput:true,diagnosticActions:0,coordinateSource:'runtime physicalCanvasRect'});
    } finally {await client.detach();}
  }
  const witnesses = () => page.evaluate(()=>globalThis.__pcxResizeGate.saves);
  async function checkApplyProbe() {
    await click('OptionsButton');await state(s=>s.menuVisible&&!s.transitionActive&&s.screen.startsWith('Options('),'Options did not open');
    await click('SoundButton');await settled('Options(Sound)');
    await page.setViewportSize({width:370,height:520});
    await state(s=>s.uiViewportPixels?.[0]===0,'Apply probe did not reach the final slider-test viewport');
    const before=await snapshot();await click('MusicVolumeSlider');
    if(equal((await snapshot()).draft,before.applied))await page.keyboard.press('ArrowRight');
    const candidate=(await snapshot()).draft;assert(!equal(candidate,before.applied),'Apply probe is not dirty');
    await capture('apply-probe-before');await click('OKButton');
    const afterMouse=await snapshot(),mouseSaved=await prefs();
    await save('apply-probe-pointer.json',{before,candidate,afterMouse,mouseSaved,diagnosticOnly:true});
    if(!mouseSaved)await page.keyboard.press('Enter');
    await settled('Main');const saved=await prefs();assert(saved);
    assert.deepEqual(JSON.parse(saved).settings,candidate);
    await save('apply-probe.json',{afterMouse,mouseSaved,keyboardNeeded:!mouseSaved,saved:JSON.parse(saved),diagnosticOnly:true});
    await capture('apply-probe-after');
  }
  async function checkSliders() {
    const before=await snapshot(), initialAudio=await audio();
    assert.equal(await prefs(),null);
    const observations=[];
    const near=(actual,expected)=>Math.abs(actual-expected)<0.0001;
    await click('OptionsButton');await state(s=>s.menuVisible&&!s.transitionActive&&s.screen.startsWith('Options('),'Options did not open');
    await capture('slider-options-entry');
    await click('SoundButton');await settled('Options(Sound)');
    // Synthetic host shapes exercise both centered and origin-zero UI viewports.
    for (const [viewport,centered] of [[{width:440,height:350},true],[{width:370,height:520},false]]) {
      await page.setViewportSize(viewport);
      await state(s=>s.uiViewportPixels && (s.uiViewportPixels[0]>0)===centered,'UI viewport did not change');
      for (const name of ['MusicVolumeSlider','SoundVolumeSlider']) {
        const s=await snapshot(), control=s.controls.find(c=>c.name===name&&!c.disabled);
        assert(control,'Enabled slider is missing');
        const {min,max,step}=control.slider, midpoint=sliderValue(control.slider,(min+max)/2);
        await click(name);
        const selected=await state(s=>near(s.draft.values[name],midpoint),'Midpoint click selected the wrong source value: '+name);
        assert(near(selected.controls.find(c=>c.name===name).slider.value,midpoint),'Widget value differs from draft');
        assert.deepEqual(await audio(),initialAudio,'Unapplied slider changed active audio');
        assert.equal(await prefs(),null,'Unapplied slider saved preferences');
        const next=sliderValue(control.slider,midpoint+step);
        await page.keyboard.press('ArrowRight');
        await state(s=>near(s.draft.values[name],next),'Focused slider keyboard step failed: '+name);
        observations.push({name,viewport,dpr:await page.evaluate(()=>devicePixelRatio),uiViewport:selected.uiViewportPixels,source:control.slider,midpoint,next});
        const rect=await click(name);await state(s=>near(s.draft.values[name],midpoint),'Midpoint restore failed');
        const x=rect.x+rect.width/2,y=rect.y+rect.height/2;
        await page.mouse.move(x,y);await page.mouse.down();
        await state(s=>s.controls.some(c=>c.name===name&&c.pressed),'Drag press did not reach '+name);
        await page.mouse.move(x+rect.width*2,y,{steps:8});
        await state(s=>near(s.draft.values[name],max),'Dragging outside the track did not clamp to the authored maximum');
        await page.mouse.up();await state(s=>s.controls.some(c=>c.name===name&&!c.pressed),'Drag release was not observed');
        await page.mouse.move(x,y);await page.waitForTimeout(150);
        assert(near((await snapshot()).draft.values[name],max),'Slider capture persisted after release');
        steps.push({action:'drag-outside-release',name,rect});
        await click(name);await state(s=>near(s.draft.values[name],midpoint),'Midpoint restore after drag failed');
      }
      await capture(centered?'slider-centered':'slider-origin-zero');
    }
    const candidate=(await snapshot()).draft;
    await click('OKButton');await settled('Main');
    const saved=await prefs();assert(saved);const parsed=JSON.parse(saved);
    assert.deepEqual(parsed.settings,candidate);assert.deepEqual((await snapshot()).applied,candidate);
    const active=await audio();
    for(const [name,field] of [['MusicVolumeSlider','userMusicGain'],['SoundVolumeSlider','userEffectsGain']]) {
      const item=observations.find(o=>o.name===name),gain=(candidate.values[name]-item.source.min)/(item.source.max-item.source.min);
      assert(near(active[field],gain),'Applied audio gain differs from authored slider projection');
    }
    await click('OptionsButton');await state(s=>s.menuVisible&&!s.transitionActive&&s.screen.startsWith('Options('),'Options did not reopen');
    await click('SoundButton');await settled('Options(Sound)');
    await click('MusicVolumeSlider');await page.keyboard.press('ArrowRight');
    await state(s=>!equal(s.draft,candidate),'Cancel case did not become dirty');
    await click('CancelButton');await settled('Main');
    assert.equal(await prefs(),saved);assert.deepEqual(await audio(),active);assert.deepEqual((await snapshot()).applied,candidate);
    await capture('slider-committed-cancelled');
    await save('slider-observations.json',{before,observations,candidate,active,saved:parsed});
  }
  try {
    // Synthetic browser host geometry, independent of authored game layout.
    const context=await browser.newContext({viewport:(sliderRun||fastClick)?{width:440,height:350}:{width:1500,height:1000},screen:{width:1280,height:720},deviceScaleFactor:(sliderRun||fastClick)?2:1,serviceWorkers:'block'});
    await context.route('**/*',route=>{const r=route.request();if(allowedRequest(r.url(),r.method(),r.postDataBuffer()!==null,files))return route.continue();blocked.push({url:r.url(),method:r.method()});return route.abort('blockedbyclient');});
    context.on('request',r=>requests.push({url:r.url(),method:r.method(),body:r.postDataBuffer()!==null}));
    context.on('response',r=>responses.push({url:r.url(),status:r.status()}));
    page=await context.newPage(); page.setDefaultTimeout(120000);
    await page.addInitScript(()=>{
      const events=[];globalThis.__pcxNativeInput=events;
      for(const type of ['pointermove','pointerdown','pointerup','pointercancel','gotpointercapture','lostpointercapture','mousedown','mouseup','mouseleave','blur','focus'])addEventListener(type,event=>{
        if(events.length===128)events.shift();
        events.push({type,trusted:event.isTrusted,target:event.target?.id??event.target?.nodeName??null,button:event.button??null,buttons:event.buttons??null,pointer:event.pointerId??null,x:event.clientX??null,y:event.clientY??null,time:event.timeStamp});
      },{capture:true,passive:true});
    });
    page.on('pageerror',e=>errors.push(e.message));
    page.on('websocket',s=>sockets.push(s.url())); page.on('console',m=>logs.push({type:m.type(),text:m.text()}));
    await page.goto(new URL('index.html',packageBase).href,{waitUntil:'domcontentloaded'});
    await page.waitForFunction(()=>globalThis.peacecraft3?.menu_state && globalThis.peacecraft3Settings && globalThis.peacecraft3Audio);
    const pixelCalibration=await page.locator('#launcher-canvas').evaluate(canvas=>new Promise(resolve=>{
      const observer=new ResizeObserver(([entry])=>{observer.disconnect();resolve({expected:[Math.round(entry.contentRect.width*devicePixelRatio),Math.round(entry.contentRect.height*devicePixelRatio)],observed:entry.devicePixelContentBoxSize?.map(s=>[s.inlineSize,s.blockSize])});});
      observer.observe(canvas,{box:'device-pixel-content-box'});
    }));
    if(pixelCalibration.observed)assert.deepEqual(pixelCalibration.observed,[pixelCalibration.expected],'Browser native/emulated pixel scales disagree');
    steps.push({action:'pixel-calibration',...pixelCalibration});
    assert.equal(await page.evaluate(()=>document.documentElement.dataset.localAssets),'true');
    assert.deepEqual(await page.evaluate(()=>peacecraft3.stored_assets()),[]);
    const revision=await page.evaluate(()=>peacecraft3.asset_revision());await page.locator('#iso').setInputFiles(fixture.isoPath);
    await page.waitForFunction(()=>document.querySelectorAll('#iso-candidates input').length===1&&!document.querySelector('#iso').disabled);
    await page.locator('#iso-candidates input').check();await page.locator('#iso-import').click();
    await page.waitForFunction(r=>peacecraft3.asset_revision()===r+1&&!document.querySelector('#iso').disabled,revision);
    assert((await page.evaluate(()=>peacecraft3.local_menu_editions())).includes('tft'));
    await page.locator('#demo-edition').selectOption('tft');await page.locator('#demo-start').click();await settled('Main');
    await poll(async()=>{const e=await extent();return equal(e.bitmap,e.clientPhysical);},'Canvas bitmap and displayed physical extent disagree');
    await page.evaluate(key=>{
      const gate={save:peacecraft3Settings.save,gains:peacecraft3Audio.setUserGains,fault:null,saves:[]};globalThis.__pcxResizeGate=gate;
      peacecraft3Settings.save=function(text,limit){
        const c=document.querySelector('#launcher-canvas'),w={target:JSON.parse(text).settings.resolution,bitmap:[c.width,c.height],clientCss:[c.clientWidth,c.clientHeight],dpr:devicePixelRatio,clientPhysical:[Math.round(c.clientWidth*devicePixelRatio),Math.round(c.clientHeight*devicePixelRatio)],fault:gate.fault,before:localStorage.getItem(key)};
        gate.saves.push(w);if(gate.fault==='storage')throw Error('Injected browser storage save refusal');
        const result=gate.save.call(this,text,limit);w.after=localStorage.getItem(key);return result;
      };
      peacecraft3Audio.setUserGains=function(...args){if(gate.fault==='audio')throw Error('Injected postcommit audio refusal');return gate.gains.apply(this,args);};
    },preferenceKey);
    if(fastClick)await checkFastClicks();
    if(applyProbe) {
      await checkApplyProbe();
    } else if(sliderRun) {
      await checkSliders();
    } else if(!fastClick) {
    await openOptions();const original=await snapshot();await targetExtent(dimensions(original.applied.resolution));
    const initialAudio=await audio(), refused=await chooseOther(original.applied.resolution), refusedDraft=(await snapshot()).draft;
    await page.evaluate(()=>{__pcxResizeGate.fault='storage';});await click('OKButton');
    await state(s=>s.screen==='Options(Video)'&&!s.transitionActive&&equalNotice(s.notice),'Initial failure did not restore render size');
    await targetExtent(original.applied.resolution);assert.equal(await prefs(),null);assert.deepEqual(await audio(),initialAudio);
    assert.deepEqual((await snapshot()).draft,refusedDraft);saveWitness((await witnesses())[0],refused);
    assert.deepEqual(await page.locator('#launcher-canvas').evaluate(c=>[c.style.width,c.style.height]),['100%','100%'],'Initial failure lost parent-following sizing');
    await page.setViewportSize({width:1125,height:750});
    await poll(async()=>{const e=await extent(),parent=await page.locator('#stage').evaluate(s=>[Math.round(s.clientWidth*devicePixelRatio),Math.round(s.clientHeight*devicePixelRatio)]);return equal(e.bitmap,parent)&&equal(e.clientPhysical,parent)&&equal((await snapshot()).applied.resolution,parent);},'Restored canvas did not follow resized parent');
    assert.deepEqual((await snapshot()).draft,refusedDraft);assert.equal(await prefs(),null);
    await capture('00-initial-failure-parent-sizing');
    await page.evaluate(()=>{__pcxResizeGate.fault=null;});await click('CancelButton');await settled('Main');assert.equal(await prefs(),null);
    await openOptions();const initial=await snapshot();await targetExtent(dimensions(initial.applied.resolution));const first=await chooseOther(initial.applied.resolution);
    await click('OKButton');await settled('Main');await targetExtent(first);
    const saved=await prefs();assert.equal(typeof saved,'string');assert.deepEqual(JSON.parse(saved).settings.resolution,first);
    const committed=await snapshot();assert.deepEqual(committed.applied.resolution,first);
    let saves=await witnesses();assert.equal(saves.length,2);saveWitness(saves[1],first);assert.equal(saves[1].after,saved);
    await capture('01-resize-committed');const oldAudio=await audio();
    await openOptions();const next=await chooseOther(first),dirty=(await snapshot()).draft;
    await page.evaluate(()=>{__pcxResizeGate.fault='storage';});await click('OKButton');
    const restored=await state(s=>s.screen==='Options(Video)'&&!s.transitionActive&&equalNotice(s.notice),'Save failure did not report restoration');
    assert(restored.notice.length<500 && !/wasm:\/\/|\n\s+at /.test(restored.notice),'Stack leaked into menu notice');
    await targetExtent(first);assert.deepEqual(restored.applied,committed.applied);assert.deepEqual(restored.draft,dirty);assert.notDeepEqual(restored.draft,restored.applied);
    assert.equal(await prefs(),saved);assert.deepEqual(await audio(),oldAudio);
    saves=await witnesses();assert.equal(saves.length,3);saveWitness(saves[2],next);assert.equal(saves[2].fault,'storage');assert.equal(saves[2].before,saved);
    await capture('02-storage-failure-restored');
    await page.evaluate(()=>{__pcxResizeGate.fault='audio';});await click('OKButton');
    const warned=await state(s=>s.screen==='Options(Video)'&&!s.transitionActive&&/Settings saved; render size confirmed, but/.test(s.notice)&&/Injected postcommit audio refusal/.test(s.notice),'Postcommit audio failure was not distinguished');
    assert(warned.notice.length<500 && !/wasm:\/\/|\n\s+at /.test(warned.notice),'Stack leaked into menu notice');
    await targetExtent(next);assert.deepEqual(warned.applied,dirty);assert.deepEqual(warned.draft,dirty);assert(!/restor|not saved/i.test(warned.notice));
    const after=await prefs();assert.deepEqual(JSON.parse(after).settings,dirty);assert.deepEqual(JSON.parse(after).audio,JSON.parse(saved).audio);assert.notEqual(after,saved);assert.deepEqual(await audio(),oldAudio);
    saves=await witnesses();assert.equal(saves.length,4);saveWitness(saves[3],next);assert.equal(saves[3].after,after);
    await capture('03-postcommit-audio-warning');
    await page.evaluate(()=>{const g=__pcxResizeGate;g.fault=null;peacecraft3Settings.save=g.save;peacecraft3Audio.setUserGains=g.gains;});
    await click('CancelButton');await settled('Main');await targetExtent(next);assert.equal(await prefs(),after);assert.deepEqual((await snapshot()).applied,dirty);
    await capture('04-cancel-after-commit');
    }
    assert.deepEqual(errors,[]);assert.deepEqual(sockets,[]);assert.deepEqual(blocked,[]);assert(requests.every(r=>allowedRequest(r.url,r.method,r.body,files)));assert(responses.every(r=>r.status<400));
    passed=true;
  } catch(error) {failure={message:error.message,stack:error.stack};if(page&&!page.isClosed())await page.screenshot({path:resolve(output,'failure.png'),fullPage:true}).catch(()=>{});throw error;}
  finally {
    const saves=page&&!page.isClosed()?await witnesses().catch(()=>[]):[];
    const finalState=page&&!page.isClosed()?await snapshot().catch(()=>lastState):lastState;
    const finalExtent=page&&!page.isClosed()?await extent().catch(()=>null):null;
    await save('proof.json',{passed,failure,finalState,finalExtent,packageReceipt:{path:env.PCX_OPTIONS_BUILD_RECEIPT,sha256:hash(receiptBytes)},fixture:{path:env.PCX_OPTIONS_PREPARED,sha256:hash(fixtureBytes)},packageChecks,requests,responses,blocked,errors,sockets,logs,steps,captures,saves,
      fastClick,diagnosticOnly:applyProbe,scope:fastClick?'Trusted browser fast pointer batches; current-update input/action witnesses required; no native parity claim'+(sliderRun?'; includes existing slider Apply/Cancel route':''):applyProbe?'Focused pointer/keyboard Apply diagnostic; not complete slider acceptance':sliderRun?'Source-derived slider pointer/keyboard input, local audio Apply/Cancel at centered/origin-zero UI viewports; no native parity claim':'Browser-reported canvas/client render extent and local preference transaction; screenshots require manual rendered review; no compositor/display-mode or native parity claim',reloadTested:false});
    await browser.close();
  }
  console.log(JSON.stringify({ok:passed,output,captures:captures.length,uploads:0}));
}
function equalNotice(notice) { return /Injected browser storage save refusal/.test(notice) && /Previous render size restored\./.test(notice); }
if (process.argv.includes('--self-check')) selfCheck(); else await run();
