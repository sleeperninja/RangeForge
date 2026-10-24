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
    const key = choices.map((c) => c.id).join('|');
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
    $('empty').classList.toggle('hidden', s.groups.length > 0);

    const run = $('run');
    run.textContent = Render.runLabel(s);
    run.disabled = Boolean(s.run_blocker);
    run.title = s.run_blocker || 'Write the finished files';

    fillSelect($('codec'), s.options.codecs, s.output.codec);
    fillSelect($('bitDepth'), s.options.bit_depths, s.output.bit_depth);
    $('container').value = s.output.container;

    setValue($('target'), s.volume.target_lufs.toFixed(1));
    $('peakProtection').checked = s.volume.peak_protection;
    setValue($('ceiling'), s.volume.ceiling_dbtp.toFixed(1));
    $('ceiling').disabled = !s.volume.peak_protection;

    $('presetList').innerHTML = Render.presetListHtml(s.presets);
    $('savePreset').disabled = !s.presets.can_save;
    $('deletePreset').disabled = !s.presets.can_delete;

    for (const radio of document.querySelectorAll('input[name="strategy"]')) {
      radio.checked = radio.value === s.strategy.kind;
    }
    setValue($('serialSuffix'), s.strategy.serial_suffix);
    setValue($('oldSuffix'), s.strategy.old_suffix);
    setValue($('folder'), s.strategy.folder);
    $('serialSuffix').disabled = s.strategy.kind !== 'serialize';
    $('oldSuffix').disabled = s.strategy.kind !== 'rename_old';
    $('folder').disabled = s.strategy.kind !== 'save_to';
    $('browse').disabled = s.strategy.kind !== 'save_to';
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

  async function pressRun(confirmed) {
    const reply = await call('run', { confirmed });
    if (reply && reply.outcome === 'needs_confirmation') {
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

    $('presetList').addEventListener('click', (e) => {
      const item = e.target.closest('li[data-index]');
      if (item) {
        call('select_preset', { index: Number(item.dataset.index) });
      }
    });
    $('savePreset').addEventListener('click', askForPresetName);
    $('deletePreset').addEventListener('click', () => call('delete_preset'));

    for (const radio of document.querySelectorAll('input[name="strategy"]')) {
      radio.addEventListener('change', (e) => call('set_strategy', { id: e.target.value }));
    }
    for (const id of ['serialSuffix', 'oldSuffix', 'folder']) {
      $(id).addEventListener('change', (e) => call('set_strategy_text', { text: e.target.value }));
    }
    $('browse').addEventListener('click', async () => {
      const folder = await pickFolder();
      if (folder) {
        await call('set_strategy_text', { text: folder });
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
