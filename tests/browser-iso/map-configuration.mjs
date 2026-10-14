// Synthetic local-file controller boundaries; optional actual Cargo WASM gate.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { pathToFileURL } from 'node:url';
const webBase = process.env.PCX_UI_WEB_DIR ? pathToFileURL(process.env.PCX_UI_WEB_DIR + '/') : new URL('../../web/',import.meta.url);
const { installMapConfiguration } = await import(new URL('map-configuration.js',webBase));

class Element extends EventTarget {
  value = ''; textContent = ''; disabled = true; dataset = {}; hidden = true; files = [];
  removeAttribute(name) { delete this[name]; }
}
function dom() {
  const ids = ['map-configuration','map-config-file','map-config-limit','map-config-submit','map-config-ordinal','map-config-inspect','map-config-revision','map-config-archive','map-config-progress','map-config-status'];
  const elements = Object.fromEntries(ids.map(id => [id,new Element()]));
  return { getElementById: id => elements[id], elements };
}
function frames() {
  let counter = 0;
  const callbacks = new Map();
  return {
    request: callback => { callbacks.set(++counter,callback); return counter; },
    cancel: id => callbacks.delete(id),
    step() { const entry = callbacks.entries().next().value;assert.ok(entry,'expected a scheduled poll');callbacks.delete(entry[0]);entry[1](); },
    count: () => callbacks.size,
  };
}
function handle(fields) {
  let frees = 0;
  const h = Object.fromEntries(Object.entries(fields).map(([key,value]) => [key,() => {assert.equal(frees,0,`${key} read after free`);return value;}]));
  return Object.assign(h,{ free() { assert.equal(frees++,0,'double free'); }, frees: () => frees });
}
function fake() {
  const made = [], submissions = [], polls = []; let revision = 12, next = 1, completed = false;
  const make = fields => { const h=handle(fields);made.push(h);return h; };
  return { made, submissions, polls, revision: value => {revision=value;}, complete: () => {completed=true;},
    api: {
      asset_revision: () => revision,
      inspect_map_archive: (index,rev) => make(index===0 ? {status:'selected',index,committed_revision:rev,archive_name:'<script>long archive العربية.mpq</script>',archive_bytes:'123'} : {status:'failed',kind:'source-unavailable',diagnostic:'archive does not exist'}),
      submit_map_configuration: (text,limit) => {submissions.push({text,limit});return make({status:'submitted',id:next++});},
      poll_map_configuration_result: id => {polls.push(id);return make(completed ? {status:'completed',kind:'map-admitted',diagnostic:'parsed'} : {status:'pending'});},
    },
  };
}
function setup(api) { const root=dom(), schedule=frames();const controller=installMapConfiguration({api,root,requestFrame:schedule.request,cancelFrame:schedule.cancel});return {root,schedule,controller,e:root.elements}; }
function input(element,value) { element.value=value;element.dispatchEvent(new Event('input')); }
function file(e,bytes) { let reads=0;e['map-config-file'].files=[{size:bytes.length,arrayBuffer:async()=>{reads++;return Uint8Array.from(bytes).buffer;}}];e['map-config-file'].dispatchEvent(new Event('input'));return ()=>reads; }
function submit(e) { e['map-configuration'].dispatchEvent(new Event('submit',{cancelable:true})); }
const flush=()=>new Promise(resolve=>setImmediate(resolve));

test('explicit pre-read byte policy refuses invalid, oversized and mismatched reads',async()=>{
 const f=fake(),{e,controller}=setup(f.api);const reads=file(e,[123,125]);
 for(const bad of ['', ' 2','+2','02','-1','1.5','1e2','4294967296']){input(e['map-config-limit'],bad);submit(e);await flush();assert.equal(reads(),0,bad);assert.equal(f.submissions.length,0);}
 input(e['map-config-limit'],'1');submit(e);await flush();assert.equal(reads(),0);assert.match(e['map-config-status'].textContent,/2 bytes; your limit is 1/);
 input(e['map-config-limit'],'2');e['map-config-file'].files=[{size:2,arrayBuffer:async()=>Uint8Array.of(123).buffer}];submit(e);await flush();assert.equal(f.submissions.length,0);assert.match(e['map-config-status'].textContent,/read length differs/);controller.dispose();
});

test('fatal UTF-8 and exact byte ceiling; no replacement-decoded identity reaches Rust',async()=>{
 const f=fake(),{e,controller}=setup(f.api);input(e['map-config-limit'],'2');const invalidReads=file(e,[0xc3,0x28]);submit(e);await flush();assert.equal(invalidReads(),1);assert.equal(f.submissions.length,0);assert.equal(e['map-config-status'].dataset.state,'error');
 const validReads=file(e,new TextEncoder().encode('é'));submit(e);await flush();assert.equal(validReads(),1);assert.deepEqual(f.submissions,[{text:'é',limit:2}]);controller.dispose();
});

