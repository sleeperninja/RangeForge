const test = require('node:test');
const assert = require('node:assert');
const Render = require('./render.js');
const demo = require('./demo-state.json');

const [houses, coda, single] = demo.groups;

test('text from files is escaped', () => {
  assert.strictEqual(Render.esc('<b>"Tom" & \'Jerry\'</b>'), '&lt;b&gt;&quot;Tom&quot; &amp; &#39;Jerry&#39;&lt;/b&gt;');
});

test('gains show their sign', () => {
  assert.strictEqual(Render.signed(1.314, 2), '+1.31');
  assert.strictEqual(Render.signed(-7.8, 2), '-7.80');
  assert.strictEqual(Render.signed(0, 2), '+0.00');
});

test('a held-back song is starred', () => {
  const html = Render.trackRowHtml(houses.tracks[1], 'alac', 0, 1);
  assert.match(html, /\+1\.31\*/);
  assert.match(html, /limited/);
});

test('silence says n/a and an unmeasured song shows dashes', () => {
  assert.match(Render.trackRowHtml(coda.tracks[2], 'alac', 1, 2), /n\/a/);
  const pending = Render.trackRowHtml(coda.tracks[3], 'alac', 1, 3);
  assert.match(pending, /<td class="num">-<\/td>/);
});

test('a measuring file says so', () => {
  assert.match(Render.groupRowsHtml(single, 'alac', 2), /measuring/);
});

test('a failure is shown and escaped', () => {
  const group = { ...single, measuring: false, failure: '<gone>' };
  assert.match(Render.groupRowsHtml(group, 'alac', 2), /! &lt;gone&gt;/);
});

test('rows carry their place in the list so actions can find them', () => {
  const html = Render.trackRowHtml(coda.tracks[0], 'alac', 1, 0);
  assert.match(html, /<tr class="track" data-group="1" data-track="0">/);
  assert.match(html, /<input type="checkbox" class="include" data-group="1" data-track="0" checked/);
  assert.match(Render.groupRowsHtml(coda, 'alac', 1), /<tr class="group cue" data-group="1">/);
});

