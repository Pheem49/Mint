// Measures actual orb GPU commands via Chrome CDP; start npm run dev:web first.
import { spawn } from 'node:child_process';
import { mkdtemp, rm } from 'node:fs/promises';
const profile = await mkdtemp('/tmp/mint-effects-chrome-');
const chrome = spawn('google-chrome', ['--headless=new','--enable-gpu','--use-angle=gl','--no-sandbox','--disable-dev-shm-usage','--remote-debugging-port=0',`--user-data-dir=${profile}`,'about:blank'], {stdio:['ignore','ignore','pipe']});
let stderr = '';
chrome.stderr.on('data', data => { stderr += data; });
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
let ws;
try {
  for (let i = 0; i < 100 && !stderr.includes('DevTools listening on'); i++) await sleep(100);
  const match = stderr.match(/DevTools listening on (ws:\/\/[^\s]+)/);
  if (!match) throw Error('Chrome did not start: ' + stderr.slice(-1000));
  const origin = 'http://' + new URL(match[1]).host;
  const pages = await (await fetch(origin + '/json/list')).json();
  ws = new WebSocket(pages.find(page => page.type === 'page').webSocketDebuggerUrl);
  await new Promise((resolve,reject) => { ws.onopen=resolve; ws.onerror=reject; });
  let sequence = 0; const pending = new Map();
  ws.onmessage = event => { const message=JSON.parse(event.data); if(message.id) { const waiter=pending.get(message.id); pending.delete(message.id); message.error ? waiter.reject(message.error) : waiter.resolve(message.result); } };
  const send = (method,params={}) => new Promise((resolve,reject)=> { const id=++sequence; pending.set(id,{resolve,reject}); ws.send(JSON.stringify({id,method,params})); });
  await send('Runtime.enable');
  await send('Page.enable');
  await send('Page.addScriptToEvaluateOnNewDocument', {source: `window.orbDraws=0;window.gpuSamples=[];window.gpuDisjoint=0;const queues=new Map();for(const C of [WebGLRenderingContext,WebGL2RenderingContext]){const draw=C.prototype.drawArrays;C.prototype.drawArrays=function(...args){window.orbDraws++;const ext=this.getExtension('EXT_disjoint_timer_query_webgl2');if(!ext)return draw.apply(this,args);const q=this.createQuery();this.beginQuery(ext.TIME_ELAPSED_EXT,q);const result=draw.apply(this,args);this.endQuery(ext.TIME_ELAPSED_EXT);if(!queues.has(this))queues.set(this,[]);queues.get(this).push(q);return result}}setInterval(()=>{for(const [gl,qs] of queues){const ext=gl.getExtension('EXT_disjoint_timer_query_webgl2');if(gl.getParameter(ext.GPU_DISJOINT_EXT)){window.gpuDisjoint++;qs.splice(0).forEach(q=>gl.deleteQuery(q));window.gpuSamples=[];continue}while(qs.length&&gl.getQueryParameter(qs[0],gl.QUERY_RESULT_AVAILABLE)){const q=qs.shift();window.gpuSamples.push(gl.getQueryParameter(q,gl.QUERY_RESULT)/1e6);gl.deleteQuery(q)}}},50)`});
  await send('Page.navigate',{url:process.argv[2] || 'http://127.0.0.1:9000/tests/effectsPerformance.browser.html'});
  await sleep(2500);
  const evaluate=async expression=>(await send('Runtime.evaluate',{expression,returnByValue:true})).result.value;
  console.log('GPU',await evaluate(`(()=>{const c=document.querySelector('canvas');const gl=c.getContext('webgl2')||c.getContext('webgl');const ext=gl.getExtension('WEBGL_debug_renderer_info');return {renderer:ext?gl.getParameter(ext.UNMASKED_RENDERER_WEBGL):gl.getParameter(gl.RENDERER),gpuTimer:!!(gl.getExtension('EXT_disjoint_timer_query_webgl2')||gl.getExtension('EXT_disjoint_timer_query'))}})()`));
  for(const mode of ['normal','reduced-motion','reduced-effects']){
    await send('Emulation.setEmulatedMedia',{features:[{name:'prefers-reduced-motion',value:mode==='reduced-motion'?'reduce':'no-preference'}]});
    await evaluate(`document.documentElement.setAttribute('data-reduced-effects','${mode==='reduced-effects'}')`);
    await sleep(300);
    await sleep(200);
    await evaluate('window.gpuSamples=[]');
    const before=await evaluate('window.orbDraws');await sleep(5000);const after=await evaluate('window.orbDraws');
    console.log(mode,JSON.stringify({drawsIn5s:after-before,gpu:await evaluate(`(()=>{const xs=window.gpuSamples.slice().sort((a,b)=>a-b);return {samples:xs.length,medianMs:xs.length?xs[Math.floor(xs.length/2)]:null,p95Ms:xs.length?xs[Math.floor(xs.length*.95)]:null,totalMs:xs.reduce((a,b)=>a+b,0),disjoint:window.gpuDisjoint}})()`),backgroundAnimation:await evaluate(`getComputedStyle(document.querySelector('.assistant-workspace'),'::before').animationName`)}));
  }

  await evaluate(`document.querySelector('#root').style.width='350px'`);await sleep(300);
  const resizeDraws=await evaluate('window.orbDraws');await sleep(1000);
  console.log('settled after resize',{drawsIn1s:(await evaluate('window.orbDraws'))-resizeDraws});
  await evaluate(`window.setOrb({hue:120})`);await sleep(300);
  console.log('hue change',{additionalDraws:(await evaluate('window.orbDraws'))-resizeDraws});
  await evaluate(`document.documentElement.setAttribute('data-reduced-effects','false');document.querySelector('#root').style.transform='translateY(2000px)'`);await sleep(300);
  const offscreen=await evaluate('window.orbDraws');await sleep(1000);
  console.log('offscreen',{drawsIn1s:(await evaluate('window.orbDraws'))-offscreen});
  await evaluate(`document.querySelector('#root').style.transform=''`);await sleep(300);
  const resumed=await evaluate('window.orbDraws');await sleep(1000);
  console.log('resumed',{drawsIn1s:(await evaluate('window.orbDraws'))-resumed});
  await evaluate(`window.setOrb({animate:false})`);await sleep(300);
  const paused=await evaluate('window.orbDraws');await sleep(1000);
  console.log('paused',{drawsIn1s:(await evaluate('window.orbDraws'))-paused});

} finally { ws?.close(); chrome.kill(); if (chrome.exitCode === null) await new Promise(resolve=>chrome.once('exit',resolve)); await rm(profile,{recursive:true,force:true,maxRetries:5,retryDelay:100}).catch(()=>{}); }
