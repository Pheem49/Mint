import React from 'react'
import {createRoot} from 'react-dom/client'
import LibraryPage from '../shared/components/studio/LibraryPage'
import {installRendererPlatform,type RendererAdapter} from '../shared/platform'
import type {LibraryItem,LibraryQuery} from '../shared/types'
let failAction=false;let offline=false;let openChat='';let contentReads=0
const canvas=document.createElement('canvas');canvas.width=480;canvas.height=300;const ctx=canvas.getContext('2d')!;ctx.fillStyle='#286454';ctx.fillRect(0,0,480,300);ctx.fillStyle='#daeee6';ctx.font='26px sans-serif';ctx.fillText('Library reference',100,160)
const image=await new Promise<Blob>(resolve=>canvas.toBlob(blob=>resolve(blob!)))
const make=(id:string,name:string,kind:LibraryItem['kind']):LibraryItem=>({id,name,filename:`${id}.bin`,kind,mimeType:kind==='file'?'text/markdown':kind==='video'?'video/mp4':'image/png',size:1280,createdAt:'2026-10-15T08:00:00Z',origin:'uploaded',source:'chat',message:'',deletedAt:null,thumbnailStatus:kind==='file'?'unavailable':'ready',availability:'ready',references:[{chatId:'A',interactionId:12}]})
let items=[make('image','ภาพอ้างอิง.png','image'),make('video','Demo recording.mp4','video'),make('notes','01_slr_model_assumptions.md','file'),make('lost','Missing original.pdf','file'),make('old','Archived notes.md','file')];items[3].availability='missing';items[4].deletedAt='2026-10-15T08:00:00Z'
items[0].message='/chat ช่วยดูภาพอ้างอิงนี้แล้วอธิบายรายละเอียดให้หน่อย โดยเปรียบเทียบโครงสร้าง สี และองค์ประกอบต่าง ๆ ที่ปรากฏในรูปภาพเพื่อใช้ประกอบการออกแบบหน้าจอของเรา รวมถึงส่วนที่ควรปรับปรุงให้เหมาะกับการใช้งานบนมือถือ [Image #1]'

