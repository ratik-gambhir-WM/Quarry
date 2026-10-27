// Regenerate golden files against a read-only TypeScript server checkout.
// Run with: node --import <checkout>/node_modules/tsx/dist/esm/index.mjs generate-reference.mjs <checkout>/server
import { createRequire } from 'node:module';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
const referenceServer = path.resolve(process.argv[2]);
const directory = path.dirname(fileURLToPath(import.meta.url));
const require = createRequire(path.join(referenceServer, 'package.json'));
const PptxGenJS = require('pptxgenjs');
const JSZip = require('jszip');
const { LibraryPowerPointConverter } = await import(pathToFileURL(path.join(referenceServer, 'src/services/PowerPointConverter.ts')).href);
const { buildThemedPptxBytes, normalizePresentationSpec } = await import(pathToFileURL(path.join(referenceServer, 'src/lib/export/PowerpointGenerator.ts')).href);

{
const pptx = new PptxGenJS(); pptx.layout='LAYOUT_WIDE';
const slide=pptx.addSlide(); slide.background={color:'FEDCBA'};
slide.addText('Lower text',{x:1,y:4,w:3,h:.3,fontFace:'Arial',fontSize:12,margin:0,fill:{color:'070154',transparency:20},color:'FFFFFF'});
slide.addText([{text:'Top',options:{bold:true,fontFace:'Arial',fontSize:19.5}},{text:' line\nNext line',options:{italic:true,fontFace:'Aptos',fontSize:11}}],{x:1,y:1,w:4,h:.5,margin:0,shape:'roundRect',rectRadius:0.1,fill:{color:'E8EEF8'}});
slide.addShape('bentConnector2',{x:2,y:2,w:3,h:1,line:{color:'0047FF',width:2,beginArrowType:'diamond',endArrowType:'triangle'}});
slide.addShape('rect',{x:14,y:1,w:1,h:1,fill:{color:'00AA00'}});
slide.addShape('rect',{x:4,y:4,w:1,h:1,fill:{color:'0099FF'}});
const zip=await JSZip.loadAsync(await pptx.write({outputType:'nodebuffer'}));
let xml=await zip.file('ppt/slides/slide1.xml').async('text');
xml=xml.replace(/(<p:cNvPr\b[^>]*name="Shape 4")/, '$1 hidden="1"');
zip.file('ppt/slides/slide1.xml',xml);
const bytes=await zip.generateAsync({type:'nodebuffer'});
await writeFile(path.join(directory, 'styled.pptx'),bytes);
await writeFile(path.join(directory, 'styled.reference.json'),JSON.stringify(await new LibraryPowerPointConverter().convert(bytes),null,2));
}

