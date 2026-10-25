'use client';

// React Bits VoicePill source supplied for this integration. Mint adds actual
// recorder-state synchronization, async startup handling, and cancellation cleanup.
import { useEffect, useRef, useState } from 'react';
import { HugeiconsIcon } from '@hugeicons/react';
import { ArrowLeft01Icon, Mic01Icon } from '@hugeicons/core-free-icons';
import './VoicePill.css';

const LOOP = 4.8;
const SYLLABLES = [[0.1,0.16,0.9],[0.3,0.12,0.7],[0.5,0.2,1],[0.95,0.14,0.8],[1.15,0.1,0.6],[1.3,0.22,0.95],[1.9,0.16,0.85],[2.12,0.12,0.7],[2.3,0.18,0.9],[2.55,0.1,0.5],[3.05,0.24,1],[3.4,0.12,0.75],[3.6,0.16,0.9]];
const MIC_BINS = [[1,4],[4,11],[11,33]];
const MIC_GAIN = 2.2, DT_MAX = 0.05, SLIDE_MIN = 4, WAVE_EVERY = 4, WAVE_MAX = 80;
const simulatedLevel = t => {
  const u = t % LOOP;
  let a = 0.06;
  for (const [s,d,p] of SYLLABLES) {
    const x = (u-s)/d;
    if (x >= 0 && x <= 1) a = Math.max(a,p*0.5*(1-Math.cos(2*Math.PI*x)));
  }
  return a*(0.7+0.3*Math.abs(Math.sin(2*Math.PI*7.1*u)));
};
const micLevel = (analyser,buf) => {
  analyser.getByteFrequencyData(buf);
  let total = 0;
  for (const [lo,hi] of MIC_BINS) {
    let s = 0;
    for(let i=lo;i<hi;i++) s+=buf[i];
    total+=s/((hi-lo)*255);
  }
  return total/MIC_BINS.length*MIC_GAIN;
};
const drawWave = (s,canvas,level,color,floor) => {
  const dpr=Math.min(2,window.devicePixelRatio||1),rect=canvas.getBoundingClientRect();
  const W=Math.max(1,Math.round(rect.width*dpr)),H=Math.max(1,Math.round(rect.height*dpr));
  if(canvas.width!==W||canvas.height!==H){canvas.width=W;canvas.height=H;}
  const ctx=canvas.getContext('2d');if(!ctx)return;
  s.acc=Math.max(s.acc,level);s.tick=(s.tick+1)%WAVE_EVERY;
  if(s.tick===0){s.hist.push(s.acc);s.acc=0;if(s.hist.length>WAVE_MAX)s.hist.shift();}
  const bw=2*dpr,step=3*dpr,shift=s.tick/WAVE_EVERY*step;
  ctx.clearRect(0,0,W,H);ctx.fillStyle=color;
  for(let i=0;i<s.hist.length;i++){
    const v=s.hist[s.hist.length-1-i],x=W-(i+1)*step-shift;if(x+bw<0)break;
    const h=Math.max(bw,(floor+(1-floor)*v)*H),t=Math.min(1,Math.max(0,(x+bw/2)/(W*0.55))),fade=t*t*(3-2*t);
    ctx.globalAlpha=(0.35+0.65*v)*fade;ctx.beginPath();ctx.roundRect(x,(H-h)/2,bw,h,bw/2);ctx.fill();
  }
  ctx.globalAlpha=1;
};
const clock=ms=>{const s=Math.floor(ms/1000);return `${Math.floor(s/60)}:${String(s%60).padStart(2,'0')}`;};
const openMic=async(s,generation)=>{
  const Ctx=window.AudioContext||window.webkitAudioContext;
  if(!Ctx||!navigator.mediaDevices?.getUserMedia)throw new Error('unsupported');
  s.audio??={ctx:new Ctx()};const a=s.audio;
  if(a.ctx.state==='suspended')await a.ctx.resume();
  const stream=await navigator.mediaDevices.getUserMedia({audio:true});
  if(!s.listening||s.generation!==generation){stream.getTracks().forEach(t=>t.stop());return;}
  a.stream=stream;a.src=a.ctx.createMediaStreamSource(stream);a.analyser=a.ctx.createAnalyser();
  a.analyser.fftSize=256;a.analyser.smoothingTimeConstant=0;a.src.connect(a.analyser);a.buf=new Uint8Array(a.analyser.frequencyBinCount);
};
const closeMic=s=>{
  const a=s.audio;if(!a?.stream)return;
  a.stream.getTracks().forEach(t=>t.stop());a.src?.disconnect();a.stream=null;a.src=null;a.analyser=null;a.buf=null;
};