test('a song left out is dimmed and unticked', () => {
  const html = Render.trackRowHtml(coda.tracks[1], 'alac', 1, 1);
  assert.match(html, /class="track excluded"/);
  assert.doesNotMatch(html, /include" data-group="1" data-track="1" checked/);
});

test('an album box shows whether all, some or none of its songs are in', () => {
  assert.strictEqual(Render.includedState(houses), 'all');
  assert.strictEqual(Render.includedState(coda), 'some');
  const none = { tracks: [{ included: false }] };
  assert.strictEqual(Render.includedState(none), 'none');
  assert.match(Render.groupRowsHtml(coda, 'alac', 1), /data-state="some"/);
  assert.match(Render.groupRowsHtml(houses, 'alac', 0), /data-state="all" checked/);
});

test('the right-click menu depends on the row and what is switched on', () => {
  const song = Render.menuItems(demo, 1, 1);
  assert.deepStrictEqual(
    song.map((item) => item.label),
    [
      'Include this song',
      'Include every song of this album',
      'Get tags from Last.fm for this song',
      'Get tags from Last.fm for this album',
      'Remove from the list',
    ],
  );
  assert.deepStrictEqual(song[0], { action: 'toggle_track', label: 'Include this song', group: 1, track: 1, on: true });

  const album = Render.menuItems(demo, 0, null);
  assert.deepStrictEqual(
    album.map((item) => item.label),
    ['Leave out every song of this album', 'Get tags from Last.fm for this album', 'Remove from the list'],
  );
  assert.strictEqual(album[0].on, false);

  assert.deepStrictEqual(Render.menuItems(demo, 99, null), []);
  assert.match(Render.menuHtml(album), /data-item="2">Remove from the list/);
});

test('the confirmation says what will happen', () => {
  assert.strictEqual(Render.confirmText({ conflict: 'number', delete_original: false }), '');
  assert.match(Render.confirmText({ conflict: 'overwrite', delete_original: false }), /overwritten/);
  const both = Render.confirmText({ conflict: 'overwrite', delete_original: true });
  assert.match(both, /overwritten/);
  assert.match(both, /original is deleted/);
});

test('the variable list names every variable and what it means', () => {
  const html = Render.variablesHtml(demo.naming.variables);
  assert.match(html, /<code>\{title\}<\/code> song title<\/li>/);
  assert.match(html, /<code>\{name\}<\/code> the original file/);
  assert.strictEqual((html.match(/<li>/g) || []).length, 8);
});

test('hovering a song says who it is and how that was worked out', () => {
  const cue = { artist: 'Led Zeppelin', title: 'No Quarter', album: 'Houses of the Holy', source: 'cue' };
  assert.strictEqual(
    Render.identityTitle(cue),
    'Led Zeppelin – No Quarter (Houses of the Holy) · from the CUE sheet',
  );
  assert.match(Render.identityTitle({ ...cue, source: 'tags' }), /from the file's tags$/);
  assert.match(Render.identityTitle({ ...cue, source: 'name' }), /a guess from the file's name/);
  // No album: no empty brackets. Nothing known: nothing said.
  assert.doesNotMatch(Render.identityTitle({ ...cue, album: null }), /\(/);
  assert.strictEqual(Render.identityTitle(null), '');
  assert.strictEqual(Render.identityTitle({ artist: null, title: null, source: 'name' }), '');
  // It lands on the row, escaped.
  const row = Render.trackRowHtml({ ...houses.tracks[0], identity: { ...cue, title: '<b>' } }, 'alac', 0, 0);
  assert.match(row, /<td class="name" title="Led Zeppelin – &lt;b&gt;/);
});

test('a Radio tag shows as a blue chip with its explanation, and tag text is escaped', () => {
  const html = Render.trackRowHtml(houses.tracks[1], 'alac', 0, 1);
  assert.match(html, /<span class="chip radio" title="Reached #14 on the Billboard Hot 100 \(1973\)">Radio<\/span><\/td>/);
  // A song with no tags, or from older data with no tags field, has an empty cell.
  assert.match(Render.trackRowHtml(coda.tracks[0], 'alac', 1, 0), /<td class="tags"><\/td>/);
  const odd = Render.chipsHtml([{ text: '<b>', kind: 'radio', title: '"x"' }]);
  assert.match(odd, /&lt;b&gt;/);
  assert.match(odd, /title="&quot;x&quot;"/);
});

test('album genres, song genres and Radio are told apart by colour class and keep their order', () => {
  const html = Render.trackRowHtml(houses.tracks[1], 'alac', 0, 1);
  const kinds = [...html.matchAll(/class="chip (\w+)"/g)].map((m) => m[1]);
  assert.deepStrictEqual(kinds, ['genre', 'genre', 'song', 'radio']);
  const texts = [...html.matchAll(/class="chip \w+" title="[^"]*">([^<]+)</g)].map((m) => m[1]);
  assert.deepStrictEqual(texts, ['Classic Rock', 'Hard Rock', 'Ballad', 'Radio']);
});

test('the Settings window says what is known about the Last.fm key, without showing it', () => {
  assert.match(Render.lastfmStatusText({ lastfm_key: 'saved' }), /A key is saved/);
  assert.match(Render.lastfmStatusText({ lastfm_key: 'environment' }), /LASTFM_API/);
  assert.match(Render.lastfmStatusText({ lastfm_key: 'none' }), /No key yet/);
  for (const key of ['saved', 'environment', 'none']) {
    assert.doesNotMatch(Render.lastfmStatusText({ lastfm_key: key }), /\w{24,}/);
  }
});

test('status line and run label', () => {
  assert.strictEqual(Render.runLabel(demo), 'Run (5)');
  assert.match(Render.statusText(demo), /measuring 1 of 3/);
  assert.match(Render.statusText(demo), /held back/);
});

test('the selected preset shows when it has been modified', () => {
  const html = Render.presetListHtml({ ...demo.presets, modified: true });
  assert.match(html, /iTunes Lossless <em>modified<\/em>/);
});
