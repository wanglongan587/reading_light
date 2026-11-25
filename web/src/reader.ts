import ePub from 'epubjs';

type Theme = { id?:string;paper:string;text:string;accent:string;font:string;font_size:number;line_height:number;width:number;font_url?:string;reduced_motion:boolean };
type Highlight = { id:string;cfi:string;text?:string };
type Command = { version:number;session:string;request_id:number;kind:string;url?:string;cfi?:string;href?:string;mode?:string;theme?:Theme;query?:string;highlight?:Highlight;highlights?:Highlight[] };
declare global { interface Window { ipc?:{postMessage:(s:string)=>void}; glowReceive:(cmd:Command)=>void; __events?:unknown[] } }
const session = new URLSearchParams(location.search).get('session') || 'test';
let book:any, rendition:any, currentCfi='', mode='scrolled', theme:Theme;
let counter=0, lastCommand=0, selectedCfi='', selectedText='', disposed=false;
let queue=Promise.resolve(); let relocating=false; let generation=0;
let tocEntries:{label:string;href:string}[]=[], currentHref='', locationsReady=false, searchGeneration=0;
let bookHighlights:Highlight[]=[]; const appliedHighlights=new Set<string>();
let openWatchdog:ReturnType<typeof setTimeout>|undefined;
let readerFailed=false;
const reader=document.querySelector<HTMLElement>('#reader')!;
const status=document.querySelector<HTMLElement>('#status')!;
const echo=document.querySelector<HTMLButtonElement>('#echo')!;
function send(kind:string, extra:Record<string,unknown>={}) {
 const event={version:1,session,request_id:++counter,kind,...extra};
 if(window.ipc) window.ipc.postMessage(JSON.stringify(event));
 else (window.__events??=[]).push(event);
}
function hideEcho(){echo.hidden=true;selectedText='';selectedCfi='';}
function clearOpenWatchdog(){if(openWatchdog!==undefined){clearTimeout(openWatchdog);openWatchdog=undefined;}}
function failure(reason?:unknown){
 readerFailed=true;
 clearOpenWatchdog();
 if(reason instanceof Error)console.error(`[GLOW_READER] ${reason.message}`);
 status.textContent='这一页暂时没有打开。请返回书架后重试。';
 send('error',{text:status.textContent});
}
function recoverNavigation(){
 if(readerFailed){readerFailed=false;status.textContent='';send('opened');}
}
async function withTimeout<T>(task:Promise<T>,milliseconds:number,label:string):Promise<T>{
 let timer:ReturnType<typeof setTimeout>|undefined;
 try{
  return await Promise.race([task,new Promise<T>((_,reject)=>{
   timer=setTimeout(()=>reject(new Error(`${label}加载超时`)),milliseconds);
  })]);
 }finally{
  if(timer!==undefined)clearTimeout(timer);
 }
}
async function displayChapter(target?:string){
 const active=rendition;
 const section=book.spine.get(target);
 if(!section)throw new Error('未找到对应章节');
 await withTimeout(Promise.resolve(section.load(book.load.bind(book))),10000,'章节内容');
 let rejectDisplay:(reason:unknown)=>void=()=>{};
 const displayError=new Promise<never>((_,reject)=>{rejectDisplay=reject;});
 const onError=(reason:unknown)=>rejectDisplay(reason instanceof Error?reason:new Error('章节显示失败'));
 active.on('displayError',onError);
 try{
  await withTimeout(Promise.race([active.display(target),displayError]),15000,'章节');
 }finally{
  active.off('displayError',onError);
 }
}
function readNavigation(items:any[]):{label:string;href:string}[]{
 const entries:{label:string;href:string}[]=[];
 const walk=(nodes:any[])=>{
  for(const item of nodes){
   const label=typeof item?.label==='string'?item.label.trim().slice(0,300):'';
   if(label&&typeof item?.href==='string')entries.push({label,href:item.href});
   if(Array.isArray(item?.subitems))walk(item.subitems);
  }
 };
 walk(Array.isArray(items)?items:[]);
 return entries;
}
function chapterForHref(href:string){
 if(!href)return '';
 const clean=href.split('#')[0];
 const entry=tocEntries.find(item=>item.href.split('#')[0]===clean);
 return entry?.label||'';
}
function firstReadableHref(entries:{label:string;href:string}[],book:any){
 const chapter=entries.find(entry=>/^(第[\d一二三四五六七八九十百千零〇两]+[章节回卷篇部]|chapter\s+[\divxlcdm]+|part\s+[\divxlcdm]+|正文|prologue|epilogue)(?:\.?\s*|[:：、]\s*|$)/i.test(entry.label.trim()));
 if(chapter)return chapter.href;
 const content=entries.find(entry=>!/^(封面|版权|书名|扉页|目录|书籍信息|cover|copyright|title\s*page|contents|人物介绍|人物关系|地图)/i.test(entry.label.trim()));
 return content?.href||entries[0]?.href||book.spine?.get(0)?.href;
}
function addHighlight(item:Highlight){
 if(!rendition||!item?.cfi||appliedHighlights.has(item.cfi))return;
 try{
  rendition.annotations.highlight(item.cfi,{id:item.id},undefined,'glow-highlight',{fill:theme.accent,'fill-opacity':'0.22','mix-blend-mode':'multiply'});
  appliedHighlights.add(item.cfi);
 }catch(error){
  if(error instanceof Error)console.error(`[GLOW_READER] ${error.message}`);
 }
}
function removeHighlight(cfi:string){
 if(!cfi)return;
 try{rendition?.annotations.remove(cfi,'highlight');}catch{}
 appliedHighlights.delete(cfi);
}
function applyHighlights(items:Highlight[]){
 for(const item of items)addHighlight(item);
}
function progressFor(cfi:string){
 if(!locationsReady||!book?.locations)return undefined;
 try{
  const value=book.locations.percentageFromCfi(cfi);
  return Number.isFinite(value)?Math.max(0,Math.min(1,value)):undefined;
 }catch{return undefined;}
}
function sendLocation(){
 if(!currentCfi)return;
 const progress=progressFor(currentCfi);
 send('location',{cfi:currentCfi,chapter:chapterForHref(currentHref),...(progress===undefined?{}:{progress})});
}
function styles(){
 const value:Record<string,Record<string,string>>={
 'html':{'background':`${theme.paper} !important`,'scrollbar-color':`${theme.accent}99 ${theme.paper}`,'scrollbar-width':'thin'},
 'body':{'color':`${theme.text} !important`,'background':`${theme.paper} !important`,'font-family':`${theme.font} !important`,'font-size':`${theme.font_size}px !important`,'line-height':`${theme.line_height} !important`,'padding':'28px 36px !important','max-width':`${theme.width}px`,'margin':'0 auto !important','overflow-wrap':'anywhere'},
 'body *':{'color':`${theme.text} !important`,'background-color':'transparent !important','font-family':'inherit !important'},
 'p':{'line-height':`${theme.line_height} !important`,'margin-bottom':'0.95em !important'},
 'h1':{'font-size':'1.35em !important','font-weight':'500','margin':'1.6em 0 !important'},
 'h2':{'font-size':'1.2em !important'},
 'img':{'max-width':'100% !important','height':'auto !important'},
 'a':{'color':`${theme.accent} !important`},
 'code, pre, kbd, samp':{'font-family':'Consolas, "Courier New", monospace !important'},
 '::-webkit-scrollbar':{'width':'10px','height':'10px','background':`${theme.paper} !important`},
 '::-webkit-scrollbar-thumb':{'background':`${theme.accent}80 !important`,'border-radius':'999px','border':`2px solid ${theme.paper}`},
 '::selection':{'background':`${theme.accent}40`}
 };
 if(theme.reduced_motion)value['*']={'animation':'none !important','transition':'none !important'};
 return value;
}
function styleText(){
 return Object.entries(styles()).map(([selector,declarations])=>`${selector}{${Object.entries(declarations).map(([name,value])=>`${name}:${value}`).join(';')}}`).join('\n');
}
let glowRaf=0, pendingX=0, pendingY=0;
function updateCursorGlow(x:number,y:number){
 if(theme?.reduced_motion)return;
 pendingX=x;pendingY=y;
 if(glowRaf)return;
 glowRaf=requestAnimationFrame(()=>{
  glowRaf=0;
  document.documentElement.style.setProperty('--mx',`${pendingX}px`);
  document.documentElement.style.setProperty('--my',`${pendingY}px`);
  document.documentElement.style.setProperty('--glow-op','1');
 });
}
function spawnRipple(x:number,y:number){
 if(theme?.reduced_motion)return;
 const el=document.createElement('span');
 el.className='prism-ripple';
 el.style.left=`${x}px`;el.style.top=`${y}px`;
 document.body.append(el);
 setTimeout(()=>el.remove(),700);
}
document.addEventListener('mousemove',e=>updateCursorGlow(e.clientX,e.clientY),{passive:true});
document.addEventListener('mouseleave',()=>document.documentElement.style.setProperty('--glow-op','0'));
async function applyTheme(){
 if(!theme)return;
 document.documentElement.dataset.theme=theme.id||'forest';
 document.documentElement.dataset.reducedMotion=theme.reduced_motion?'yes':'no';
 document.documentElement.style.setProperty('--paper',theme.paper);
 document.documentElement.style.setProperty('--ink',theme.text);
 document.documentElement.style.setProperty('--accent',theme.accent);
 if(theme.reduced_motion)document.documentElement.style.setProperty('--glow-op','0');
 if(rendition){
   for(const content of rendition.getContents()) await prepareContent(content);
 }
}
async function prepareContent(content:any){
 const doc:Document=content.document;
 // This frame contains sanitized book markup, never application scripts.
 content.addStylesheetCss(styleText(),'glow-theme');
 if(theme?.font_url){
  let style=doc.querySelector<HTMLStyleElement>('#glow-font');
  if(!style){style=doc.createElement('style');style.id='glow-font';doc.head.append(style);}
  const rule=`@font-face{font-family:"${theme.font}";src:url("${theme.font_url}")}`;
  if(style.textContent!==rule)style.textContent=rule;
  if(doc.fonts){
   try{await withTimeout(doc.fonts.load(`${theme.font_size}px "${theme.font}"`),2500,'字体');}catch{}
  }
 }else{
  doc.querySelector('#glow-font')?.remove();
 }
 if(doc.fonts){
  try{await withTimeout(doc.fonts.ready,2500,'字体');}catch{}
 }
 if(doc.documentElement.dataset.glowBound)return;
 doc.documentElement.dataset.glowBound='yes';
 doc.addEventListener('keydown',onKey);
 doc.addEventListener('click',(event)=>{
  const a=(event.target as Element)?.closest('a');
  if(a){const href=a.getAttribute('href')||'';if(/^[a-z]+:|^\/\//i.test(href)){event.preventDefault();event.stopImmediatePropagation();}}
 },true);
 doc.addEventListener('mousemove',(event)=>{
  const frame=(content.window.frameElement as HTMLElement|null)?.getBoundingClientRect();
  if(frame)updateCursorGlow(frame.left+event.clientX,frame.top+event.clientY);
 },{passive:true});
 doc.addEventListener('mousedown',(event)=>{
  hideEcho();
  const frame=(content.window.frameElement as HTMLElement|null)?.getBoundingClientRect();
  if(frame&&event.button===0)spawnRipple(frame.left+event.clientX,frame.top+event.clientY);
 });
 doc.addEventListener('scroll',hideEcho,{passive:true});
 doc.addEventListener('mouseup',()=>setTimeout(()=>{
  const selection=doc.getSelection();if(!selection?.rangeCount||selection.isCollapsed){hideEcho();return;}
  const range=selection.getRangeAt(0);const text=selection.toString().trim();
  if(!text||text.length>4000){hideEcho();return;}
  const bounds=range.getBoundingClientRect();const frame=(content.window.frameElement as HTMLElement).getBoundingClientRect();
  selectedText=text;selectedCfi=content.cfiFromRange(range);
  echo.style.left=`${Math.max(10,Math.min(innerWidth-110,frame.left+bounds.left+bounds.width/2-45))}px`;
  echo.style.top=`${Math.max(10,Math.min(innerHeight-55,frame.top+bounds.bottom+10))}px`;
  echo.hidden=false;send('selection',{text:selectedText,cfi:selectedCfi});
 },20));
}
function onKey(e:KeyboardEvent){
 if(e.key==='Escape'){hideEcho();send('escape');return;}
 if((e.ctrlKey||e.metaKey)&&e.key==='Enter'&&selectedText){e.preventDefault();requestEcho();return;}
 if(mode==='paginated'&&['ArrowLeft','ArrowRight','PageUp','PageDown'].includes(e.key)){
  e.preventDefault();queue=queue.then(async()=>{
   await (e.key==='ArrowLeft'||e.key==='PageUp'?rendition.prev():rendition.next());
   await captureLocation();
  }).catch(failure);
 }
}
function requestEcho(){if(selectedText&&selectedCfi){send('echo',{text:selectedText,cfi:selectedCfi});hideEcho();}}
echo.addEventListener('mousedown',e=>e.preventDefault());echo.addEventListener('click',requestEcho);
document.addEventListener('keydown',onKey);

async function open(cmd:Command){
 readerFailed=false;
 const own=++generation;searchGeneration++;clearOpenWatchdog();
 openWatchdog=setTimeout(()=>failure(new Error('打开书籍超时')),75000);
 status.textContent='正在铺开这一页…';hideEcho();
 rendition?.destroy();book?.destroy();reader.replaceChildren();currentCfi='';currentHref='';selectedText='';selectedCfi='';locationsReady=false;appliedHighlights.clear();
 theme=cmd.theme!;mode=cmd.mode||'scrolled';
 const response=await withTimeout(fetch(cmd.url!),15000,'书籍文件');if(!response.ok)throw new Error('book load');
 const data=await withTimeout(response.arrayBuffer(),30000,'书籍内容');if(disposed||own!==generation)return;
 book=ePub(data as any,{replacements:'blobUrl'});
 await withTimeout(Promise.resolve(book.ready),20000,'书籍结构');if(disposed||own!==generation)return;
 let entries:{label:string;href:string}[]=[];
 try{
  const navigation=await withTimeout(Promise.resolve(book.loaded.navigation),8000,'目录');
  entries=readNavigation(navigation.toc).slice(0,2000);
 }catch(error){
  if(error instanceof Error)console.error(`[GLOW_READER] ${error.message}`);
 }
 if(disposed||own!==generation)return;
 tocEntries=entries;bookHighlights=cmd.highlights??[];
 send('toc',{entries});
 const mount=()=>{
  appliedHighlights.clear();
  rendition=book.renderTo(reader,{width:'100%',height:'100%',flow:mode==='paginated'?'paginated':'scrolled-doc',manager:mode==='paginated'?'default':'continuous',spread:'none',allowScriptedContent:false});
  rendition.hooks.content.register(prepareContent);
  rendition.on('relocated',(location:any)=>{if(relocating)return;currentCfi=location.start.cfi;currentHref=location.start.href||'';sendLocation();});
  applyHighlights(bookHighlights);
 };
 mount();await applyTheme();
 status.textContent=entries.length?'目录已整理，正在找到你上次停留的页面…':'正在找到可阅读的正文…';
 const firstHref=firstReadableHref(entries,book);
 const requested=cmd.cfi||firstHref;
 let displayed=false;
 if(requested){
  try{await displayChapter(requested);displayed=true;}catch(error){
   if(error instanceof Error)console.error(`[GLOW_READER] ${error.message}`);
  }
 }
 if(!displayed){
  rendition?.destroy();rendition=undefined;
  if(disposed||own!==generation)return;
  mount();await applyTheme();
  status.textContent='正在换一条路线进入正文…';
  const fallback=entries.find(entry=>entry.href!==requested)?.href||firstHref;
  if(!fallback)throw new Error('没有可显示的章节');
  await displayChapter(fallback);
 }
 await applyTheme();
 try{await withTimeout(captureLocation(),5000,'阅读位置');}catch{}
 if(disposed||own!==generation)return;
 status.textContent='';clearOpenWatchdog();send('opened');
 void generateLocations(own);
}
async function captureLocation(){
 if(!rendition)return;
 await new Promise<void>(resolve=>requestAnimationFrame(()=>resolve()));
 const raw=await rendition.manager.currentLocation();
 const location=rendition.located(raw);
 if(location?.start?.cfi){currentCfi=location.start.cfi;currentHref=location.start.href||currentHref;sendLocation();}
}
async function generateLocations(own:number){
 try{
  await withTimeout(Promise.resolve(book.locations.generate(1600)),60000,'阅读进度');
  if(disposed||own!==generation)return;
  locationsReady=true;
  await captureLocation();
 }catch(error){
  if(error instanceof Error)console.error(`[GLOW_READER] ${error.message}`);
 }
}
function textNodes(doc:Document){
 const nodes:{node:Text;start:number;end:number}[]=[];
 const walker=doc.createTreeWalker(doc.body,NodeFilter.SHOW_TEXT);
 let text='';
 while(walker.nextNode()){
  const node=walker.currentNode as Text;
  const value=node.nodeValue||'';
  if(!value)continue;
  nodes.push({node,start:text.length,end:text.length+value.length});
  text+=value;
 }
 return {nodes,text};
}
function rangeAtOffsets(doc:Document,nodes:{node:Text;start:number;end:number}[],start:number,end:number){
 const startNode=nodes.find(item=>start>=item.start&&start<=item.end)||nodes.at(-1);
 const endNode=nodes.find(item=>end>=item.start&&end<=item.end)||nodes.at(-1);
 if(!startNode||!endNode)return;
 const range=doc.createRange();
 range.setStart(startNode.node,Math.max(0,Math.min(start-startNode.start,startNode.end-startNode.start)));
 range.setEnd(endNode.node,Math.max(0,Math.min(end-endNode.start,endNode.end-endNode.start)));
 return range;
}
async function searchBook(query:string){
 const value=query.trim().slice(0,100);
 if(!book||!value)return;
 const own=++searchGeneration;
 const needle=value.toLocaleLowerCase();
 const results:{cfi:string;excerpt:string;chapter:string}[]=[];
 const currentIndex=currentCfi?book.spine.get(currentCfi)?.index:-1;
 for(const section of book.spine.spineItems||[]){
  if(disposed||own!==searchGeneration)return;
  try{await withTimeout(Promise.resolve(section.load(book.load.bind(book))),10000,'章节内容');}catch{continue;}
  if(disposed||own!==searchGeneration)return;
  try{
   const {nodes,text}=textNodes(section.document);
   if(!nodes.length||!text)continue;
   const haystack=text.toLocaleLowerCase();
   let from=0,position=0;
   while((position=haystack.indexOf(needle,from))!==-1){
    const end=position+needle.length;
    const range=rangeAtOffsets(section.document,nodes,position,end);
    if(range){
     const cfi=section.cfiFromRange(range);
     if(cfi&&!results.some(item=>item.cfi===cfi)){
      const excerpt=text.slice(Math.max(0,position-55),Math.min(text.length,end+75)).replace(/\s+/g,' ').trim();
      results.push({cfi,excerpt,chapter:chapterForHref(section.href)});
     }
    }
    if(results.length>=80)break;
    from=Math.max(end,position+1);
   }
  }catch(error){
   if(error instanceof Error)console.error(`[GLOW_READER] ${error.message}`);
  }
  if(section.index!==currentIndex){
   try{section.unload();}catch{}
  }
  if(results.length>=80)break;
 }
 if(disposed||own!==searchGeneration)return;
 send('search',{text:value,results});
}
async function reflow(cmd:Command){
 if(!rendition)return;
 const saved=currentCfi;relocating=true;hideEcho();
 try{
  if(cmd.theme)theme=cmd.theme;
  if(cmd.mode&&cmd.mode!==mode){mode=cmd.mode;rendition.flow(mode==='paginated'?'paginated':'scrolled-doc');}
  await applyTheme();rendition.resize(reader.clientWidth,reader.clientHeight);
  if(saved)await displayChapter(saved);
 }finally{relocating=false;}
 await captureLocation();
}
window.glowReceive=(cmd)=>{
 if(cmd.version!==1||cmd.session!==session||!Number.isSafeInteger(cmd.request_id)||cmd.request_id<=lastCommand)return;
 lastCommand=cmd.request_id;
 queue=queue.then(async()=>{
  if(disposed)return;
  switch(cmd.kind){
   case 'open':await open(cmd);break;
   case 'configure':await reflow(cmd);break;
   case 'locate':
    if(rendition){await displayChapter(cmd.cfi||cmd.href);await captureLocation();recoverNavigation();}
    break;
   case 'next':
    hideEcho();
    if(rendition){await rendition.next();await captureLocation();recoverNavigation();}
    break;
   case 'prev':
    hideEcho();
    if(rendition){await rendition.prev();await captureLocation();recoverNavigation();}
    break;
   case 'restore':await reflow(cmd);break;
   case 'search':void searchBook(cmd.query||'');break;
   case 'highlight':
    if(cmd.highlight){
     if(!bookHighlights.some(item=>item.cfi===cmd.highlight!.cfi))bookHighlights.push(cmd.highlight);
     addHighlight(cmd.highlight);
    }
    break;
   case 'unhighlight':
    bookHighlights=bookHighlights.filter(item=>item.cfi!==cmd.cfi);
    removeHighlight(cmd.cfi||'');
    break;
  }
 }).catch(failure);
};
let resizeTimer:ReturnType<typeof setTimeout>;
new ResizeObserver(()=>{clearTimeout(resizeTimer);resizeTimer=setTimeout(()=>{
 if(rendition&&reader.clientWidth>0)queue=queue.then(()=>reflow({version:1,session,request_id:0,kind:'restore'})).catch(failure);
},150);}).observe(reader);
window.addEventListener('beforeunload',()=>{disposed=true;clearTimeout(resizeTimer);rendition?.destroy();book?.destroy();});
send('ready');