installRendererPlatform({
 listLibrary:async(q:LibraryQuery)=>{if(offline)throw new Error('Offline');return {items:items.filter(i=>Boolean(i.deletedAt)===Boolean(q.trashed)&&(!q.kind||i.kind===q.kind)&&(!q.search||i.name.toLowerCase().includes(q.search.toLowerCase()))),nextCursor:null}},
 getLibraryItem:async(id:string)=>{const i=items.find(i=>i.id===id);if(!i)throw new Error('Missing item');return i},
 readLibraryFile:async(id:string,thumb:boolean)=>{if(!thumb)contentReads++;return id==='notes'||id==='old'?new Blob(['# Notes\nRegression reference'],{type:'text/markdown'}):image},
 removeLibraryItem:async(id:string)=>{if(failAction)throw new Error('System Trash unavailable');items=items.filter(i=>i.id!==id)},
 setLibraryTrashed:async(id:string,trashed:boolean)=>{if(failAction)throw new Error('Storage unavailable');items=items.map(i=>i.id===id?{...i,deletedAt:trashed?'2026-10-15':null}:i)},
} as unknown as RendererAdapter)
createRoot(document.getElementById('root')!).render(<><style>{`*{box-sizing:border-box}body{margin:0;font-family:Arial,sans-serif;--bg-gradient:#101112;--bg-overlay:#18191b;--bg-surface-soft:#1a1c1e;--bg-surface-hover:#242729;--border:#303438;--text-main:#edf0ee;--text-soft:#abb6b0;--text-muted:#89958e;--accent:#91d6b5;--radius-md:10px}#root{position:relative;height:100dvh}#qa-results{position:fixed;bottom:4px;left:8px;color:#a9e5c7;font-size:10px;pointer-events:none;white-space:pre-wrap;z-index:99999}`}</style><LibraryPage chatId="A" onClose={()=>{}} onOpenConversation={id=>{openChat=id}}/><pre id="qa-results">Running…</pre></>)
const delay=(ms=40)=>new Promise(r=>setTimeout(r,ms))
async function waitFor(check:()=>boolean,name:string){for(let i=0;i<100;i++){if(check())return;await delay()}throw new Error(name)}
const results:string[]=[];const check=(value:unknown,name:string)=>{if(!value)throw new Error(name);results.push(`PASS ${name}`)}
const click=(selector:string)=>{const el=document.querySelector<HTMLElement>(selector);if(!el)throw new Error(`Missing ${selector}`);el.focus();el.click()}
async function run(){
 if(location.search.includes('item=old')){
  await waitFor(()=>Boolean(document.querySelector('.library-item.is-linked')),'Source link highlights its Library entry in Trash')
  check(document.querySelector('.library-item.is-linked')?.textContent?.includes('Archived notes.md'),'linked Trash entry is visible')
  document.getElementById('qa-results')!.textContent=results.join('\n')+'\nALL PASSED';return
 }
 await waitFor(()=>document.querySelectorAll('.library-item').length===4,'Library lists active items')
 check(contentReads===0,'listing never loads original media')
 check(!document.querySelector('[data-remove]'),'remove action is available only in Library Trash')
 check(document.documentElement.scrollWidth<=innerWidth,'Library fits viewport')
 if(new URLSearchParams(location.search).has('preview')){document.getElementById('qa-results')!.textContent='';return}
 click('[data-preview="image"]');await waitFor(()=>Boolean(document.querySelector('.library-preview img')),'Image preview')
 const excerpt=document.querySelector<HTMLElement>('.library-preview-source p')
 check(excerpt?.textContent?.startsWith('ช่วยดูภาพอ้างอิงนี้'),'image preview shows the source message without transport markers')
 check(!excerpt?.textContent?.includes('[Image #1]'),'source excerpt omits attachment markers')
 check(excerpt!.clientHeight<=parseFloat(getComputedStyle(excerpt!).lineHeight)*2+1,'source message fits within two lines')
 if(innerWidth<600)check(excerpt!.scrollHeight>excerpt!.clientHeight,'long source message is visually truncated')
 check(document.documentElement.scrollWidth<=innerWidth,'long source message fits viewport')
 if(location.search.includes('source-preview')){document.getElementById('qa-results')!.textContent='';return}
 document.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true}));await waitFor(()=>!document.querySelector('.library-preview'),'Close image preview')
 click('[data-filter="file"]');await waitFor(()=>document.querySelectorAll('.library-item').length===2,'Files filter')
 click('[data-preview="notes"]');await waitFor(()=>Boolean(document.querySelector('.library-preview pre')?.textContent?.includes('Regression reference')),'Document preview')
 check(document.querySelector('.library-preview pre')?.textContent?.includes('Regression reference'),'text preview reads original file')
 check(!document.querySelector('.library-preview-source'),'preview without a source message omits the excerpt')
 click('.library-preview [data-open-chat]');check(openChat==='A','source conversation opens')
 document.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true}));await waitFor(()=>!document.querySelector('.library-preview'),'Escape closes preview')
 check(document.activeElement===document.querySelector('[data-preview="notes"]'),'preview restores keyboard focus')
 failAction=true;click('[data-trash="notes"]');await waitFor(()=>Boolean(document.querySelector('.library-action-error')),'Trash error')
 check(Boolean(document.querySelector('[data-preview="notes"]')),'failed Trash keeps file visible')
 failAction=false;click('[data-trash="notes"]');await waitFor(()=>!document.querySelector('[data-preview="notes"]'),'Successful Trash')
 click('[data-view="trash"]');await waitFor(()=>document.querySelectorAll('.library-item').length===2,'Trash view')
 check(Boolean(document.querySelector('[data-remove="old"]')),'Library Trash offers remove from Mint')
 failAction=true;click('[data-remove="old"]');await waitFor(()=>Boolean(document.querySelector('.library-action-error')),'System Trash error')
 check(Boolean(document.querySelector('[data-remove="old"]')),'failed system Trash keeps entry visible')
 failAction=false;click('[data-remove="old"]');await waitFor(()=>!document.querySelector('[data-remove="old"]'),'Removed from Mint')
 click('[data-restore="notes"]');await waitFor(()=>!document.querySelector('[data-restore="notes"]'),'Restore')
 click('[data-view="active"]');await waitFor(()=>Boolean(document.querySelector('[data-preview="notes"]')),'Restored file')
 check(Boolean(document.querySelector('[data-preview="lost"][disabled]')),'missing original remains visible with disabled preview')
 offline=true;click('[aria-label="Refresh library"]');await waitFor(()=>Boolean(document.querySelector('.library-error')),'Refresh error')
 check(document.querySelectorAll('.library-item').length===2,'failed refresh preserves known files')
 offline=false;click('.library-error button');await waitFor(()=>!document.querySelector('.library-error'),'Retry')
 const search=document.querySelector<HTMLInputElement>('input[type="search"]')!;Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,'value')!.set!.call(search,'absent');search.dispatchEvent(new Event('input',{bubbles:true}));await waitFor(()=>Boolean(document.querySelector('.library-empty')),'Empty search')
 check(document.querySelector('.library-empty')?.textContent?.includes('No files match'),'empty search explains filters')
 const navigate=(query:string)=>{history.pushState({},'',`?${query}`);window.dispatchEvent(new PopStateEvent('popstate'))}
 navigate('item=image');await waitFor(()=>Boolean(document.querySelector('.library-preview img')),'Source navigation opens while Library remains mounted')
 check(document.querySelector<HTMLInputElement>('input[type="search"]')?.value==='','source navigation clears filters hiding the target')
 items.push({...make('source-trash','Another archived file.md','file'),deletedAt:'2026-10-15'})
 navigate('item=source-trash&trash=1');await waitFor(()=>Boolean(document.querySelector('[data-remove="source-trash"]')),'Second source navigation switches to Trash')
 check(!document.querySelector('.library-preview'),'second source navigation closes the previous preview')
 check(document.querySelector('.library-item.is-linked')?.textContent?.includes('Another archived file.md'),'second source navigation highlights its own entry')
 navigate('item=notes');await waitFor(()=>Boolean(document.querySelector('.library-preview pre')),'Source navigation returns from Trash to an active original')
 document.getElementById('qa-results')!.textContent=results.join('\n')+'\nALL PASSED'
}
run().catch(error=>{document.getElementById('qa-results')!.textContent=results.join('\n')+`\nFAIL ${error.message}`})
