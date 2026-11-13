import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { readFile } from 'node:fs/promises';
import { execFile } from 'node:child_process';

const moduleSource = await readFile(new URL('../../web/iso/installer.js', import.meta.url), 'utf8');
const { inspectInstaller, inspectInstallerHeader, InstallerError } = await import(`data:text/javascript;base64,${Buffer.from(moduleSource).toString('base64')}`);
import { u32, join, script, setupHeader, executable } from './installer-fixtures.mjs';

test('strict framed script derives source/password from reverse arguments and redacts JSON', () => {
  const result=inspectInstallerHeader(setupHeader(script()));
  assert.equal(result.bindings.length,1);
  assert.equal(result.bindings[0].source.path,'fixture.arc');
  assert.equal(Buffer.from(result.bindings[0].password).toString(),'synthetic-key');
  assert.equal(result.bindings[0].evidence.passwordArgument,7);
  assert.doesNotMatch(JSON.stringify(result),/synthetic-key/);
});
test('language binding requires authored search, non-directory and ActiveLanguage guards', () => {
  // Independent synthetic procedures share the same supported imports.
  const languageCode=script({language:true});
  const result=inspectInstallerHeader(setupHeader(languageCode));
  const binding=result.bindings.find(b=>b.source.kind==='language');
  assert.equal(binding.source.pattern,'fixture-*.arc');
  assert.equal(binding.source.excludeDirectories,true);
  assert.equal(binding.source.match,'case-sensitive-substring');
  assert.equal(Buffer.from(binding.password).toString(),'synthetic-key');
  assert.throws(()=>inspectInstallerHeader(setupHeader(script({language:true,missingGuard:true}))),e=>e.code==='UNSUPPORTED_SELECTION');
});
test('ambiguous containers/imports, transformed passwords, bypassed definitions and budgets refuse', () => {
  const code=script(), twice=join(u32(code.length),code,u32(code.length),code);
  for(const [header,expected] of [[twice,'AMBIGUOUS_SCRIPT'],[setupHeader(script({duplicateImport:true})),'AMBIGUOUS_IMPORT'],[setupHeader(script({dynamic:true})),'UNSUPPORTED_PASSWORD'],[setupHeader(script({bypass:true})),'UNSUPPORTED_CONTROL_FLOW']])
    assert.throws(()=>inspectInstallerHeader(header),e=>e instanceof InstallerError&&e.code===expected);
  assert.throws(()=>inspectInstallerHeader(setupHeader(code),{limits:{maxProcedures:1}}),e=>e.code==='BUDGET');
  assert.throws(()=>inspectInstallerHeader(setupHeader(code.subarray(0,-1))),e=>e.code==='MALFORMED');
  assert.throws(()=>inspectInstallerHeader(setupHeader(code),{limits:{maxScriptBytes:1}}),e=>e.code==='BUDGET');
  assert.throws(()=>inspectInstallerHeader(setupHeader(code),{limits:{maxAnalysisWork:1}}),e=>e.code==='BUDGET');
  assert.throws(()=>inspectInstallerHeader(setupHeader(script({language:true})),{limits:{maxBindings:1}}),e=>e.code==='BUDGET');
});
test('PE loader/chunk CRC validation and bounded LZMA callback preflight', async () => {
  const header=setupHeader(script()), exe=executable(header), blob=new Blob([exe]), entry={path:'synthetic.exe',offset:0,size:exe.length};
  assert.equal((await inspectInstaller(blob,entry)).bindings.length,1);
  const bad=Buffer.from(exe);bad[bad.length-1]^=1;
  await assert.rejects(inspectInstaller(new Blob([bad]),entry),e=>e.code==='CRC');
  const loaderBad=Buffer.from(exe);loaderBad[1024+96+20]^=1;
  await assert.rejects(inspectInstaller(new Blob([loaderBad]),entry),e=>e.code==='CRC');
  let storedDecoderCalls=0;
  await assert.rejects(inspectInstaller(blob,entry,{limits:{maxHeaderBytes:1},decodeLzma:()=>{storedDecoderCalls++;}}),e=>e.code==='BUDGET');
  assert.equal(storedDecoderCalls,0);
  const packed=executable(header,{compressed:true});let called=0;
  const decodeLzma=async(bytes,options)=>{called++;assert.equal(options.dictSize,4096);assert.equal(options.maxOutputBytes,8*1024*1024);return bytes;};
  assert.equal((await inspectInstaller(new Blob([packed]),{...entry,size:packed.length},{decodeLzma})).bindings.length,1);assert.equal(called,1);
  called=0;await assert.rejects(inspectInstaller(new Blob([packed]),{...entry,size:packed.length},{decodeLzma,limits:{maxDictionaryBytes:1}}),e=>e.code==='BUDGET');assert.equal(called,0);
  await assert.rejects(inspectInstaller(blob,{...entry,size:exe.length+1}),e=>e.code==='SOURCE');
  const controller=new AbortController();controller.abort();await assert.rejects(inspectInstaller(blob,entry,{signal:controller.signal}),e=>e.name==='AbortError');
});

const originalIso=process.env.PEACECRAFT3_INSTALLER_ISO;
test('locally supplied original installer: bounded Python LZMA callback and redacted binding proof', {skip:!originalIso}, async () => {
  const isoSource=await readFile(new URL('../../web/iso.js',import.meta.url),'utf8');
  const {discoverIso}=await import(`data:text/javascript;base64,${Buffer.from(isoSource).toString('base64')}`);
  const blob=await fs.openAsBlob(originalIso);const discovery=await discoverIso(blob);
  const candidates=[];
  for(const entry of discovery.files){const b=new Uint8Array(await blob.slice(entry.offset,entry.offset+2).arrayBuffer());if(b[0]===77&&b[1]===90)candidates.push(entry);}
  assert.equal(candidates.length,1,'requires exactly one local PE installer');
  const decodeLzma=(bytes,{properties,dictSize,maxOutputBytes})=>new Promise((resolve,reject)=>{
    const py='import sys,lzma\np,d,m=map(int,sys.argv[1:])\nf={"id":lzma.FILTER_LZMA1,"dict_size":d,"lc":p%9,"lp":(p%45)//9,"pb":p//45}\nz=lzma.LZMADecompressor(lzma.FORMAT_RAW,filters=[f])\no=z.decompress(sys.stdin.buffer.read(),max_length=m+1)\nassert len(o)<=m and z.eof and not z.unused_data\nsys.stdout.buffer.write(o)';
    const child=execFile('python3',['-I','-c',py,String(properties),String(dictSize),String(maxOutputBytes)],{encoding:'buffer',maxBuffer:maxOutputBytes+1},(error,out)=>error?reject(new Error('local decoder refused stream')):resolve(new Uint8Array(out)));
    child.stdin.end(bytes);
  });
  const result=await inspectInstaller(blob,candidates[0],{decodeLzma});
  assert.equal(result.bindings.filter(b=>b.source.kind==='literal').length,1);
  assert.equal(result.bindings.filter(b=>b.source.kind==='language').length,1);
  for(const binding of result.bindings)assert.ok(binding.password.length>0);
  assert.ok(result.diagnostic.bindings.every(b=>b.password==='[redacted]'));
});
