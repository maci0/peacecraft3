import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
for (const entry of ['index.html', 'demo.html']) test(`${entry} supplies the loader's optional module controls`, async () => {
  const html = await readFile(new URL(`../../web/${entry}`, import.meta.url), 'utf8');
  for (const id of ['client-module', 'client-module-request']) {
    assert.equal([...html.matchAll(new RegExp(`id="${id}"`, 'g'))].length, 1);
  }
  assert.match(html, /<input[^>]+id="client-module"[^>]+type="file"|<input[^>]+type="file"[^>]+id="client-module"/);
  assert.match(html, /aria-describedby="client-module-request"/);
  assert.match(html, /<output[^>]+id="client-module-request"[^>]+aria-live="polite"/);
  assert.match(html, /src="\.\/loader.js"/);
});
