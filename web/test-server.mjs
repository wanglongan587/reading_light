import http from 'node:http';
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import JSZip from 'jszip';
const routes={ '/index.html':['dist/index.html','text/html'], '/reader.js':['dist/reader.js','text/javascript'], '/reader.css':['dist/reader.css','text/css'], '/book.epub':['../target/fixtures/book.epub','application/epub+zip'] };
const publicBook = process.env.GLOW_TEST_BOOK
 ? path.resolve(process.env.GLOW_TEST_BOOK)
 : fileURLToPath(new URL('../target/fixtures/external.epub',import.meta.url));
const brokenSpineBook = process.env.GLOW_TEST_BROKEN_SPINE ? path.resolve(process.env.GLOW_TEST_BROKEN_SPINE) : null;
const frontmatterBook = await (async()=>{
 const zip=new JSZip();
 zip.file('mimetype','application/epub+zip',{compression:'STORE'});
 zip.file('META-INF/container.xml','<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>');
 zip.file('OEBPS/content.opf','<?xml version="1.0" encoding="UTF-8"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="bookid"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="bookid">frontmatter-test</dc:identifier><dc:title>前置页测试</dc:title><dc:language>zh-CN</dc:language></metadata><manifest><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/><item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/><item id="title" href="text/title.xhtml" media-type="application/xhtml+xml"/><item id="ch1" href="text/chapter1.xhtml" media-type="application/xhtml+xml"/><item id="ch2" href="text/chapter2.xhtml" media-type="application/xhtml+xml"/></manifest><spine toc="ncx"><itemref idref="title"/><itemref idref="ch1"/><itemref idref="ch2"/></spine></package>');
 zip.file('OEBPS/nav.xhtml','<?xml version="1.0" encoding="UTF-8"?><html xmlns="http://www.w3.org/1999/xhtml"><head><title>目录</title></head><body><nav epub:type="toc" xmlns:epub="http://www.idpf.org/2007/ops"><ol><li><a href="text/title.xhtml">版权信息</a></li><li><a href="text/chapter1.xhtml">第一章 开始</a></li><li><a href="text/chapter2.xhtml">第二章 后来</a></li></ol></nav></body></html>');
 zip.file('OEBPS/toc.ncx','<?xml version="1.0" encoding="UTF-8"?><ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1"><head/><docTitle><text>前置页测试</text></docTitle><navMap><navPoint id="n1" playOrder="1"><navLabel><text>版权信息</text></navLabel><content src="text/title.xhtml"/></navPoint><navPoint id="n2" playOrder="2"><navLabel><text>第一章 开始</text></navLabel><content src="text/chapter1.xhtml"/></navPoint><navPoint id="n3" playOrder="3"><navLabel><text>第二章 后来</text></navLabel><content src="text/chapter2.xhtml"/></navPoint></navMap></ncx>');
 zip.file('OEBPS/text/title.xhtml','<html xmlns="http://www.w3.org/1999/xhtml"><head><title>版权信息</title></head><body><h1>版权信息</h1><p>前置页不应作为第一阅读页。</p></body></html>');
 zip.file('OEBPS/text/chapter1.xhtml','<html xmlns="http://www.w3.org/1999/xhtml"><head><title>第一章</title></head><body><h1>第一章 开始</h1><p>正文从这里开始。窗外的光落在书页上。</p></body></html>');
 zip.file('OEBPS/text/chapter2.xhtml','<html xmlns="http://www.w3.org/1999/xhtml"><head><title>第二章</title></head><body><h1>第二章 后来</h1><p>故事继续向前。</p></body></html>');
 return zip.generateAsync({type:'nodebuffer',compression:'DEFLATE',mimeType:'application/epub+zip'});
})();
const animationBook = await (async()=>{
 const zip=new JSZip();
 zip.file('mimetype','application/epub+zip',{compression:'STORE'});
 zip.file('META-INF/container.xml','<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>');
 zip.file('OEBPS/content.opf','<?xml version="1.0" encoding="UTF-8"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="bookid"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="bookid">animation-test</dc:identifier><dc:title>动画测试</dc:title><dc:language>zh-CN</dc:language></metadata><manifest><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/><item id="ch1" href="text/chapter1.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="ch1"/></spine></package>');
 zip.file('OEBPS/nav.xhtml','<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><head><title>目录</title></head><body><nav epub:type="toc"><ol><li><a href="text/chapter1.xhtml">第一章 动画</a></li></ol></nav></body></html>');
 zip.file('OEBPS/text/chapter1.xhtml','<html xmlns="http://www.w3.org/1999/xhtml"><head><title>第一章</title><style>@keyframes glow-pulse{from{opacity:1}to{opacity:.6}}p{animation:glow-pulse 2s infinite}</style></head><body><h1>第一章 动画</h1><p>这一页带有书内动画，用来检查减少动态设置。</p></body></html>');
 return zip.generateAsync({type:'nodebuffer',compression:'DEFLATE',mimeType:'application/epub+zip'});
})();
const brokenChapterZip=await JSZip.loadAsync(frontmatterBook);
brokenChapterZip.remove('OEBPS/text/chapter1.xhtml');
const brokenChapterBook=await brokenChapterZip.generateAsync({type:'nodebuffer',compression:'DEFLATE'});
http.createServer(async(req,res)=>{
 const pathname = new URL(req.url,'http://localhost').pathname;
 const route = pathname === '/external.epub' && publicBook
  ? [publicBook,'application/epub+zip']
  : pathname === '/broken-spine.epub' && brokenSpineBook
  ? [brokenSpineBook,'application/epub+zip']
  : routes[pathname];
 if(pathname === '/frontmatter.epub'){res.writeHead(200,{'Content-Type':'application/epub+zip'});res.end(frontmatterBook);return;}
 if(pathname === '/animation.epub'){res.writeHead(200,{'Content-Type':'application/epub+zip'});res.end(animationBook);return;}
 if(pathname === '/broken-chapter.epub'){res.writeHead(200,{'Content-Type':'application/epub+zip'});res.end(brokenChapterBook);return;}
 if(!route){res.writeHead(404).end();return;}
 try {
  const bytes = await readFile(route[0].startsWith('/') || /^[A-Za-z]:[\\/]/.test(route[0]) ? route[0] : fileURLToPath(new URL(route[0],import.meta.url)));
  if (!res.headersSent) res.writeHead(200,{'Content-Type':route[1]});
  res.end(bytes);
 } catch (error) {
  console.error('TEST_SERVER_READ_FAILED',route[0],error.message);
  if (!res.headersSent) res.writeHead(500);
  res.end();
 }
}).listen(4178,'127.0.0.1');
