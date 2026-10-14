import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { readFile } from 'node:fs/promises';
import { execFile } from 'node:child_process';

const moduleSource = await readFile(new URL('../../web/iso/installer.js', import.meta.url), 'utf8');
const { inspectInstaller, inspectInstallerHeader, InstallerError } = await import(`data:text/javascript;base64,${Buffer.from(moduleSource).toString('base64')}`);
const u32 = n => { const b = Buffer.alloc(4); b.writeUInt32LE(n >>> 0); return b; };
const join = (...parts) => Buffer.concat(parts.map(p => Buffer.isBuffer(p) ? p : Buffer.from(p)));
const string = s => join(u32(Buffer.byteLength(s,'latin1')), Buffer.from(s,'latin1'));
const variable = n => join([0],u32(0x60000000+n));
const literal = s => join([1],u32(0),string(s));
const integer = n => join([1],u32(1),u32(n));
const assign = (n,value) => join([0],variable(n),value);
const push = type => join([11],u32(type));
const reference = n => join([3],variable(n));
const globalVariable = n => join([0],u32(n));
const field = index => join([2],u32(0),u32(index));
const call = n => join([5],u32(n));
const pop = Buffer.from([4]);
const dll = 'dll:files:synthetic.dll\0ISArcExtract\0'+String.fromCharCode(3,1,0,1,...Array(10).fill(0));
const external = (name,decl) => join([3,Buffer.byteLength(name)],Buffer.from(name),string(decl));

