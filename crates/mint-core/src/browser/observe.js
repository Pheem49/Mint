// Keep actual DOM nodes rather than reconstructing selectors from labels.
const elements = new Map();
const all = [];
const nodes = globalThis.__mintNodes ||= {ids:new WeakMap(),next:0,documentId:(crypto.randomUUID?.() || String(performance.timeOrigin)+Math.random())};
function walk(root) {
  for (const el of root.querySelectorAll('*')) {
    if (el.id?.startsWith('mint-')) continue;
    if (el.matches('a[href],button,input:not([type=hidden]),textarea,select,[role=button],[role=link],[role=checkbox],[role=combobox],[contenteditable=true]')) {
      const style = getComputedStyle(el), rect = el.getBoundingClientRect();
      if (rect.width && rect.height && style.display !== 'none' && style.visibility !== 'hidden') all.push(el);
    }
    if (el.shadowRoot) walk(el.shadowRoot);
  }
}
walk(document);
const page = all.slice(elementOffset, elementOffset + 100).map((el, index) => {
  const ref = observationId + ':' + (elementOffset + index);
  elements.set(ref, el);
  if (!nodes.ids.has(el)) nodes.ids.set(el, ++nodes.next);
  const tag = el.tagName.toLowerCase();
  const labelled = (el.getAttribute('aria-labelledby') || '').split(/\s+/).filter(Boolean)
    .map(id => el.getRootNode().getElementById?.(id)?.textContent || '').join(' ');
  const name = el.getAttribute('aria-label') || labelled || Array.from(el.labels || []).map(l => l.textContent).join(' ')
    || el.innerText || el.getAttribute('placeholder') || el.getAttribute('title') || '';
  const role = el.getAttribute('role') || ({a:'link',button:'button',textarea:'textbox',select:'combobox'})[tag]
    || (el.type === 'checkbox' ? 'checkbox' : el.type === 'radio' ? 'radio' : ['submit','button'].includes(el.type) ? 'button' : 'textbox');
  const rect = el.getBoundingClientRect();
  return {ref,nodeId:nodes.documentId+':'+nodes.ids.get(el),role,name:name.trim().slice(0,120),inputType:el.type || null,
    value:el.type === 'password' ? null : (el.value == null ? null : String(el.value).slice(0,300)),checked:el.checked ?? null,
    enabled:!el.disabled && el.getAttribute('aria-disabled') !== 'true',visible:true,
    inViewport:rect.bottom > 0 && rect.top < innerHeight && rect.right > 0 && rect.left < innerWidth};
});
globalThis.__mintObservation = {id:observationId,elements,document};
const text = document.body?.innerText || '';
return {tabId:'',url:location.href,title:document.title,observationId,scrollX,scrollY,
  text:text.slice(textOffset,textOffset+12000),elements:page,
  textTruncated:textOffset+12000<text.length,elementsTruncated:elementOffset+100<all.length,
  nextTextOffset:textOffset+12000<text.length?textOffset+12000:null,
  nextElementOffset:elementOffset+100<all.length?elementOffset+100:null,
  unsupportedFrames:document.querySelectorAll('iframe,frame').length};
