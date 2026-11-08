// The glue between the page and the program: show the state, and send the user's actions.
// All the rules live in the program; nothing here decides what is allowed.
(function () {
  'use strict';

  const tauri = window.__TAURI__;
  const inTauri = Boolean(tauri && tauri.core);
  const $ = (id) => document.getElementById(id);
  let state = null;

  // The songs picked in the table, as 'group:song' keys, and where a Shift-click range starts.
  let selection = new Set();
  let anchor = null;
  let selectionGroups = 0;
  const pickKey = (group, track) => `${group}:${track}`;

  function selectedPicks() {
    return [...selection]
      .map((key) => {
        const [group, track] = key.split(':').map(Number);
        return { group, track };
      })
      .sort((a, b) => a.group - b.group || a.track - b.track);
  }

  // Shows which songs are picked. A changed list (an album added or removed) clears the pick,
  // since the numbers no longer mean the same songs.
  function applySelection() {
    if (state && state.groups.length !== selectionGroups) {
      selection = new Set();
      anchor = null;
      selectionGroups = state.groups.length;
    }
    for (const row of document.querySelectorAll('#rows tr.track')) {
      row.classList.toggle('selected', selection.has(pickKey(row.dataset.group, row.dataset.track)));
    }
  }

  // Album headings: which album is folded, and the files ("1,4,5") an album stands for.
  const collapsed = new Set();
  const blockIndexes = (text) => text.split(',').map(Number);
  const blockTargets = (text) => blockIndexes(text).map((group) => ({ group, track: null }));
  const blockKeys = (text) =>
    blockIndexes(text).flatMap((group) => state.groups[group].tracks.map((_, t) => pickKey(group, t)));

  function setSelection(keys, newAnchor) {
    selection = new Set(keys);
    anchor = newAnchor === undefined ? null : newAnchor;
    applySelection();
  }
  let toastTimer = null;

  const AUDIO_EXTENSIONS = ['flac', 'wav', 'm4a', 'mp3', 'ogg', 'aac', 'aiff', 'cue'];

  function toast(message) {
    const box = $('toast');
    box.textContent = message;
    box.classList.remove('hidden');
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => box.classList.add('hidden'), 6000);
  }

  // Sends an action to the program. Every reply carries the new screen, and a message if
  // something was refused.
  async function call(name, args) {
    if (!inTauri) {
      toast('Demo mode: this page is not connected to the program.');
      return null;
    }
    try {
      const reply = await tauri.core.invoke(name, args || {});
      take(reply);
      return reply;
    } catch (error) {
      toast(String(error));
      return null;
    }
  }

  function take(reply) {
    if (reply.error) {
      toast(reply.error);
    }
    if (reply.message) {
      toast(reply.message);
    }
    if (reply.state) {
      state = reply.state;
      draw();
    }
  }

  // Sets a field unless the user is typing in it, so a refresh never fights the keyboard.
  function setValue(input, value) {
    const text = String(value);
    if (document.activeElement !== input && input.value !== text) {
      input.value = text;
    }
  }

  function fillSelect(select, choices, selectedId) {
    // The labels count too: the container's name follows the codec while its id stays the same.
    const key = choices.map((c) => `${c.id}:${c.label}`).join('|');
    if (select.dataset.key !== key) {
      select.innerHTML = Render.optionsHtml(choices, selectedId);
      select.dataset.key = key;
    }
    select.value = selectedId;
  }

  function draw() {
    if (!state) {
      return;
    }
    const s = state;
    $('rows').innerHTML = Render.tableBodyHtml(s, collapsed);
    for (const box of document.querySelectorAll('input.include[data-state="some"]')) {
      box.indeterminate = true;
    }
    applySelection();
    $('empty').classList.toggle('hidden', s.groups.length > 0);
    closeMenu();

    const run = $('run');
    run.textContent = Render.runLabel(s);
    run.disabled = Boolean(s.run_blocker);
    run.title = s.run_blocker || 'Write the finished files';

    fillSelect($('codec'), s.options.codecs, s.output.codec);
    fillSelect($('bitDepth'), s.options.bit_depths, s.output.bit_depth);
    fillSelect($('container'), s.options.containers, s.output.container);
    fillSelect($('sampleRate'), s.options.sample_rates, s.output.sample_rate);
    fillSelect($('dsdCodec'), s.options.dsd_codecs, s.dsd.codec);
    fillSelect($('dsdContainer'), s.options.dsd_containers, s.dsd.container);
    fillSelect($('dsdSampleRate'), s.options.dsd_sample_rates, s.dsd.sample_rate);
    fillSelect($('dsdBitDepth'), s.options.dsd_bit_depths, s.dsd.bit_depth);
    $('confirmFirst').checked = s.strategy.confirm_first;

    setValue($('target'), s.volume.target_lufs.toFixed(1));
    $('peakProtection').checked = s.volume.peak_protection;
    setValue($('ceiling'), s.volume.ceiling_dbtp.toFixed(1));
    $('ceiling').disabled = !s.volume.peak_protection;

    $('presetList').innerHTML = Render.presetListHtml(s.presets);
    $('savePreset').disabled = !s.presets.can_save;
    $('deletePreset').disabled = !s.presets.can_delete;

    for (const radio of document.querySelectorAll('input[name="place"]')) {
      radio.checked = radio.value === s.strategy.place;
    }
    for (const radio of document.querySelectorAll('input[name="conflict"]')) {
      radio.checked = radio.value === s.strategy.conflict;
    }
    setValue($('subfolder'), s.strategy.subfolder);
    setValue($('folder'), s.strategy.folder);
    setValue($('numberSuffix'), s.strategy.number_suffix);
    setValue($('oldSuffix'), s.strategy.old_suffix);
    $('deleteOriginal').checked = s.strategy.delete_original;
    $('metadataToggle').checked = s.metadata.enabled;
    $('lastfmEnabled').checked = s.metadata.lastfm_enabled;
    $('lastfmStatus').textContent = Render.lastfmStatusText(s.metadata);
    $('lastfmForget').disabled = s.metadata.lastfm_key !== 'saved';
    const problem = $('strategyProblem');
    problem.textContent = s.strategy.problem || '';
    problem.classList.toggle('hidden', !s.strategy.problem);

    $('logLines').innerHTML = Render.logHtml(s.log);
    const logBox = $('logLines');
    logBox.scrollTop = logBox.scrollHeight;

    document.title = 'RangeForge';
    $('run').dataset.status = Render.statusText(s);
    $('statusLine').textContent = Render.statusText(s);
  }

  async function pickFiles() {
    if (!inTauri) {
      toast('Demo mode: the file picker needs the program.');
      return;
    }
    const picked = await tauri.dialog.open({
      multiple: true,
      directory: false,
      filters: [{ name: 'Audio and CUE sheets', extensions: AUDIO_EXTENSIONS }],
    });
    if (picked) {
      await call('add_paths', { paths: Array.isArray(picked) ? picked : [picked] });
    }
  }

  async function pickFolder() {
    if (!inTauri) {
      toast('Demo mode: the folder picker needs the program.');
      return null;
    }
    const picked = await tauri.dialog.open({ multiple: false, directory: true });
    return picked || null;
  }

  let currentMenu = null;

  function openMenu(x, y, items) {
    if (items.length === 0) {
      return;
    }
    currentMenu = items;
    const menu = $('menu');
    menu.innerHTML = Render.menuHtml(items);
    menu.classList.remove('hidden');
    // Keep the menu inside the window.
    menu.style.left = Math.max(0, Math.min(x, window.innerWidth - menu.offsetWidth - 4)) + 'px';
    menu.style.top = Math.max(0, Math.min(y, window.innerHeight - menu.offsetHeight - 4)) + 'px';
  }

  function closeMenu() {
    currentMenu = null;
    $('menu').classList.add('hidden');
  }

  function runMenuItem(item) {
    closeMenu();
    if (item.action === 'toggle_track') {
      call('set_included', { group: item.group, track: item.track, on: item.on });
    } else if (item.action === 'toggle_group') {
      call('set_included', { group: item.group, track: null, on: item.on });
    } else if (item.action === 'include_many') {
      includeMany(item.targets, item.on);
    } else if (item.action === 'add_tag') {
      askForTag(item.targets);
    } else if (item.action === 'edit_names') {
      askForNames(item.targets);
    } else if (item.action === 'lastfm') {
      call('get_lastfm_tags', { targets: item.targets, albums: Boolean(item.albums) });
    } else if (item.action === 'select_album') {
      const same = Render.sameAlbum(state, item.group, item.track);
      if (same.length === 0) {
        toast('This song has no album name yet. Use Edit artist, title and album to give it one.');
      } else {
        setSelection(same.map((p) => pickKey(p.group, p.track)), pickKey(same[0].group, same[0].track));
        toast(`${same.length} song${same.length === 1 ? '' : 's'} selected. Right-click one to work on them all.`);
      }
    } else if (item.action === 'remove_many') {
      removeMany(item.groups);
    } else if (item.action === 'remove') {
      call('remove_group', { group: item.group });
    }
  }

  async function pressRun(confirmed) {
    const reply = await call('run', { confirmed });
    if (reply && reply.outcome === 'needs_confirmation') {
      $('confirmText').textContent = Render.confirmText(state.strategy);
      const dialog = $('confirmDialog');
      dialog.returnValue = '';
      $('confirmNoAsk').checked = false;
      dialog.addEventListener(
        'close',
        async () => {
          if (dialog.returnValue === 'run') {
            // Only "Run anyway" can turn the question off, never Cancel.
            if ($('confirmNoAsk').checked) {
              await call('set_confirm_destructive', { on: false });
            }
            pressRun(true);
          }
        },
        { once: true },
      );
      dialog.showModal();
    }
  }

  function showPreview(preview) {
    const show = (which, example, problem) => {
      $('example' + which).textContent = example || '';
      const box = $('problem' + which);
      box.textContent = problem || '';
      box.classList.toggle('hidden', !problem);
    };
    show('Album', preview.album_example, preview.album_problem);
    show('Single', preview.single_example, preview.single_problem);
    $('settingsSave').disabled = Boolean(preview.album_problem || preview.single_problem);
  }

  let previewTimer = null;

  // Asks the program what the patterns would give, so the example follows the typing.
  function updatePreview() {
    clearTimeout(previewTimer);
    previewTimer = setTimeout(async () => {
      if (!inTauri) {
        return;
      }
      try {
        showPreview(
          await tauri.core.invoke('preview_naming', {
            album: $('nameAlbum').value,
            single: $('nameSingle').value,
          }),
        );
      } catch (error) {
        toast(String(error));
      }
    }, 120);
  }

  function openSettings() {
    if (!state) {
      return;
    }
    $('nameAlbum').value = state.naming.album;
    $('nameSingle').value = state.naming.single;
    $('variableList').innerHTML = Render.variablesHtml(state.naming.variables);
    $('lastfmKey').value = '';
    showPreview(state.naming.example);
    $('settingsDialog').showModal();
  }

  async function saveSettings() {
    const reply = await call('set_naming', {
      album: $('nameAlbum').value,
      single: $('nameSingle').value,
    });
    // A refusal stays on screen as the toast; the dialog stays open to be fixed.
    if (!reply || !reply.error) {
      $('settingsDialog').close();
    }
  }

  function askForPresetName() {
    const dialog = $('presetDialog');
    const selected = state.presets.selected;
    $('presetName').value = selected === null ? '' : state.presets.names[selected];
    dialog.returnValue = '';
    dialog.addEventListener(
      'close',
      () => {
        if (dialog.returnValue === 'save') {
          call('save_preset', { name: $('presetName').value });
        }
      },
      { once: true },
    );
    dialog.showModal();
    $('presetName').select();
  }

  // Switches several songs in or out of the run, one after the other.
  async function includeMany(targets, on) {
    for (const t of targets) {
      await call('set_included', { group: t.group, track: t.track, on });
    }
  }

  // Takes several files off the list, last first so the numbers of the others stay right.
  async function removeMany(groups) {
    for (const group of [...groups].sort((a, b) => b - a)) {
      await call('remove_group', { group });
    }
  }

  // The songs a list of targets stands for (a target with no song means the whole album).
  function songsOf(targets) {
    return targets.flatMap((t) => {
      const tracks = state.groups[t.group] ? state.groups[t.group].tracks : [];
      return t.track === null ? tracks : tracks.slice(t.track, t.track + 1);
    });
  }

  // Asks for a tag to add to the songs the targets stand for.
  function askForTag(targets) {
    const dialog = $('tagDialog');
    const n = songsOf(targets).length;
    const whole = targets.length === 1 && targets[0].track === null;
    $('tagPrompt').textContent = whole
      ? 'Add a tag to every song of this album:'
      : n === 1
        ? 'Add a tag to this song:'
        : `Add a tag to these ${n} songs:`;
    $('tagText').value = '';
    dialog.returnValue = '';
    dialog.addEventListener(
      'close',
      () => {
        if (dialog.returnValue === 'save') {
          call('add_tag', { targets, text: $('tagText').value });
        }
      },
      { once: true },
    );
    dialog.showModal();
    $('tagText').focus();
  }

  // The value every song agrees on, or an empty string if they differ.
  function shared(values) {
    const first = values[0] || '';
    return values.every((v) => (v || '') === first) ? first : '';
  }

  // Asks for who songs are, or what an album is called when the targets are a whole album.
  // With several songs, fields left empty keep what each song already has.
  function askForNames(targets) {
    const songs = songsOf(targets);
    const identities = songs.map((s) => s.identity || {});
    const whole = targets.every((t) => t.track === null);
    const several = !whole && songs.length > 1;
    $('namesPrompt').textContent = whole
      ? 'The album’s names, for every song in it:'
      : several
        ? `Names for these ${songs.length} songs. A field left empty keeps what each song has.`
        : 'Who this song is:';
    $('namesArtistRow').classList.toggle('hidden', whole);
    $('namesTitleRow').classList.toggle('hidden', whole);
    $('namesArtist').value = shared(identities.map((i) => i.artist));
    $('namesTitle').value = shared(identities.map((i) => i.title));
    $('namesAlbum').value = shared(identities.map((i) => i.album));
    $('namesAlbumArtist').value = shared(identities.map((i) => i.album_artist));
    const dialog = $('namesDialog');
    dialog.returnValue = '';
    dialog.addEventListener(
      'close',
      () => {
        if (dialog.returnValue === 'save') {
          call('set_names', {
            targets,
            // Several songs, or several files under one album heading: an empty field leaves
            // what each has rather than wiping it.
            onlyFilled: several || (whole && targets.length > 1),
            names: {
              artist: $('namesArtist').value,
              title: $('namesTitle').value,
              album: $('namesAlbum').value,
              albumArtist: $('namesAlbumArtist').value,
            },
          });
        }
      },
      { once: true },
    );
    dialog.showModal();
    (whole ? $('namesAlbum') : $('namesArtist')).select();
  }

  function wire() {
    $('run').addEventListener('click', () => pressRun(false));
    $('addFiles').addEventListener('click', pickFiles);
    $('addFolder').addEventListener('click', async () => {
      const folder = await pickFolder();
      if (folder) {
        await call('add_paths', { paths: [folder] });
      }
    });
    $('showLogs').addEventListener('change', (e) => {
      $('logPanel').classList.toggle('hidden', !e.target.checked);
    });

    $('target').addEventListener('change', (e) => call('set_target', { value: Number(e.target.value) }));
    $('ceiling').addEventListener('change', (e) => call('set_ceiling', { value: Number(e.target.value) }));
    $('peakProtection').addEventListener('change', (e) => call('set_peak_protection', { on: e.target.checked }));
    $('codec').addEventListener('change', (e) => call('set_codec', { id: e.target.value }));
    $('bitDepth').addEventListener('change', (e) => call('set_bit_depth', { id: e.target.value }));
    $('container').addEventListener('change', (e) => call('set_container', { id: e.target.value }));
    for (const [id, field] of [
      ['dsdCodec', 'codec'],
      ['dsdContainer', 'container'],
      ['dsdSampleRate', 'sample_rate'],
      ['dsdBitDepth', 'bit_depth'],
    ]) {
      $(id).addEventListener('change', (e) => call('set_dsd_format', { field, id: e.target.value }));
    }
    $('confirmFirst').addEventListener('change', (e) =>
      call('set_confirm_destructive', { on: e.target.checked }),
    );
    $('sampleRate').addEventListener('change', (e) => call('set_sample_rate', { id: e.target.value }));

    $('presetList').addEventListener('click', (e) => {
      const item = e.target.closest('li[data-index]');
      if (item) {
        call('select_preset', { index: Number(item.dataset.index) });
      }
    });
    $('savePreset').addEventListener('click', askForPresetName);
    $('deletePreset').addEventListener('click', () => call('delete_preset'));

    for (const radio of document.querySelectorAll('input[name="place"]')) {
      radio.addEventListener('change', (e) => call('set_place', { id: e.target.value }));
    }
    for (const radio of document.querySelectorAll('input[name="conflict"]')) {
      radio.addEventListener('change', (e) => call('set_conflict', { id: e.target.value }));
    }
    // Typing in a field chooses its row first, because the program keeps one text for the
    // chosen row.
    const textFields = [
      ['subfolder', 'set_place', 'set_place_text', 'subfolder'],
      ['folder', 'set_place', 'set_place_text', 'folder'],
      ['numberSuffix', 'set_conflict', 'set_conflict_text', 'number'],
      ['oldSuffix', 'set_conflict', 'set_conflict_text', 'rename_existing'],
    ];
    for (const [id, choose, set, choice] of textFields) {
      $(id).addEventListener('change', async (e) => {
        await call(choose, { id: choice });
        await call(set, { text: e.target.value });
      });
    }
    $('deleteOriginal').addEventListener('change', (e) => call('set_delete_original', { on: e.target.checked }));
    $('browse').addEventListener('click', async () => {
      const folder = await pickFolder();
      if (folder) {
        await call('set_place', { id: 'folder' });
        await call('set_place_text', { text: folder });
      }
    });

    $('metadataToggle').addEventListener('change', (e) => call('set_metadata', { on: e.target.checked }));
    $('lastfmEnabled').addEventListener('change', (e) => call('set_lastfm_enabled', { on: e.target.checked }));
    $('lastfmSave').addEventListener('click', async () => {
      const key = $('lastfmKey').value.trim();
      if (!key) {
        toast('Type the key first, or press Forget to remove the saved one.');
        return;
      }
      const reply = await call('set_lastfm_key', { key });
      // Once it is handed over, the key does not stay in the page.
      if (reply && !reply.error) {
        $('lastfmKey').value = '';
      }
    });
    $('lastfmForget').addEventListener('click', async () => {
      $('lastfmKey').value = '';
      await call('set_lastfm_key', { key: '' });
    });
    $('lastfmClear').addEventListener('click', async () => {
      const reply = await call('clear_lastfm_cache');
      if (reply && !reply.error) {
        toast('Saved Last.fm answers cleared. Genres will be pulled fresh.');
      }
    });
    $('settingsButton').addEventListener('click', openSettings);
    $('settingsCancel').addEventListener('click', () => $('settingsDialog').close());
    $('settingsSave').addEventListener('click', saveSettings);
    $('nameAlbum').addEventListener('input', updatePreview);
    $('nameSingle').addEventListener('input', updatePreview);

    $('clearCompleted').addEventListener('click', () => call('clear_completed'));
    $('clearAll').addEventListener('click', () => call('clear_list'));

    // Tick boxes switch songs (or whole albums) in or out of the run.
    $('rows').addEventListener('change', (e) => {
      const box = e.target.closest('input.include');
      if (!box) {
        return;
      }
      if (box.dataset.groups !== undefined) {
        // An album heading: every file under it.
        includeMany(blockTargets(box.dataset.groups), box.checked);
        return;
      }
      const args = { group: Number(box.dataset.group), on: box.checked };
      args.track = box.dataset.track === undefined ? null : Number(box.dataset.track);
      call('set_included', args);
    });

    // An album heading's ▾ folds its songs away, and a click on it picks all of them.
    $('rows').addEventListener('click', (e) => {
      const row = e.target.closest('tr[data-groups]');
      if (!row || !state) {
        return;
      }
      if (e.target.closest('button.fold')) {
        const key = row.dataset.key;
        if (collapsed.has(key)) {
          collapsed.delete(key);
        } else {
          collapsed.add(key);
        }
        draw();
        return;
      }
      if (e.target.closest('input, button')) {
        return;
      }
      const keys = blockKeys(row.dataset.groups);
      if (e.ctrlKey || e.metaKey) {
        const all = keys.every((k) => selection.has(k));
        const next = new Set(selection);
        keys.forEach((k) => (all ? next.delete(k) : next.add(k)));
        setSelection(next, keys[0]);
      } else {
        setSelection(keys, keys[0]);
      }
    });

    // The × on a tag takes it off that song.
    $('rows').addEventListener('click', (e) => {
      const x = e.target.closest('button.chip-x');
      if (x) {
        call('remove_tag', {
          group: Number(x.dataset.group),
          track: Number(x.dataset.track),
          text: x.dataset.tag,
        });
      }
    });

    // Click picks a song; Ctrl-click adds or drops one; Shift-click picks a range. Clicking an
    // album's row picks all of its songs.
    $('rows').addEventListener('click', (e) => {
      const row = e.target.closest('tr[data-group]');
      if (!row || !state || e.target.closest('input, button')) {
        return;
      }
      const group = Number(row.dataset.group);
      const song = row.dataset.track === undefined ? null : Number(row.dataset.track);
      const keys =
        song === null
          ? state.groups[group].tracks.map((_, t) => pickKey(group, t))
          : [pickKey(group, song)];
      if (e.shiftKey && anchor && song !== null) {
        const order = [...document.querySelectorAll('#rows tr.track')].map((r) =>
          pickKey(r.dataset.group, r.dataset.track),
        );
        const [from, to] = [order.indexOf(anchor), order.indexOf(keys[0])].sort((a, b) => a - b);
        setSelection(from < 0 ? keys : order.slice(from, to + 1), anchor);
      } else if (e.ctrlKey || e.metaKey) {
        const all = keys.every((k) => selection.has(k));
        const next = new Set(selection);
        keys.forEach((k) => (all ? next.delete(k) : next.add(k)));
        setSelection(next, keys[0]);
      } else {
        setSelection(keys, keys[0]);
      }
    });

    // Right-click a row for actions on it. On one of several picked songs, they work on all of
    // them; on any other row, that row is picked first.
    $('rows').addEventListener('contextmenu', (e) => {
      const heading = e.target.closest('tr[data-groups]');
      if (heading && state) {
        e.preventDefault();
        setSelection(blockKeys(heading.dataset.groups));
        openMenu(e.clientX, e.clientY, Render.blockMenuItems(state, blockIndexes(heading.dataset.groups)));
        return;
      }
      const row = e.target.closest('tr[data-group]');
      if (!row || !state) {
        return;
      }
      e.preventDefault();
      const group = Number(row.dataset.group);
      const song = row.dataset.track === undefined ? null : Number(row.dataset.track);
      let picks;
      if (song === null) {
        setSelection(state.groups[group].tracks.map((_, t) => pickKey(group, t)));
      } else {
        if (!selection.has(pickKey(group, song))) {
          setSelection([pickKey(group, song)], pickKey(group, song));
        }
        picks = selectedPicks();
      }
      openMenu(e.clientX, e.clientY, Render.menuItems(state, group, song, picks));
    });
    $('menu').addEventListener('click', (e) => {
      const button = e.target.closest('button[data-item]');
      if (button && currentMenu) {
        runMenuItem(currentMenu[Number(button.dataset.item)]);
      }
    });
    document.addEventListener('click', (e) => {
      if (!e.target.closest('#menu')) {
        closeMenu();
      }
    });
    document.addEventListener('keydown', (e) => {
      if (e.key === 'Escape') {
        closeMenu();
      }
    });

    document.addEventListener('keydown', (e) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'o') {
        e.preventDefault();
        pickFiles();
      }
    });
  }

  async function start() {
    wire();
    if (!inTauri) {
      // Outside the program (for example in a browser while designing the page), show sample
      // data so the layout can be seen.
      try {
        const response = await fetch('demo-state.json');
        state = await response.json();
        draw();
        $('dropOverlay').classList.add('hidden');
      } catch (error) {
        toast('Demo data could not be loaded.');
      }
      return;
    }

    await tauri.event.listen('state', (event) => take(event.payload));
    await tauri.event.listen('tauri://drag-enter', () => $('dropOverlay').classList.remove('hidden'));
    await tauri.event.listen('tauri://drag-leave', () => $('dropOverlay').classList.add('hidden'));
    await tauri.event.listen('tauri://drag-drop', async (event) => {
      $('dropOverlay').classList.add('hidden');
      const paths = (event.payload && event.payload.paths) || [];
      if (paths.length > 0) {
        await call('add_paths', { paths });
      }
    });
    await call('get_state');
  }

  start();
})();
