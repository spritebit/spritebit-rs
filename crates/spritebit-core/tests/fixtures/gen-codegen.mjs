// Erzeugt Code-Ausgaben der Web-Version für das Fixture-Projekt.
// Aufruf: node gen.mjs <web-js-ordner> <fixture.json> <ausgabeordner>
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { pathToFileURL } from 'node:url';
import { join } from 'node:path';
const [jsDir, fixture, outDir] = process.argv.slice(2);
globalThis.document = { querySelectorAll: () => [], querySelector: () => null, getElementById: () => null, documentElement: { setAttribute() {}, removeAttribute() {}, dataset: {} }, title: '' };
globalThis.localStorage = { getItem: () => null, setItem() {}, removeItem() {} };

const imp = f => import(pathToFileURL(join(jsDir, f)).href);
const st = await imp('state.js');
const cg = await imp('codegen.js');
const i18n = await imp('i18n.js');
const proj = JSON.parse(readFileSync(fixture, 'utf8'));
mkdirSync(outDir, { recursive: true });
for (const [id, s] of Object.entries(proj.sprites)) {
  st.sprites[id] = st.makeSprite({ ...s, frames: s.frames });
  st.state.curSprite = id;
}
for (const lang of ['de', 'en']) {
  i18n.setLang(lang);
  for (const f of Object.keys(cg.CODE_FORMATS)) {
    for (const pal of [false, true]) {
      if (pal && !cg.CODE_FORMATS[f].palOption) continue;
      writeFileSync(join(outDir, `${f}${pal ? '-pal' : ''}.${lang}.txt`), cg.buildCode(f, pal));
    }
  }
}
console.log('ok', Object.keys(cg.CODE_FORMATS).join(' '));