export default function VoicePill({
  accentColor='#f5f5f5',iconColor='#a1a1aa',background='#27272a',size=28,shape='pill',reach=8,
  showTime=true,waveform=true,slideToCancel=true,cancelDistance=64,attack=40,release=240,sensitivity=1,
  floor=0.1,openDuration=200,pressScale=0.95,mode='auto',holdAfter=300,reactive='simulated',
  disabled=false,ariaLabel='Dictate',onStart,onStop,className='',active,id,title
}){
  const [listening,setListening]=useState(Boolean(active));
  const [pending,setPending]=useState(false);
  const [pressed,setPressed]=useState(false);
  const [input,setInput]=useState('pointer');
  const timeRef=useRef(null),rootRef=useRef(null),waveRef=useRef(null);
  const st=useRef({listening:Boolean(active),pointerId:null,ownPress:false,downX:0,sliding:false,hist:[],tick:0,acc:0,downAt:0,startedAt:0,raf:0,last:0,env:0,t0:0,audio:null,generation:0});
  const cfg=useRef({});
  cfg.current={attack,release,sensitivity,floor,mode,holdAfter,reactive,showTime,waveform,slideToCancel,cancelDistance,accentColor,onStart,onStop,active,disabled};
  const frame=now=>{
    const s=st.current,c=cfg.current,dt=Math.min((now-s.last)/1000,DT_MAX);s.last=now;
    let target=0;
    if(s.listening){if(s.audio?.analyser)target=micLevel(s.audio.analyser,s.audio.buf);else if(c.reactive!=='mic')target=simulatedLevel((now-s.t0)/1000);}
    target=Math.min(1,target*c.sensitivity);
    const tau=Math.max(1,target>s.env?c.attack:c.release)/1000;s.env+=(target-s.env)*(1-Math.exp(-dt/tau));
    if(s.listening&&c.showTime&&timeRef.current){const text=clock(now-s.startedAt);if(timeRef.current.textContent!==text)timeRef.current.textContent=text;}
    if(s.listening&&c.waveform&&waveRef.current)drawWave(s,waveRef.current,s.env,c.accentColor,c.floor);
    s.raf=s.listening?requestAnimationFrame(frame):0;
  };
  const begin=kind=>{
    const s=st.current,c=cfg.current;if(s.listening||c.disabled)return;
    s.listening=true;s.generation++;const generation=s.generation;
    s.hist=[];s.tick=0;s.acc=0;s.env=0;s.startedAt=performance.now();s.t0=s.startedAt;s.last=s.startedAt;
    if(timeRef.current)timeRef.current.textContent='0:00';
    setListening(c.active===undefined);setPending(c.active!==undefined);setInput(kind);
    if(!s.raf)s.raf=requestAnimationFrame(frame);
    try{
      const started=c.onStart?.({source:c.reactive});
      Promise.resolve(started).then(ok=>{
        if(s.generation!==generation)return;
        setPending(false);if(ok===false)end('start-failed',false);
      }).catch(()=>{if(s.generation===generation)end('start-failed',false);});
    }catch{end('start-failed',false);}
    if(c.reactive==='mic')openMic(s,generation).catch(()=>{if(s.generation===generation)end('mic-denied');});
  };
  const end=(reason,notify=true)=>{
    const s=st.current,c=cfg.current;if(!s.listening)return;
    s.listening=false;s.generation++;closeMic(s);cancelAnimationFrame(s.raf);s.raf=0;
    setListening(false);setPending(false);setInput(reason==='key'||reason==='escape'?'key':'pointer');
    if(notify)c.onStop?.({reason,duration:Math.round(performance.now()-s.startedAt)});
  };
  const settleSlide=()=>{
    st.current.sliding=false;const root=rootRef.current;if(!root)return;
    delete root.dataset.sliding;root.style.setProperty('--vp-slide','0px');root.style.setProperty('--vp-cancel','0');
  };
  const onPointerMove=e=>{
    const s=st.current,c=cfg.current,root=rootRef.current;
    if(!root||s.pointerId!==e.pointerId||!c.slideToCancel||!s.listening||!s.ownPress)return;
    const dx=e.clientX-s.downX;if(!s.sliding&&dx>-SLIDE_MIN)return;
    s.sliding=true;root.dataset.sliding='';const pull=Math.min(c.cancelDistance+24,Math.max(0,-dx));
    root.style.setProperty('--vp-slide',`${-pull}px`);const progress=Math.min(1,pull/c.cancelDistance);
    root.style.setProperty('--vp-cancel',progress.toFixed(3));if(progress>=1){settleSlide();end('cancel');}
  };
  const onPointerDown=e=>{
    const s=st.current;if(disabled||e.button!==0||!e.isPrimary||s.pointerId!==null)return;
    e.preventDefault();e.currentTarget.focus({preventScroll:true});
    s.pointerId=e.pointerId;s.downX=e.clientX;s.downAt=performance.now();
    try{e.currentTarget.setPointerCapture(e.pointerId);}catch{}
    setPressed(true);s.ownPress=!s.listening;if(!s.listening)begin('pointer');
  };
  const clearPress=e=>{
    const s=st.current;if(e.pointerId!==s.pointerId)return false;
    s.pointerId=null;setPressed(false);if(s.sliding)settleSlide();
    try{if(e.currentTarget.hasPointerCapture(e.pointerId))e.currentTarget.releasePointerCapture(e.pointerId);}catch{}
    return true;
  };
  const onPointerUp=e=>{
    const s=st.current,c=cfg.current;if(!clearPress(e)||!s.listening)return;
    const held=performance.now()-s.downAt,isHold=c.mode==='hold'||(c.mode==='auto'&&held>=c.holdAfter);
    if(s.ownPress){if(isHold)end('release');}else end(held<c.holdAfter?'tap':'release');
  };
  const onPointerCancel=e=>{if(clearPress(e))end('cancel');};
  const onKeyDown=e=>{
    if(e.key==='Escape'&&st.current.listening){e.preventDefault();e.stopPropagation();settleSlide();end('escape');return;}
    if((e.key===' '||e.key==='Enter')&&!e.repeat){e.preventDefault();if(st.current.listening)end('key');else begin('key');}
  };
  const onClick=e=>{
    if(e.detail===0&&st.current.pointerId===null&&!e.nativeEvent.pointerType){if(st.current.listening)end('key');else begin('key');}
  };
  useEffect(()=>{
    if(active===undefined)return;
    const s=st.current;
    if(active){
      if(!s.listening){s.listening=true;s.generation++;s.hist=[];s.env=0;}
      s.startedAt=performance.now();s.t0=s.startedAt;s.last=s.startedAt;
      setListening(true);setPending(false);if(!s.raf)s.raf=requestAnimationFrame(frame);
    }else if(s.listening)end('external',false);
  },[active]);
  useEffect(()=>{
    if(!listening&&!pending)return;
    const stop=()=>{settleSlide();st.current.pointerId=null;setPressed(false);end('blur');};
    const onVis=()=>{if(document.hidden)stop();};
    window.addEventListener('blur',stop);document.addEventListener('visibilitychange',onVis);
    return()=>{window.removeEventListener('blur',stop);document.removeEventListener('visibilitychange',onVis);};
  },[listening,pending]);
  useEffect(()=>{if(disabled){settleSlide();st.current.pointerId=null;setPressed(false);end('disabled');}},[disabled]);
  useEffect(()=>{
    const s=st.current;
    return()=>{
      if(s.listening){s.listening=false;s.generation++;cfg.current.onStop?.({reason:'unmount',duration:Math.round(performance.now()-s.startedAt)});}
      closeMic(s);cancelAnimationFrame(s.raf);s.raf=0;s.audio?.ctx.close();
    };
  },[]);
  const radius=shape==='rounded'?Math.round(size*0.29):size/2,hit=Math.max(0,Math.min(10,(44-size)/2));
  const timeSize=Math.max(10,Math.round(size*0.36)),clockW=showTime?Math.round(timeSize*2.5)+4:0,waveW=waveform?Math.round(size*1.9):0,extra=clockW+waveW;
  return <button id={id} title={title} type="button" disabled={disabled} aria-label={ariaLabel} aria-pressed={listening} aria-busy={pending||undefined}
    className={`voice-pill${className?` ${className}`:''}`} data-state={listening?'listening':'idle'} data-pending={pending?'':undefined}
    data-pressed={pressed?'':undefined} data-input={input} data-time={showTime?'':undefined} ref={rootRef}
    onPointerDown={onPointerDown} onPointerMove={onPointerMove} onPointerUp={onPointerUp} onPointerCancel={onPointerCancel}
    onLostPointerCapture={onPointerCancel} onKeyDown={onKeyDown} onClick={onClick} onContextMenu={e=>e.preventDefault()}
    style={{'--vp-accent':accentColor,'--vp-icon':iconColor,'--vp-bg':background,'--vp-size':`${size}px`,'--vp-radius':`${radius}px`,
      '--vp-reach':`${reach}px`,'--vp-extra':`${extra}px`,'--vp-clock-w':`${clockW}px`,'--vp-wave-w':`${waveW}px`,
      '--vp-stop':`${Math.round(size*0.32)}px`,'--vp-icon-size':`${Math.round(size*0.54)}px`,'--vp-time-size':`${timeSize}px`,
      '--vp-open':`${openDuration}ms`,'--vp-press':pressScale,'--vp-hit':`${hit}px`}}>
    <span className="voice-pill__capsule" aria-hidden="true" />
    {waveform?<canvas ref={waveRef} className="voice-pill__wave" aria-hidden="true" />:null}
    {slideToCancel?<span className="voice-pill__cancel" aria-hidden="true"><HugeiconsIcon icon={ArrowLeft01Icon} size={12} strokeWidth={2.2}/><span>Cancel</span></span>:null}
    {showTime?<span ref={timeRef} className="voice-pill__time" aria-hidden="true">0:00</span>:null}
    <span className="voice-pill__glyph" aria-hidden="true"><span className="voice-pill__mic"><HugeiconsIcon icon={Mic01Icon} size={Math.round(size*0.54)} strokeWidth={2}/></span><span className="voice-pill__stop"/></span>
  </button>;
}
