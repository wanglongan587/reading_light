import { test,expect,Page } from '@playwright/test';
const theme={paper:'#FAF6EA',text:'#353E35',accent:'#536A49',font:'serif',font_size:20,line_height:1.9,width:720,reduced_motion:true};
async function command(page:Page,value:object){await page.evaluate(value=>(window as any).glowReceive(value),value);}
async function open(page:Page,mode='scrolled',url='/book.epub',settings=theme){
 await page.goto('/index.html?session=test');
 await expect.poll(()=>page.evaluate(()=>(window as any).__events?.some((e:any)=>e.kind==='ready'))).toBeTruthy();
 await command(page,{version:1,session:'test',request_id:1,kind:'open',url,theme:settings,mode});
 await expect.poll(()=>page.evaluate(()=>(window as any).__events?.some((e:any)=>e.kind==='opened'))).toBeTruthy();
}
test('real EPUB renders, exposes chapters, and selected text produces a located echo',async({page})=>{
 await open(page);
 const frame=page.frames().find(f=>f.parentFrame()!==null)!;
 await expect(frame.locator('body')).toContainText('今天有些疲惫');
 await frame.evaluate(()=>{
  const p=document.querySelector('p')!;const range=document.createRange();range.selectNodeContents(p);
  const selection=getSelection()!;selection.removeAllRanges();selection.addRange(range);
  p.dispatchEvent(new MouseEvent('mouseup',{bubbles:true}));
 });
 await expect(page.locator('#echo')).toBeVisible();await page.locator('#echo').click();
 const event=await page.evaluate(()=>(window as any).__events.find((e:any)=>e.kind==='echo'));
 expect(event.text).toContain('疲惫');expect(event.cfi).toMatch(/^epubcfi\(/);expect(event.session).toBe('test');
 expect(await page.evaluate(()=>(window as any).__events.find((e:any)=>e.kind==='toc').entries.length)).toBe(2);
});
test('pagination, reflow and position restore keep the same passage',async({page})=>{
 await open(page,'paginated');
 await command(page,{version:1,session:'test',request_id:2,kind:'next'});
 await page.waitForTimeout(700);
 const cfi=await page.evaluate(()=>(window as any).__events.filter((e:any)=>e.kind==='location').at(-1).cfi);
 expect(cfi).toMatch(/^epubcfi\(/);
 await command(page,{version:1,session:'test',request_id:3,kind:'configure',theme:{...theme,font_size:24},mode:'scrolled'});
 await page.setViewportSize({width:570,height:680});await page.waitForTimeout(600);
 await command(page,{version:1,session:'test',request_id:4,kind:'locate',cfi});
 await expect.poll(()=>page.evaluate(()=>(window as any).__events.filter((e:any)=>e.kind==='error').length)).toBe(0);
 const ids=[...cfi.matchAll(/\[([^\]]+)\]/g)].map(m=>m[1]);
 const anchorId=ids.find(id=>/^p\d+$/.test(id));
 expect(anchorId).toBeTruthy();
 const first=page.frames().find(f=>f.parentFrame()!==null)!;
 await expect(first.locator('body')).toHaveCSS('font-size','24px');
 await expect.poll(async()=>{
  for(const frame of page.frames()){
   const visible=await frame.evaluate(id=>{
    const p=document.getElementById(id!);if(!p)return false;
    const rect=p.getBoundingClientRect();const outer=window.frameElement?.getBoundingClientRect();
    return !!outer&&outer.top+rect.bottom>0&&outer.top+rect.top<parent.innerHeight;
   },anchorId).catch(()=>false);
   if(visible)return true;
  }
  return false;
 }).toBe(true);
});
test('resizing after a page turn does not jump back to the chapter start',async({page})=>{
 await open(page,'paginated');
 await command(page,{version:1,session:'test',request_id:2,kind:'next'});
 await page.waitForTimeout(600);
 const visible=async()=>{
  for(const frame of page.frames()){
   const text=await frame.evaluate(()=>{
    for(const p of document.querySelectorAll('p')){
     const rect=p.getBoundingClientRect();
     if(rect.bottom>0&&rect.top<innerHeight&&(p.textContent?.trim().length||0)>12)return p.textContent!.trim();
    }
    return '';
   }).catch(()=>'');
   if(text)return text;
  }
  return '';
 };
 const before=await visible();
 expect(before.length).toBeGreaterThan(12);
 await page.setViewportSize({width:820,height:640});
 await page.waitForTimeout(1000);
 expect(await visible()).toBe(before);
 expect(await page.evaluate(()=>(window as any).__events.filter((e:any)=>e.kind==='error').length)).toBe(0);
});
test('stale sessions and replayed commands cannot replace the current book',async({page})=>{
 await open(page);
 await command(page,{version:1,session:'old',request_id:999,kind:'open',url:'/missing.epub',theme});
 await command(page,{version:1,session:'test',request_id:1,kind:'open',url:'/missing.epub',theme});
 await page.waitForTimeout(200);
 expect(await page.evaluate(()=>(window as any).__events.filter((e:any)=>e.kind==='error').length)).toBe(0);
 await expect(page.locator('#status')).toHaveText('');
});
test('closing and reopening restores a later page after initial layout settles',async({page})=>{
 await open(page,'paginated');
 for(let step=2;step<=5;step++){
  const previous=await page.evaluate(()=>(window as any).__events.filter((event:any)=>event.kind==='location').at(-1)?.cfi);
  await command(page,{version:1,session:'test',request_id:step,kind:'next'});
  await expect.poll(()=>page.evaluate(()=>(window as any).__events.filter((event:any)=>event.kind==='location').at(-1)?.cfi)).not.toBe(previous);
 }
 await page.waitForTimeout(400);
 const saved=await page.evaluate(()=>(window as any).__events.filter((event:any)=>event.kind==='location').at(-1).cfi);
 const anchor=[...saved.matchAll(/\[(p\d+)\]/g)].at(-1)?.[1];
 expect(anchor).toBeTruthy();
 await page.reload();
 await expect.poll(()=>page.evaluate(()=>(window as any).__events?.some((event:any)=>event.kind==='ready'))).toBeTruthy();
 await command(page,{version:1,session:'test',request_id:1,kind:'open',url:'/book.epub',theme,mode:'paginated',cfi:saved});
 await expect.poll(()=>page.evaluate(()=>(window as any).__events?.some((event:any)=>event.kind==='opened'))).toBeTruthy();
 await page.waitForTimeout(1200);
 const visible=await page.frames().find(frame=>frame.parentFrame()!==null)!.evaluate(id=>{
  const bounds=document.getElementById(id!)!.getBoundingClientRect();
  return bounds.right>0&&bounds.left<innerWidth&&bounds.bottom>0&&bounds.top<innerHeight;
 },anchor);
 expect(visible).toBe(true);
});
test('reader makes no external requests and reports a missing book',async({page})=>{
 const external:string[]=[];page.on('request',r=>{if(/^https?:/.test(r.url())&&!r.url().startsWith('http://127.0.0.1:4178'))external.push(r.url());});
 await open(page);expect(external).toEqual([]);
 await command(page,{version:1,session:'test',request_id:2,kind:'open',url:'/missing.epub',theme});
 await expect(page.locator('#status')).toContainText('暂时没有打开');
});

test('clearing a selection prevents the keyboard shortcut from submitting old text',async({page})=>{
 await open(page);
 const frame=page.frames().find(frame=>frame.parentFrame()!==null)!;
 await frame.evaluate(()=>{
  const paragraph=document.querySelector('p')!;
  const range=document.createRange();range.selectNodeContents(paragraph);
  const selection=getSelection()!;selection.removeAllRanges();selection.addRange(range);
  paragraph.dispatchEvent(new MouseEvent('mouseup',{bubbles:true}));
 });
 await expect(page.locator('#echo')).toBeVisible();
 await page.keyboard.press('Escape');
 await expect(page.locator('#echo')).toBeHidden();
 await page.keyboard.press('Control+Enter');
 await page.waitForTimeout(100);
 expect(await page.evaluate(()=>(window as any).__events.filter((event:any)=>event.kind==='echo'))).toEqual([]);
});

test('a failed replacement book can be retried in the same reader session',async({page})=>{
 await open(page);
 await command(page,{version:1,session:'test',request_id:2,kind:'open',url:'/missing.epub',theme});
 await expect(page.locator('#status')).toContainText('暂时没有打开');
 await command(page,{version:1,session:'test',request_id:3,kind:'open',url:'/book.epub',theme});
 await expect(page.locator('#status')).toHaveText('');
 await expect.poll(()=>page.evaluate(()=>(window as any).__events.filter((event:any)=>event.kind==='opened').length)).toBe(2);
 const frame=page.frames().find(frame=>frame.parentFrame()!==null)!;
 await expect(frame.locator('body')).toContainText('今天有些疲惫');
});

test('a valid chapter jump clears an earlier navigation failure',async({page})=>{
 await open(page);
 await command(page,{version:1,session:'test',request_id:2,kind:'locate',href:'missing-chapter.xhtml'});
 await expect(page.locator('#status')).toContainText('暂时没有打开');
 await command(page,{version:1,session:'test',request_id:3,kind:'locate',href:'c0.xhtml'});
 await expect(page.locator('#status')).toHaveText('');
 await expect.poll(()=>page.evaluate(()=>(window as any).__events.filter((event:any)=>event.kind==='opened').length)).toBe(2);
});

test('multi-chapter EPUB exposes its chapters and can navigate to a later chapter',async({page})=>{
 page.on('pageerror', error=>console.log('EXTERNAL_PAGE_ERROR',error.message));
 page.on('console', message=>{if(message.type()==='error')console.log('EXTERNAL_CONSOLE',message.text());});
 await page.goto('/index.html?session=test');
 await expect.poll(()=>page.evaluate(()=>(window as any).__events?.some((e:any)=>e.kind==='ready'))).toBeTruthy();
 await command(page,{version:1,session:'test',request_id:1,kind:'open',url:'/external.epub',theme,mode:'scrolled'});
 await expect.poll(()=>page.evaluate(()=>(window as any).__events?.some((e:any)=>e.kind==='opened'))).toBeTruthy();
 const toc=await page.evaluate(()=>(window as any).__events.find((e:any)=>e.kind==='toc').entries);
 expect(toc.length).toBeGreaterThan(3);
 const chapter=toc.find((entry:any)=>entry.label.includes('CHAPTER III')) || toc[3];
 await command(page,{version:1,session:'test',request_id:2,kind:'locate',href:chapter.href});
 await expect.poll(async()=>{
  for(const frame of page.frames()){
   if(await frame.locator('body').textContent().then(text=>text?.replace(/\s+/g,' ').includes(chapter.label.replace(/\s+/g,' '))).catch(()=>false))return true;
  }
  return false;
 }).toBe(true);
 expect(await page.evaluate(()=>(window as any).__events.filter((e:any)=>e.kind==='error').length)).toBe(0);
});

test('legacy full-screen spine EPUB opens and exposes its table of contents',async({page})=>{
 test.skip(!process.env.GLOW_TEST_BOOK,'GLOW_TEST_BOOK is required for the real-world EPUB regression');
 await page.goto('/index.html?session=test');
 await expect.poll(()=>page.evaluate(()=>(window as any).__events?.some((e:any)=>e.kind==='ready'))).toBeTruthy();
 await command(page,{version:1,session:'test',request_id:1,kind:'open',url:'/external.epub',theme,mode:'scrolled'});
 await expect.poll(()=>page.evaluate(()=>(window as any).__events?.some((e:any)=>e.kind==='opened')),{timeout:15000}).toBeTruthy();
 await expect(page.locator('#status')).toHaveText('');
 const toc=await page.evaluate(()=>(window as any).__events.find((e:any)=>e.kind==='toc')?.entries||[]);
 expect(toc.length).toBeGreaterThan(3);
 await expect.poll(async()=>{
  for(const frame of page.frames()){
   const text=await frame.locator('body').textContent().catch(()=>null);
   if((text?.trim().length||0)>40)return true;
  }
  return false;
 }).toBe(true);
 expect(await page.evaluate(()=>(window as any).__events.filter((e:any)=>e.kind==='error').length)).toBe(0);
});

test('a broken first spine item falls back to a readable navigation entry',async({page})=>{
 test.skip(!process.env.GLOW_TEST_BROKEN_SPINE,'GLOW_TEST_BROKEN_SPINE is required for the fallback regression');
 await page.goto('/index.html?session=test');
 await expect.poll(()=>page.evaluate(()=>(window as any).__events?.some((e:any)=>e.kind==='ready'))).toBeTruthy();
 await command(page,{version:1,session:'test',request_id:1,kind:'open',url:'/broken-spine.epub',theme,mode:'scrolled'});
 await expect.poll(()=>page.evaluate(()=>(window as any).__events?.some((e:any)=>e.kind==='opened')),{timeout:15000}).toBeTruthy();
 const events=await page.evaluate(()=>(window as any).__events);
 expect(events.find((e:any)=>e.kind==='toc').entries.length).toBeGreaterThan(3);
 expect(events.filter((e:any)=>e.kind==='error')).toEqual([]);
 await expect(page.locator('#status')).toHaveText('');
});

test('theme changes are applied to existing chapter documents without losing text',async({page})=>{
 await open(page);
 const first=page.frames().find(f=>f.parentFrame()!==null)!;
 await expect(first.locator('body')).toContainText('今天有些疲惫');
 const night={...theme,paper:'#202923',text:'#E5E7D9',font_size:22,accent:'#B4C99D'};
 await command(page,{version:1,session:'test',request_id:2,kind:'configure',theme:night,mode:'paginated'});
 await expect.poll(async()=>{
  for(const frame of page.frames()){
   if(await frame.locator('body').evaluate((el,expected)=>{
    const paragraph=el.querySelector('p');
    return getComputedStyle(el).backgroundColor==='rgb(32, 41, 35)'
     &&getComputedStyle(el).fontSize==='22px'
     &&!!paragraph
     &&getComputedStyle(paragraph).color===expected;
   },`rgb(229, 231, 217)`).catch(()=>false))return true;
  }
  return false;
 }).toBe(true);
 expect(await page.evaluate(()=>(window as any).__events.filter((e:any)=>e.kind==='error').length)).toBe(0);
});

test('front matter is skipped when no reading position has been saved',async({page})=>{
 await page.goto('/index.html?session=test');
 await expect.poll(()=>page.evaluate(()=>(window as any).__events?.some((e:any)=>e.kind==='ready'))).toBeTruthy();
 await command(page,{version:1,session:'test',request_id:1,kind:'open',url:'/frontmatter.epub',theme,mode:'scrolled'});
 await expect.poll(()=>page.evaluate(()=>(window as any).__events?.some((e:any)=>e.kind==='opened'))).toBeTruthy();
 const frame=page.frames().find(f=>f.parentFrame()!==null)!;
 await expect(frame.locator('body')).toContainText('正文从这里开始');
 await expect(frame.locator('body')).not.toContainText('前置页不应作为第一阅读页');
});

test('missing initial chapter recovers without reporting a fatal reader error',async({page})=>{
 await page.goto('/index.html?session=test');
 await expect.poll(()=>page.evaluate(()=>(window as any).__events?.some((event:any)=>event.kind==='ready'))).toBeTruthy();
 await command(page,{version:1,session:'test',request_id:1,kind:'open',url:'/broken-chapter.epub',theme,mode:'paginated'});
 await expect.poll(()=>page.evaluate(()=>(window as any).__events?.some((event:any)=>event.kind==='opened')),{timeout:6000}).toBeTruthy();
 expect(await page.evaluate(()=>(window as any).__events.filter((event:any)=>event.kind==='error'))).toEqual([]);
 await expect(page.locator('#status')).toHaveText('');
});

test('reduced motion setting controls animations inside the book',async({page})=>{
 await open(page,'scrolled','/animation.epub',{...theme,reduced_motion:false});
 const frame=page.frames().find(f=>f.parentFrame()!==null)!;
 const paragraph=frame.locator('p').first();
 await expect.poll(()=>paragraph.evaluate(p=>getComputedStyle(p).animationName)).toBe('glow-pulse');
 await command(page,{version:1,session:'test',request_id:2,kind:'configure',theme:{...theme,reduced_motion:true},mode:'scrolled'});
 await expect.poll(()=>paragraph.evaluate(p=>getComputedStyle(p).animationName)).toBe('none');
 await command(page,{version:1,session:'test',request_id:3,kind:'configure',theme:{...theme,reduced_motion:false},mode:'scrolled'});
 await expect.poll(()=>paragraph.evaluate(p=>getComputedStyle(p).animationName)).toBe('glow-pulse');
});

test('whole-book search returns a locatable excerpt',async({page})=>{
 await open(page);
 await command(page,{version:1,session:'test',request_id:2,kind:'search',query:'疲惫'});
 await expect.poll(()=>page.evaluate(()=>(window as any).__events.find((event:any)=>event.kind==='search')?.results.length||0)).toBeGreaterThan(0);
 const hit=await page.evaluate(()=>(window as any).__events.find((event:any)=>event.kind==='search').results[0]);
 expect(hit.excerpt).toContain('疲惫');
 expect(hit.cfi).toMatch(/^epubcfi\(/);
 expect(hit.chapter.length).toBeGreaterThan(0);
 await command(page,{version:1,session:'test',request_id:3,kind:'locate',cfi:hit.cfi});
 await expect(page.locator('#status')).toHaveText('');
 expect(await page.evaluate(()=>(window as any).__events.filter((event:any)=>event.kind==='error'))).toEqual([]);
});

test('highlights can be applied and removed without reloading the book',async({page})=>{
 page.on('pageerror',error=>console.log('HIGHLIGHT_PAGE_ERROR',error.message));
 await open(page);
 const frame=page.frames().find(f=>f.parentFrame()!==null)!;
 await frame.evaluate(()=>{
  const paragraph=document.querySelector('p')!;
  const range=document.createRange();range.selectNodeContents(paragraph);
  const selection=getSelection()!;selection.removeAllRanges();selection.addRange(range);
  paragraph.dispatchEvent(new MouseEvent('mouseup',{bubbles:true}));
 });
 await page.locator('#echo').click();
 const echo=await page.evaluate(()=>(window as any).__events.find((event:any)=>event.kind==='echo'));
 await command(page,{version:1,session:'test',request_id:2,kind:'highlight',highlight:{id:'highlight-1',cfi:echo.cfi,text:echo.text}});
 await expect(page.locator('[ref="glow-highlight"]')).toHaveCount(1);
 await command(page,{version:1,session:'test',request_id:3,kind:'unhighlight',cfi:echo.cfi});
 await expect(page.locator('[ref="glow-highlight"]')).toHaveCount(0);
});

test('saved highlights are restored when a book is reopened',async({page})=>{
 await open(page);
 const frame=page.frames().find(f=>f.parentFrame()!==null)!;
 await frame.evaluate(()=>{
  const paragraph=document.querySelector('p')!;
  const range=document.createRange();range.selectNodeContents(paragraph);
  const selection=getSelection()!;selection.removeAllRanges();selection.addRange(range);
  paragraph.dispatchEvent(new MouseEvent('mouseup',{bubbles:true}));
 });
 await page.locator('#echo').click();
 const echo=await page.evaluate(()=>(window as any).__events.find((event:any)=>event.kind==='echo'));
 await command(page,{version:1,session:'test',request_id:2,kind:'open',url:'/book.epub',theme,mode:'scrolled',highlights:[{id:'saved-1',cfi:echo.cfi,text:echo.text}]});
 await expect.poll(()=>page.evaluate(()=>(window as any).__events.filter((event:any)=>event.kind==='opened').length)).toBe(2);
 await expect(page.locator('[ref="glow-highlight"]')).toHaveCount(1);
});

test('reading progress advances as the reader moves to a later chapter',async({page})=>{
 await open(page,'scrolled','/external.epub');
 await expect.poll(()=>page.evaluate(()=>(window as any).__events.filter((event:any)=>event.kind==='location').at(-1)?.progress),{timeout:20000}).toEqual(expect.any(Number));
 const before=await page.evaluate(()=>(window as any).__events.filter((event:any)=>event.kind==='location').at(-1).progress);
 expect(await page.evaluate(()=>(window as any).__events.filter((event:any)=>event.kind==='location').at(-1).chapter.length)).toBeGreaterThan(0);
 expect(before).toBeGreaterThanOrEqual(0);
 const entries=await page.evaluate(()=>(window as any).__events.find((event:any)=>event.kind==='toc').entries);
 await command(page,{version:1,session:'test',request_id:2,kind:'locate',href:entries.at(-1).href});
 await expect.poll(()=>page.evaluate(()=>(window as any).__events.filter((event:any)=>event.kind==='location').at(-1)?.progress)).toBeGreaterThan(before);
});
