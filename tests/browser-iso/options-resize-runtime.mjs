// Real local browser gate; --self-check performs no browser, network, or ISO reads.
import assert from 'node:assert/strict';
import {readFile, writeFile, mkdir, realpath} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {resolve, relative, sep, dirname} from 'node:path';
import {pathToFileURL} from 'node:url';

const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const equal = (a, b) => JSON.stringify(a) === JSON.stringify(b);
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
  console.log(JSON.stringify({ok:true,selfCheck:true,browserLaunched:false,networkRequests:0,originalReads:0}));
}

async function run() {
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
  const browser = await chromium.launch({executablePath:env.PCX_CHROMIUM ?? '/usr/bin/chromium',args:['--no-sandbox','--enable-unsafe-swiftshader']});
  const requests=[],responses=[],blocked=[],errors=[],sockets=[],logs=[],steps=[],captures=[];
  let page, failure, passed=false;
  async function poll(check, why) {
    const end = Date.now()+120000;
    while (Date.now()<end) {
      assert.deepEqual(errors,[]); assert.deepEqual(sockets,[]);
      assert(!logs.some(m => /Caught rendering error|Quitting application due to|Attachments have differing sizes/.test(m.text)), 'Renderer stopped');
      if (await check()) return; await new Promise(r => setTimeout(r,100));
    }
    throw Error(why);
  }
  const snapshot = () => page.evaluate(() => { const text=peacecraft3.menu_state(); return text ? JSON.parse(text) : null; });
  async function state(predicate, why) { let s; await poll(async()=>{s=await snapshot(); return s && predicate(s);},why); return s; }
  const settled = screen => state(s=>s.menuVisible && !s.transitionActive && s.screen===screen,'Did not settle: '+screen);
  const extent = () => page.locator('#launcher-canvas').evaluate(c=>({bitmap:[c.width,c.height],clientPhysical:[Math.round(c.clientWidth*devicePixelRatio),Math.round(c.clientHeight*devicePixelRatio)]}));
  const prefs = () => page.evaluate(key=>localStorage.getItem(key),preferenceKey);
  const audio = () => page.evaluate(()=>{const a=peacecraft3Audio.status(); return Object.fromEntries(['musicBaseVolume','clickBaseVolume','userMusicGain','userEffectsGain','userMusicEnabled','userEffectsEnabled','musicVolume','clickVolume'].map(k=>[k,a[k]]));});
  async function targetExtent(target) { await poll(async()=>{const e=await extent();return equal(e.bitmap,target)&&equal(e.clientPhysical,target);},'Canvas did not reach '+target); }
  async function click(name) {
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
    await page.mouse.move(x,y); await page.mouse.down(); await page.waitForTimeout(80); await page.mouse.up();
    steps.push({action:'pointer',name,rect:r});
  }
  async function openOptions() { await click('OptionsButton'); const s=await state(s=>s.menuVisible&&!s.transitionActive&&s.screen.startsWith('Options('),'Options did not open'); if(s.screen!=='Options(Video)')await click('VideoButton'); await settled('Options(Video)'); }
  async function chooseOther(old) { await click('ResolutionMenu');const s=await state(s=>s.controls.some(c=>c.name.startsWith('ResolutionChoice:')),'No resolution choices');const choice=distinctChoice(s.controls,old);await click(choice.name);await state(s=>equal(s.draft.resolution,choice.size),'Draft not selected');return choice.size; }
  async function capture(name) { await page.evaluate(async()=>{await new Promise(requestAnimationFrame);await new Promise(requestAnimationFrame);});const path=resolve(output,name+'.png');await page.screenshot({path,fullPage:true});captures.push({name,path,sha256:hash(await readFile(path)),state:await snapshot(),extent:await extent()}); }
  const witnesses = () => page.evaluate(()=>globalThis.__pcxResizeGate.saves);
  try {
    // Synthetic browser host geometry, independent of authored game layout.
    const context=await browser.newContext({viewport:{width:1500,height:1000},screen:{width:1280,height:720},deviceScaleFactor:1,serviceWorkers:'block'});
    await context.route('**/*',route=>{const r=route.request();if(allowedRequest(r.url(),r.method(),r.postDataBuffer()!==null,files))return route.continue();blocked.push({url:r.url(),method:r.method()});return route.abort('blockedbyclient');});
    context.on('request',r=>requests.push({url:r.url(),method:r.method(),body:r.postDataBuffer()!==null}));
    context.on('response',r=>responses.push({url:r.url(),status:r.status()}));
    page=await context.newPage(); page.setDefaultTimeout(120000);
    page.on('pageerror',e=>errors.push(e.message));
    page.on('websocket',s=>sockets.push(s.url())); page.on('console',m=>logs.push({type:m.type(),text:m.text()}));
    await page.goto(new URL('index.html',packageBase).href,{waitUntil:'domcontentloaded'});
    await page.waitForFunction(()=>globalThis.peacecraft3?.menu_state && globalThis.peacecraft3Settings && globalThis.peacecraft3Audio);
    assert.equal(await page.evaluate(()=>document.documentElement.dataset.localAssets),'true');
    assert.deepEqual(await page.evaluate(()=>peacecraft3.stored_assets()),[]);
    const revision=await page.evaluate(()=>peacecraft3.asset_revision());await page.locator('#iso').setInputFiles(fixture.isoPath);
    await page.waitForFunction(()=>document.querySelectorAll('#iso-candidates input').length===1&&!document.querySelector('#iso').disabled);
    await page.locator('#iso-candidates input').check();await page.locator('#iso-import').click();
    await page.waitForFunction(r=>peacecraft3.asset_revision()===r+1&&!document.querySelector('#iso').disabled,revision);
    assert((await page.evaluate(()=>peacecraft3.local_menu_editions())).includes('tft'));
    await page.locator('#demo-edition').selectOption('tft');await page.locator('#demo-start').click();await settled('Main');
    await page.evaluate(key=>{
      const gate={save:peacecraft3Settings.save,gains:peacecraft3Audio.setUserGains,fault:null,saves:[]};globalThis.__pcxResizeGate=gate;
      peacecraft3Settings.save=function(text,limit){
        const c=document.querySelector('#launcher-canvas'),w={target:JSON.parse(text).settings.resolution,bitmap:[c.width,c.height],clientCss:[c.clientWidth,c.clientHeight],dpr:devicePixelRatio,clientPhysical:[Math.round(c.clientWidth*devicePixelRatio),Math.round(c.clientHeight*devicePixelRatio)],fault:gate.fault,before:localStorage.getItem(key)};
        gate.saves.push(w);if(gate.fault==='storage')throw Error('Injected browser storage save refusal');
        const result=gate.save.call(this,text,limit);w.after=localStorage.getItem(key);return result;
      };
      peacecraft3Audio.setUserGains=function(...args){if(gate.fault==='audio')throw Error('Injected postcommit audio refusal');return gate.gains.apply(this,args);};
    },preferenceKey);
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
    assert.deepEqual(errors,[]);assert.deepEqual(sockets,[]);assert.deepEqual(blocked,[]);assert(requests.every(r=>allowedRequest(r.url,r.method,r.body,files)));assert(responses.every(r=>r.status<400));
    passed=true;
  } catch(error) {failure={message:error.message,stack:error.stack};if(page&&!page.isClosed())await page.screenshot({path:resolve(output,'failure.png'),fullPage:true}).catch(()=>{});throw error;}
  finally {
    const saves=page&&!page.isClosed()?await witnesses().catch(()=>[]):[];
    const finalState=page&&!page.isClosed()?await snapshot().catch(()=>null):null;
    const finalExtent=page&&!page.isClosed()?await extent().catch(()=>null):null;
    await save('proof.json',{passed,failure,finalState,finalExtent,packageReceipt:{path:env.PCX_OPTIONS_BUILD_RECEIPT,sha256:hash(receiptBytes)},fixture:{path:env.PCX_OPTIONS_PREPARED,sha256:hash(fixtureBytes)},packageChecks,requests,responses,blocked,errors,sockets,logs,steps,captures,saves,
      scope:'Browser-reported canvas/client render extent and local preference transaction; screenshots require manual rendered review; no compositor/display-mode or native parity claim',reloadTested:false});
    await browser.close();
  }
  console.log(JSON.stringify({ok:passed,output,captures:captures.length,uploads:0}));
}
function equalNotice(notice) { return /Injected browser storage save refusal/.test(notice) && /Previous render size restored\./.test(notice); }
if (process.argv.includes('--self-check')) selfCheck(); else await run();