function script({ dynamic = false, duplicateImport = false, bypass = false, language = false, missingGuard = false } = {}) {
  const prelude=[],branches=[];
  if(language) {
    prelude.push(push(2),push(3),join([14],variable(2),globalVariable(0)),push(0),push(0),push(0),assign(5,literal('{src}\\')),reference(4),call(1),pop,pop,assign(3,variable(4)),pop,join([1,0],variable(3),literal('fixture-*.arc')),reference(1),call(2),pop,pop,pop,pop);
    prelude.push(push(1),assign(1,field(1)),join([1,7],variable(1),integer(16)),push(2),join([12,5],variable(2),variable(1),integer(0)),join([17],variable(2),[1]),pop,pop);
    branches.push(prelude.length);prelude.push(Buffer.alloc(5));prelude.at(-1)[0]=18;
    prelude.push(push(1),push(0),assign(2,field(0)),push(0),reference(3),call(3),pop,reference(1),call(4),pop,pop,pop,push(2),join([12,2],variable(2),variable(1),integer(0)),join([17],variable(2),[1]),pop,pop);
    if(!missingGuard){branches.push(prelude.length);prelude.push(Buffer.alloc(5));prelude.at(-1)[0]=18;}
  }
  const ops = [push(2)], args = [integer(0),integer(0),null,literal('{app}\\'),literal(''),integer(0),literal('synthetic-key'),literal('{tmp}\\codec.ini'),literal('{tmp}\\'),integer(0)];
  let slot = 1, passwordEnd;
  for (let argument = 9; argument >= 0; argument--) {
    slot++; ops.push(push(argument === 2 || [3,4,6,7,8].includes(argument) ? 0 : 1));
    if (argument === 2) {
      ops.push(push(0),push(0),assign(slot+2,literal(language?'{src}\\':'{src}\\fixture.arc')),reference(slot+1),call(1),pop,pop);
      if(language)ops.push(join([1,0],variable(slot+1),field(0)));
      ops.push(assign(slot,variable(slot+1)),pop);
    } else {
      ops.push(assign(slot,args[argument]));
      if (argument === 6) {
        passwordEnd = ops.reduce((n,p)=>n+p.length,0);
        if (dynamic) ops.push(join([1,0],variable(slot),literal('-changed')));
      }
    }
  }
  ops.push(reference(1),call(0),[9]);
  let code = join(...prelude,...ops);
  for(const branch of branches){const at=prelude.slice(0,branch).reduce((n,p)=>n+p.length,0);code.writeUInt32LE(code.length-1-at-5,at+1);}
  if (bypass) code = join([6],u32(passwordEnd),code);
  const imports = [external('',dll),external('EXPANDCONSTANT',String.fromCharCode(1,0))];
  if(language) imports.push(external('FINDFIRST',String.fromCharCode(1,0,1)),external('ACTIVELANGUAGE',String.fromCharCode(1)),external('POS',String.fromCharCode(1,0,0)));
  if (duplicateImport) imports.push(external('',dll));
  const header = join(u32(0x53504649),u32(23),u32(language?5:3),u32(imports.length+(language?2:1)),u32(language?1:0),u32(imports.length),u32(0));
  const types = join([10],u32(0),[5],u32(0),[1],u32(0),...(language?[[13],u32(0),[11],u32(2),u32(0),u32(1),u32(0)]:[]));
  const prefix = join(header,types,...imports), globals=language?join(u32(4),[0]):Buffer.alloc(0);
  if(language){
    const base=script(),basePrefix=28+15+external('',dll).length+external('EXPANDCONSTANT',String.fromCharCode(1,0)).length;
    const baseCode=base.subarray(base.readUInt32LE(basePrefix+1));
    const start=prefix.length+18+globals.length;
    return join(prefix,[0],u32(start),u32(code.length),[0],u32(start+code.length),u32(baseCode.length),globals,code,baseCode);
  }
  return join(prefix,[0],u32(prefix.length+9+globals.length),u32(code.length),globals,code);
}
const setupHeader = s => join(string('synthetic metadata'),u32(s.length),s);
function crc(bytes) {
  let n=0xffffffff;
  for(const b of bytes){n^=b;for(let i=0;i<8;i++)n=(n>>>1)^((n&1)?0xedb88320:0);}
  return (n^0xffffffff)>>>0;
}
function executable(header, { compressed = false, dictionary = 4096 } = {}) {
  const payload = compressed ? join([93],u32(dictionary),header) : header;
  const chunks=[];
  for(let at=0;at<payload.length;at+=4096){const part=payload.subarray(at,at+4096);chunks.push(u32(crc(part)),part);}
  const stored=join(...chunks), blockInfo=join(u32(stored.length),[Number(compressed)]), version=Buffer.alloc(64);
  version.write('Inno Setup Setup Data (5.5.0) (u)');
  const block=join(version,u32(crc(blockInfo)),blockInfo,stored), exe=Buffer.alloc(2048+block.length), v=new DataView(exe.buffer,exe.byteOffset,exe.length);
  exe.write('MZ');v.setUint32(0x3c,0x80,true);exe.write('PE\0\0',0x80);v.setUint16(0x84,0x14c,true);v.setUint16(0x86,1,true);v.setUint16(0x94,224,true);
  v.setUint16(0x98,0x10b,true);v.setUint32(0x98+112,0x1000,true);v.setUint32(0x98+116,256,true);
  const section=0x98+224;v.setUint32(section+12,0x1000,true);v.setUint32(section+16,256,true);v.setUint32(section+20,1024,true);
  const root=1024;
  for(const [at,id,next] of [[0,10,24],[24,11111,48]]){v.setUint16(root+at+14,1,true);v.setUint32(root+at+16,id,true);v.setUint32(root+at+20,(0x80000000|next)>>>0,true);}
  v.setUint16(root+48+14,1,true);v.setUint32(root+64,0,true);v.setUint32(root+68,72,true);v.setUint32(root+72,0x1000+96,true);v.setUint32(root+76,44,true);
  const table=root+96;exe.set([114,68,108,80,116,83,205,230,215,123,11,42],table);v.setUint32(table+12,1,true);v.setUint32(table+32,2048,true);v.setUint32(table+40,crc(exe.subarray(table,table+40)),true);
  exe.set(block,2048);return exe;
}

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