test('single producer, Pending handles freed, completion acknowledged once and retained until next result/dispose',async()=>{
 const f=fake(),{e,root,schedule,controller}=setup(f.api);assert.equal(installMapConfiguration({api:f.api,root,requestFrame:schedule.request,cancelFrame:schedule.cancel}),controller);
 input(e['map-config-limit'],'2');file(e,[123,125]);submit(e);submit(e);await flush();assert.equal(f.submissions.length,1);assert.equal(schedule.count(),1);assert.equal(f.made[0].frees(),1);
 schedule.step();assert.equal(schedule.count(),1);assert.equal(f.made[1].frees(),1);assert.equal(e['map-config-submit'].disabled,true);f.complete();schedule.step();assert.equal(schedule.count(),0);assert.equal(f.polls.length,2);assert.equal(f.made[2].frees(),0);assert.match(e['map-config-status'].textContent,/map data parsed.*does not run gameplay/);
 submit(e);await flush();assert.equal(f.submissions.length,2);assert.equal(f.made[2].frees(),1);controller.dispose();assert.equal(schedule.count(),0);assert.equal(e['map-config-submit'].disabled,true);
});

test('archive inspection uses real supplied ordinal/current revision; missing remains distinct and handles free',()=>{
 const f=fake(),{e,controller}=setup(f.api);input(e['map-config-ordinal'],'0');e['map-config-inspect'].dispatchEvent(new Event('click'));assert.match(e['map-config-archive'].textContent,/Archive 0 at revision 12/);assert.match(e['map-config-archive'].textContent,/<script>/);assert.equal(f.made[0].frees(),1);f.revision(13);controller.refreshArchives();assert.match(e['map-config-revision'].textContent,/13/);input(e['map-config-ordinal'],'1');e['map-config-inspect'].dispatchEvent(new Event('click'));assert.match(e['map-config-archive'].textContent,/source-unavailable/);assert.equal(f.made[1].frees(),1);controller.dispose();
});

test('in-flight metadata reads cannot submit through an archive import; failed read can be retried',async()=>{
 const f=fake(),{e,controller}=setup(f.api);let release;e['map-config-file'].files=[{size:2,arrayBuffer:()=>new Promise(resolve=>{release=resolve;})}];input(e['map-config-limit'],'2');submit(e);controller.setImportBusy(true);release(Uint8Array.of(123,125).buffer);await flush();assert.equal(f.submissions.length,0);assert.match(e['map-config-status'].textContent,/Archive import began/);assert.equal(e['map-config-submit'].disabled,true);controller.setImportBusy(false);assert.equal(e['map-config-submit'].disabled,false);controller.dispose();
});

test('typed submission failure and completed refusal remain distinct; polling throw retains ownership and retries once per frame',async()=>{
 const f=fake();let calls=0;f.api.submit_map_configuration=()=>handle({status:'failed',kind:'metadata-schema',diagnostic:'missing caller field'});const {e,controller,schedule}=setup(f.api);input(e['map-config-limit'],'2');file(e,[123,125]);submit(e);await flush();assert.match(e['map-config-status'].textContent,/metadata-schema: missing caller field/);assert.equal(schedule.count(),0);
 f.api.submit_map_configuration=()=>handle({status:'submitted',id:9});f.api.poll_map_configuration_result=()=>{if(calls++===0)throw Error('temporary JS boundary failure');return handle({status:'completed',kind:'map-binding',diagnostic:'selected source is stale'});};submit(e);await flush();schedule.step();assert.equal(schedule.count(),1);assert.equal(e['map-config-submit'].disabled,true);assert.match(e['map-config-status'].textContent,/awaiting a successful poll/);schedule.step();assert.equal(schedule.count(),0);assert.match(e['map-config-status'].textContent,/Request 9.*map-binding: selected source is stale/);controller.dispose();
});

if(process.env.PCX_REAL_WASM_JS){
 test('actual Cargo application WASM: DOM producer -> decoder -> globals -> App admission and stale rollback',async()=>{
  const api=createRequire(import.meta.url)(process.env.PCX_REAL_WASM_JS);const text=api.ffi_test_metadata();const {e,schedule,controller}=setup(api);const bytes=new TextEncoder().encode(text);input(e['map-config-limit'],String(bytes.length));file(e,bytes);input(e['map-config-ordinal'],'0');e['map-config-inspect'].dispatchEvent(new Event('click'));assert.match(e['map-config-archive'].textContent,/Archive 0 at revision/);
  const occupied=api.submit_map_configuration(text,bytes.length);const occupiedId=occupied.id();occupied.free();submit(e);await flush();assert.match(e['map-config-status'].textContent,/queue-busy/);assert.equal(schedule.count(),0);api.ffi_test_frame();const original=api.poll_map_configuration_result(occupiedId);assert.equal(original.kind(),'map-admitted');original.free();
  submit(e);await flush();schedule.step();assert.equal(e['map-config-submit'].disabled,true);api.ffi_test_frame();schedule.step();assert.equal(e['map-config-status'].dataset.state,'complete');assert.equal(api.ffi_test_active_revision(),'73');
  const value=JSON.parse(text);value.binding_revision='18446744073709551615';const second=new TextEncoder().encode(JSON.stringify(value));input(e['map-config-limit'],String(second.length));file(e,second);submit(e);await flush();api.ffi_test_stale_before_frame();schedule.step();assert.equal(e['map-config-status'].dataset.state,'error');assert.match(e['map-config-status'].textContent,/map-binding.*unavailable/);assert.equal(api.ffi_test_active_revision(),'73');controller.dispose();
 });
}
