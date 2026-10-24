const test = require('node:test');
const assert = require('node:assert');
const Render = require('./render.js');
const demo = require('./demo-state.json');

test('text from files is escaped', () => {
  assert.strictEqual(Render.esc('<b>"Tom" & \'Jerry\'</b>'), '&lt;b&gt;&quot;Tom&quot; &amp; &#39;Jerry&#39;&lt;/b&gt;');
});

test('gains show their sign', () => {
  assert.strictEqual(Render.signed(1.314, 2), '+1.31');
  assert.strictEqual(Render.signed(-7.8, 2), '-7.80');
  assert.strictEqual(Render.signed(0, 2), '+0.00');
});

test('a held-back song is starred', () => {
  const html = Render.trackRowHtml(demo.groups[0].tracks[1], 'alac');
  assert.match(html, /\+1\.31\*/);
  assert.match(html, /limited/);
});

test('silence says n/a and an unmeasured song shows dashes', () => {
  assert.match(Render.trackRowHtml(demo.groups[0].tracks[3], 'alac'), /n\/a/);
  const pending = Render.trackRowHtml(demo.groups[0].tracks[4], 'alac');
  assert.match(pending, /<td class="num">-<\/td>/);
});

test('a measuring file says so', () => {
  assert.match(Render.groupRowsHtml(demo.groups[1], 'alac'), /measuring/);
});

test('a failure is shown and escaped', () => {
  const group = { ...demo.groups[1], measuring: false, failure: '<gone>' };
  assert.match(Render.groupRowsHtml(group, 'alac'), /! &lt;gone&gt;/);
});

test('status line and run label', () => {
  assert.strictEqual(Render.runLabel(demo), 'Run (4)');
  assert.match(Render.statusText(demo), /measuring 1 of 2/);
  assert.match(Render.statusText(demo), /held back/);
});

test('the selected preset shows when it has been modified', () => {
  const html = Render.presetListHtml({ ...demo.presets, modified: true });
  assert.match(html, /iTunes Lossless <em>modified<\/em>/);
});
