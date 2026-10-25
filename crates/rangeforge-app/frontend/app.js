// The glue between the page and the program: show the state, and send the user's actions.
// All the rules live in the program; nothing here decides what is allowed.
(function () {
  'use strict';

  const tauri = window.__TAURI__;
  const inTauri = Boolean(tauri && tauri.core);
  const $ = (id) => document.getElementById(id);
  let state = null;
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
    $('rows').innerHTML = Render.tableBodyHtml(s);
    for (const box of document.querySelectorAll('input.include[data-state="some"]')) {
      box.indeterminate = true;
    }
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
      dialog.addEventListener(
        'close',
        () => {
          if (dialog.returnValue === 'run') {
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
      const args = { group: Number(box.dataset.group), on: box.checked };
      args.track = box.dataset.track === undefined ? null : Number(box.dataset.track);
      call('set_included', args);
    });

    // Right-click a row for the same actions plus removing it.
    $('rows').addEventListener('contextmenu', (e) => {
      const row = e.target.closest('tr[data-group]');
      if (!row || !state) {
        return;
      }
      e.preventDefault();
      const track = row.dataset.track === undefined ? null : Number(row.dataset.track);
      openMenu(e.clientX, e.clientY, Render.menuItems(state, Number(row.dataset.group), track));
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
