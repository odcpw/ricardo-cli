import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
const source = await readFile(new URL('../extension/background.js', import.meta.url), 'utf8');
const { validatePath, handleRequest } = await import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`);
test('route validation rejects arbitrary origins and endpoint bypasses', () => {
  for (const path of ['https://evil.test/de/', '//evil.test/de/', '/\\evil.test/de/', '/api/bids', '/de/s/../my-ricardo/private', '/de/s/%2e%2e/', '/de/s/%2Fetc', '/de/#fragment']) {
    assert.throws(() => validatePath(path), path);
  }
  for (const path of ['/de/', '/de/s/bambu%20p2s/', '/de/a/printer-12345/', '/de/a/12345/', '/de/my-ricardo/saved/articles/', '/api/frontend/notifications?locale=de']) {
    assert.equal(validatePath(path).origin, 'https://www.ricardo.ch');
  }
});
test('only fetch operations accepted and a Ricardo tab is required', async () => {
  await assert.rejects(handleRequest({ op: 'eval', code: 'anything' }), /only fetch/);
  await assert.rejects(handleRequest({ op: 'fetch', path: '/de/' }, {tabs:{query:async()=>[]}}), /Open Ricardo/);
});
test('fetch targets existing tab and returns HTTP failures without turning them into success', async () => {
  let called;
  const chrome = { tabs: {query:async()=>[{id:12, active:true}]}, scripting:{ executeScript:async args=>{called=args;return [{result:{status:403,body:'blocked'}}]}}};
  const result = await handleRequest({op:'fetch',path:'/de/'}, chrome);
  assert.equal(result.status,403);
  assert.equal(called.world,'MAIN');
  assert.equal(called.target.tabId,12);
  assert.deepEqual(called.args,['https://www.ricardo.ch/de/','GET',null]);
});
test('numeric URL resolution uses same-origin fetch and validates status, origin and exact ID', async () => {
  let script;
  const chrome = {tabs:{query:async()=>[{id:44,active:true}]},scripting:{executeScript:async args=>{script=args;return [{result:{url:'https://www.ricardo.ch/de/a/printer-12345/'}}]}}};
  await handleRequest({op:'resolve',article:'12345'},chrome);
  assert.equal(script.world,'MAIN');
  assert.equal(script.target.tabId,44);
  assert.deepEqual(script.args,['12345']);
  await assert.rejects(handleRequest({op:'resolve',article:'//evil.test'},chrome),/numeric/);
  const savedFetch=globalThis.fetch; const savedLocation=globalThis.location;
  const savedTimeout=AbortSignal.timeout;
  let destination='https://www.ricardo.ch/de/a/printer-12345/';
  let status=200; let cancelled=0; let requests=0;
  try {
    AbortSignal.timeout=ms=>{assert.equal(ms,10000);return savedTimeout(ms)};
    globalThis.location={origin:'https://www.ricardo.ch'};
    globalThis.fetch=async (url,options)=>{
      requests++;
      assert.equal(url,'https://www.ricardo.ch/de/a/12345/');
      assert.equal(options.mode,'same-origin');
      assert.equal(options.redirect,'follow');
      assert.equal(options.credentials,'include');
      assert.equal(options.cache,'no-store');
      assert.equal(options.method,'GET');
      assert.ok(options.signal instanceof AbortSignal);
      return {url:destination,status,body:{cancel:async()=>{cancelled++}}};
    };
    assert.deepEqual(await script.func(...script.args),{url:destination});
    destination='https://evil.test/de/a/printer-12345/';
    await assert.rejects(script.func(...script.args),/outside Ricardo/);
    destination='https://www.ricardo.ch/de/a/printer-99999/';
    await assert.rejects(script.func(...script.args),/canonical listing/);
    destination='https://www.ricardo.ch/de/a/printer-12345/'; status=403;
    await assert.rejects(script.func(...script.args),/HTTP 403/);
    assert.equal(cancelled,4);
    globalThis.fetch=async()=>{requests++;throw new DOMException('Timed out','TimeoutError')};
    await assert.rejects(script.func(...script.args),{name:'TimeoutError'});
    assert.equal(requests,5);
  } finally {globalThis.fetch=savedFetch;globalThis.location=savedLocation;AbortSignal.timeout=savedTimeout;}
});
test('favorite has fixed routes, methods and bodies and rejects caller overrides', async () => {
  let called;
  const chrome = {tabs:{query:async()=>[{id:12}]},scripting:{executeScript:async args=>{called=args;return [{result:{status:200,body:''}}]}}};
  await handleRequest({op:'favorite',action:'add',article:'12345'},chrome);
  assert.deepEqual(called.args,['https://www.ricardo.ch/marketplace-spa/api/saved-articles','POST','{"articleId":"12345"}']);
  await handleRequest({op:'favorite',action:'remove',article:'12345'},chrome);
  assert.deepEqual(called.args,['https://www.ricardo.ch/marketplace-spa/api/saved-articles/12345','DELETE',null]);
  for (const request of [
    {op:'favorite',action:'add',article:'../123'},
    {op:'favorite',action:'bid',article:'123'},
    {op:'favorite',action:'add',article:'123',method:'PUT'},
    {op:'favorite',action:'remove',article:'123',path:'/api/bids'}
  ]) await assert.rejects(handleRequest(request,chrome));
});
test('favorite keeps credentials in page fetch and makes no retry on failure', async () => {
  let script;
  const chrome = {tabs:{query:async()=>[{id:12}]},scripting:{executeScript:async args=>{script=args;return [{result:{status:200,body:''}}]}}};
  await handleRequest({op:'favorite',action:'add',article:'12345'},chrome);
  const savedFetch=globalThis.fetch; const savedLocation=globalThis.location;
  let calls=0;
  try {
    globalThis.location={origin:'https://www.ricardo.ch'};
    globalThis.fetch=async (url,options)=>{
      calls++;
      assert.equal(options.credentials,'include');
      assert.equal(options.redirect,'error');
      assert.equal(options.method,'POST');
      assert.deepEqual(options.headers,{'Content-Type':'application/json'});
      assert.equal(options.body,'{"articleId":"12345"}');
      throw new Error('network interrupted');
    };
    await assert.rejects(script.func(...script.args),/network interrupted/);
    assert.equal(calls,1);
  } finally {globalThis.fetch=savedFetch;globalThis.location=savedLocation;}
});

const {validateTuttiPath,validateImageUrl}=await import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`);
test('Tutti paths and image CDNs are exact allowlists',()=>{
  assert.equal(validateTuttiPath('/de/vi/zuerich/garden/tools/printer/123').hostname,'www.tutti.ch');
  assert.equal(validateTuttiPath('/de/q/suche/Ak6pQcnVzYSBDb3JlwJTAwMDA').hostname,'www.tutti.ch');
  for (const path of ['/de/messages','//evil.test/de/vi/a/b/123','/de/vi/a/%2e%2e/b/123','/de/vi/a/b/123?x=1','/de/q/suche/abc']) assert.throws(()=>validateTuttiPath(path));
  for (const url of ['https://evil.test/a.jpg','https://c.tutti.ch/big/123.svg','https://c.tutti.ch/big/123.jpg?x=1','https://me@c.tutti.ch/big/123.jpg']) assert.throws(()=>validateImageUrl(url));
  assert.equal(validateImageUrl('https://c.tutti.ch/big/123.jpg').hostname,'c.tutti.ch');
});
test('Tutti uses its own existing tab and never accepts a caller method',async()=>{
  let script;
  const chrome={tabs:{query:async arg=>{assert.deepEqual(arg,{url:'https://www.tutti.ch/*'});return [{id:7}]}},scripting:{executeScript:async arg=>{script=arg;return [{result:{status:200}}]}}};
  await handleRequest({op:'fetch_tutti',path:'/de/vi/a/b/printer/123'},chrome);
  assert.equal(script.target.tabId,7);assert.deepEqual(script.args,['https://www.tutti.ch/de/vi/a/b/printer/123','GET',null]);
  await assert.rejects(handleRequest({op:'fetch_tutti',path:'/de/vi/a/b/123',method:'POST'},chrome),/unsupported fields/);
});
test('image fetch omits credentials, rejects redirects and stops on HTTP 429',async()=>{
  const saved=globalThis.fetch;let calls=0;const url='https://c.tutti.ch/big/123.jpg';
  try {
    globalThis.fetch=async (u,options)=>{calls++;assert.equal(u,url);assert.equal(options.credentials,'omit');assert.equal(options.redirect,'error');return {ok:true,status:200,url,headers:new Headers({'content-type':'image/jpeg'}),body:new Blob([new Uint8Array([255,216,255])]).stream()}};
    const r=await handleRequest({op:'image',url});assert.equal(r.base64,'/9j/');assert.equal(calls,1);
    globalThis.fetch=async()=>{calls++;return {ok:false,status:429,url,headers:new Headers(),body:null}};
    const blocked=await handleRequest({op:'image',url});assert.equal(blocked.status,429);assert.equal(blocked.base64,undefined);assert.equal(calls,2);
  } finally {globalThis.fetch=saved;}
});
test('image stream is cancelled at the byte cap and HTML is rejected',async()=>{
  const saved=globalThis.fetch;let cancelled=false;const url='https://c.tutti.ch/big/123.jpg';
  try {
    globalThis.fetch=async()=>({ok:true,status:200,url,headers:new Headers({'content-type':'image/jpeg'}),body:{getReader:()=>({read:async()=>({done:false,value:new Uint8Array(4*1024*1024+1)}),cancel:async()=>{cancelled=true}})}});
    await assert.rejects(handleRequest({op:'image',url}),/4 MiB/);assert.equal(cancelled,true);
    globalThis.fetch=async()=>({ok:true,status:200,url,headers:new Headers({'content-type':'text/html'}),body:null});
    await assert.rejects(handleRequest({op:'image',url}),/content type/);
  } finally {globalThis.fetch=saved;}
});