{
const ns='xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"';
const shape=(id,text,x=0,y=0,extra='')=>`<p:sp><p:nvSpPr><p:cNvPr id="${id}" name="Box ${id}"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="${x}" y="${y}"/><a:ext cx="952500" cy="476250"/></a:xfrm><a:prstGeom prst="rect"/><a:solidFill><a:srgbClr val="E8EEF8"/></a:solidFill></p:spPr><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr sz="1200"/><a:t>${text}</a:t></a:r></a:p></p:txBody>${extra}</p:sp>`;
const zip=await JSZip.loadAsync(await readFile(path.join(directory, 'styled.pptx')));
const ph=`<p:sp><p:nvSpPr><p:cNvPr id="40" name="Title"/><p:cNvSpPr/><p:nvPr><p:ph type="title"/></p:nvPr></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr/><a:t>Inherited title</a:t></a:r></a:p></p:txBody></p:sp>`;
const phLayout=shape(41,'Click to edit master',952500,476250).replace('<p:nvPr/>','<p:nvPr><p:ph type="title"/></p:nvPr>').replace('<a:bodyPr/>','<a:bodyPr anchor="t" lIns="127000"/>').replace('<a:lstStyle/>','<a:lstStyle><a:lvl1pPr algn="ctr"><a:defRPr sz="2500" b="1"><a:solidFill><a:srgbClr val="0047FF"/></a:solidFill><a:latin typeface="Arial"/></a:defRPr></a:lvl1pPr></a:lstStyle>');
zip.file('ppt/slides/slide1.xml',`<p:sld ${ns}><p:cSld><p:spTree>${ph}</p:spTree></p:cSld></p:sld>`);
zip.file('ppt/slideLayouts/slideLayout1.xml',`<p:sldLayout ${ns}><p:cSld><p:bg><p:bgPr><a:solidFill><a:srgbClr val="F1E2D3"/></a:solidFill></p:bgPr></p:bg><p:spTree>${phLayout}${shape(42,'Layout decoration',0,4000000)}${shape(44,'Outside',19000000,0)}</p:spTree></p:cSld></p:sldLayout>`);
zip.file('ppt/slideMasters/slideMaster1.xml',`<p:sldMaster ${ns}><p:cSld><p:spTree>${shape(43,'Master decoration',0,5000000)}</p:spTree></p:cSld></p:sldMaster>`);
async function save(name,z){ const bytes=await z.generateAsync({type:'nodebuffer'}); await writeFile(`${directory}/${name}.pptx`,bytes);await writeFile(`${directory}/${name}.reference.json`,JSON.stringify(await new LibraryPowerPointConverter().convert(bytes),null,2)); }
await save('inherited',zip);
zip.file('ppt/slides/slide1.xml',`<p:sld ${ns} showMasterSp="0"><p:cSld><p:spTree>${ph}</p:spTree></p:cSld></p:sld>`);
await save('hidden-master',zip);
const transform=(attr,x,y,cx,cy,chx,chy,chcx,chcy)=>`<p:grpSpPr><a:xfrm ${attr}><a:off x="${x}" y="${y}"/><a:ext cx="${cx}" cy="${cy}"/><a:chOff x="${chx}" y="${chy}"/><a:chExt cx="${chcx}" cy="${chcy}"/></a:xfrm></p:grpSpPr>`;
zip.file('ppt/slides/slide1.xml',`<p:sld ${ns} showMasterSp="0"><p:cSld><p:spTree><p:grpSp>${transform('rot="1800000" flipH="1"',2000000,1000000,3000000,2000000,0,0,1000000,1000000)}<p:grpSp>${transform('rot="5400000"',100000,200000,500000,500000,0,0,500000,500000)}${shape(61,'Transformed',0,0)}</p:grpSp></p:grpSp></p:spTree></p:cSld></p:sld>`);
zip.file('ppt/slideLayouts/slideLayout1.xml',`<p:sldLayout ${ns}><p:cSld><p:spTree/></p:cSld></p:sldLayout>`);
await save('nested-transform',zip);
zip.file('ppt/slides/slide1.xml',`<p:sld ${ns} showMasterSp="0"><p:cSld><p:spTree>${shape(70,'').replace('<a:prstGeom prst="rect"/>','<a:custGeom/>').replace('<a:solidFill><a:srgbClr val="E8EEF8"/></a:solidFill>','<a:gradFill/>')}<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="71" name="Chart"/></p:nvGraphicFramePr><p:xfrm><a:off x="0" y="0"/><a:ext cx="952500" cy="952500"/></p:xfrm><a:graphic><a:graphicData><c:chart xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" r:id="missing"/></a:graphicData></a:graphic></p:graphicFrame></p:spTree></p:cSld></p:sld>`);
await save('fallback-warnings',zip);
zip.file('ppt/slides/slide1.xml',`<p:sld ${ns} showMasterSp="0"><p:cSld/></p:sld>`);
await save('empty',zip);
}

{
const reference=JSON.parse(await readFile(path.join(directory, 'styled.reference.json'),'utf8'));
const normalized=normalizePresentationSpec(reference.templateJson);
const bytes=await buildThemedPptxBytes(normalized.presentation);
await writeFile(path.join(directory, 'styled.export-reference.json'),JSON.stringify(await new LibraryPowerPointConverter().convert(Buffer.from(bytes)),null,2));
}

{
  const source = new PptxGenJS();
  source.layout = 'LAYOUT_WIDE';
  const slide = source.addSlide();
  slide.addTable([
    ['Category', 'Assessment'],
    ['Architecture', 'A long assessment that wraps across several lines to exercise weighted automatic row sizing.'],
    ['Security', 'Brief'],
  ], { x: 1, y: 1, w: 7, h: 4, colW: [2, 5], fontSize: 12, fontFace: 'Arial', color: 'FFFFFF', fill: '070154', margin: 0, border: { color: '0047FF', pt: 1 } });
  const zip = await JSZip.loadAsync(await source.write({ outputType: 'nodebuffer' }));
  let index = 0;
  const xml = (await zip.file('ppt/slides/slide1.xml').async('text'))
    .replace(/<a:tr h="\d+">/g, () => `<a:tr h="${index++ === 0 ? 914400 : 0}">`);
  zip.file('ppt/slides/slide1.xml', xml);
  const bytes = await zip.generateAsync({ type: 'nodebuffer' });
  await writeFile(path.join(directory, 'table.pptx'), bytes);
  await writeFile(path.join(directory, 'table.reference.json'), JSON.stringify(await new LibraryPowerPointConverter().convert(bytes), null, 2));
}
