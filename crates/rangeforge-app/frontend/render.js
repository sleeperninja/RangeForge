// Turns the screen state into HTML. These are plain functions with no access to the page, so
// they can be tested with Node (render.test.js) as well as used by app.js.
(function (root, factory) {
  if (typeof module === 'object' && module.exports) {
    module.exports = factory();
  } else {
    root.Render = factory();
  }
})(typeof self !== 'undefined' ? self : this, function () {
  'use strict';

  const ESCAPES = { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' };

  // File names and tags come from the user's files, so everything shown is escaped.
  function esc(text) {
    return String(text).replace(/[&<>"']/g, (c) => ESCAPES[c]);
  }

  function fixed(value, digits) {
    return value === null || value === undefined ? '-' : value.toFixed(digits);
  }

  // A gain always shows its sign, so "+0.40" and "-7.80" line up and read as changes.
  function signed(value, digits) {
    const text = value.toFixed(digits);
    return value >= 0 && !text.startsWith('-') ? '+' + text : text;
  }

  function adjustCell(track) {
    if (track.unadjustable) {
      return '<td class="num adjust">n/a</td>';
    }
    if (track.adjust_db === null || track.adjust_db === undefined) {
      return '<td class="num adjust">-</td>';
    }
    // `peak` says what the new peak does; older data only has `limited`.
    const peak = track.peak !== undefined ? track.peak : track.limited ? 'limited' : '';
    const titles = {
      kept: 'Above 0 dB, and kept: 32-bit float WAV holds it',
      limited: 'Peak protection held this song back from the target',
      clips: 'Peak protection is off: this song will clip',
    };
    const star = peak ? '*' : '';
    const cls = peak ? `num adjust peak-${peak}` : 'num adjust';
    const title = peak ? ` title="${titles[peak]}"` : '';
    return `<td class="${cls}"${title}>${signed(track.adjust_db, 2)}${star}</td>`;
  }

  // The labels from outside lookups in the Tags column. Each kind has its own colour.
  // With the song's place in the list given, each has a × that removes it.
  function chipsHtml(tags, groupIndex, trackIndex) {
    const removable = groupIndex !== undefined && trackIndex !== undefined;
    return (tags || [])
      .map((tag) => {
        const x = removable
          ? `<button type="button" class="chip-x" data-group="${groupIndex}" data-track="${trackIndex}" data-tag="${esc(tag.text)}" title="Remove this tag">×</button>`
          : '';
        return `<span class="chip ${esc(tag.kind)}" title="${esc(tag.title)}">${esc(tag.text)}${x}</span>`;
      })
      .join('');
  }

  // What to say when the pointer rests on a song: who it is taken to be, and how that was
  // worked out. A guess from the file name is called one, since it can be wrong.
  // What a song's row says: the name it will be written as (without the extension) when the file
  // name pattern changes it, otherwise its own name. A name resting on a guess is in italics.
  function nameHtml(track) {
    if (!track.output_name) {
      return esc(track.label);
    }
    const text = esc(track.output_name);
    return track.output_guessed ? `<span class="guess">${text}</span>` : text;
  }

  // The hover text: where the new name comes from and the original name, then who the song is.
  function nameTitle(track) {
    const who = identityTitle(track.identity);
    if (!track.output_name) {
      return who;
    }
    const guess = track.output_guessed ? ' (made from a guess at the names)' : '';
    return [`Will be saved as: ${track.output_name}${guess}`, `Original: ${track.label}`, who].filter(Boolean).join('\n');
  }

  function identityTitle(identity) {
    if (!identity || (!identity.artist && !identity.title)) {
      return '';
    }
    const who = [identity.artist, identity.title].filter(Boolean).join(' – ');
    const album = identity.album ? ` (${identity.album})` : '';
    const how = {
      hand: 'typed in by you',
      cue: 'from the CUE sheet',
      tags: "from the file's tags",
      name: "a guess from the file's name and folders",
    }[identity.source];
    return `${who}${album} · ${how}`;
  }

  // What the Settings window says about the Last.fm key. The key itself is never shown.
  function lastfmStatusText(metadata) {
    switch (metadata.lastfm_key) {
      case 'saved':
        return 'A key is saved. Type a new one and press Save key to replace it.';
      case 'environment':
        return 'Using the key from the LASTFM_API setting of this computer. Save one here to use that instead.';
      default:
        return 'No key yet. You can get a free one at last.fm/api.';
    }
  }

  function trackRowHtml(track, outputCodec, groupIndex, trackIndex) {
    const mark = track.saved ? '✓' : '↳';
    const cls = ['track', track.saved ? 'saved' : '', track.included ? '' : 'excluded']
      .filter(Boolean)
      .join(' ');
    const checked = track.included ? ' checked' : '';
    return (
      `<tr class="${cls}" data-group="${groupIndex}" data-track="${trackIndex}">` +
      `<td class="name" title="${esc(nameTitle(track))}"><input type="checkbox" class="include" data-group="${groupIndex}" data-track="${trackIndex}"${checked} title="Include this song in the run">` +
      `<span class="mark">${mark}</span>${nameHtml(track)}</td>` +
      `<td class="codec">${esc(outputCodec)}</td>` +
      `<td class="num">${fixed(track.loudness_lufs, 2)}</td>` +
      `<td class="num">${fixed(track.true_peak_dbtp, 2)}</td>` +
      adjustCell(track) +
      `<td class="num">${fixed(track.range_lu, 1)}</td>` +
      `<td class="tags">${chipsHtml(track.tags, groupIndex, trackIndex)}</td>` +
      '</tr>'
    );
  }

  // What the album row says where the numbers will go: measuring, or how far writing has got.
  function progressCell(group) {
    if (group.run_state && group.run_state !== 'idle' && group.run_state !== 'failed') {
      return `<td class="num status ${esc(group.run_state)}">${esc(group.run_text)}</td>`;
    }
    if (group.measuring) {
      return '<td class="num status measuring">measuring…</td>';
    }
    return '<td class="num status"></td>';
  }

  // Whether every, some or none of a group's songs are in the run.
  function includedState(group) {
    const on = group.tracks.filter((track) => track.included).length;
    if (on === 0) {
      return 'none';
    }
    return on === group.tracks.length ? 'all' : 'some';
  }

  // `heading` is false for songs drawn under an album heading, which stands in for the
  // per-file row.
  function groupRowsHtml(group, outputCodec, groupIndex, heading = true) {
    let html = '';
    if (heading) {
      const included = includedState(group);
      const checked = included === 'all' ? ' checked' : '';
      html =
        `<tr class="group ${esc(group.kind)}" data-group="${groupIndex}">` +
        `<td class="name"><input type="checkbox" class="include" data-group="${groupIndex}" data-state="${included}"${checked} title="Include every song of this album in the run">${esc(group.name)}</td>` +
        `<td class="codec">${esc(group.codec_label)}</td>` +
        progressCell(group) +
        '<td class="num"></td><td class="num"></td><td class="num"></td><td class="tags"></td>' +
        '</tr>';
    }
    if (group.failure) {
      html += `<tr class="message error"><td colspan="7">! ${esc(group.failure)}</td></tr>`;
    }
    if (group.run_failure) {
      html += `<tr class="message error"><td colspan="7">! could not write: ${esc(group.run_failure)}</td></tr>`;
    }
    group.tracks.forEach((track, trackIndex) => {
      html += trackRowHtml(track, outputCodec, groupIndex, trackIndex);
    });
    return html;
  }

  // Which songs belong together under one album heading: single files, already scanned and
  // identified, with the same album, artist and folder. The folder keeps a stereo and a
  // six-channel copy of one album apart. `null` for anything that stands alone.
  function albumKey(group) {
    if (group.kind !== 'single' || group.measuring || group.tracks.length !== 1) {
      return null;
    }
    const identity = group.tracks[0].identity;
    if (!identity || !identity.album || !identity.album.trim()) {
      return null;
    }
    const who = identity.album_artist || identity.artist || '';
    return [identity.album, who, group.folder || ''].map((part) => part.trim().toLowerCase()).join('|');
  }

  // The rows of the table, in order: a lone group, or an album with its groups (every group in
  // it holds one file). An album needs at least two files; each is placed where its first
  // file was.
  function blocksOf(state) {
    const blocks = [];
    const byKey = new Map();
    state.groups.forEach((group, index) => {
      const key = albumKey(group);
      if (key === null) {
        blocks.push({ key: null, groups: [index] });
      } else if (byKey.has(key)) {
        byKey.get(key).groups.push(index);
      } else {
        const block = { key, groups: [index] };
        byKey.set(key, block);
        blocks.push(block);
      }
    });
    return blocks.map((block) => (block.key !== null && block.groups.length < 2 ? { key: null, groups: block.groups } : block));
  }

  // Every song of some groups, as targets. A target with a song names exactly that song.
  function songsOfGroups(state, groupIndexes) {
    return groupIndexes.flatMap((group) =>
      state.groups[group] ? state.groups[group].tracks.map((_, track) => ({ group, track })) : [],
    );
  }

  // Every song of the album a group belongs to, as targets: the files gathered under one album
  // heading together, or a lone group's own songs. The album dialog works on all of them
  // whichever song or row it was opened from.
  function albumScope(state, groupIndex) {
    const block = blocksOf(state).find((b) => b.groups.includes(groupIndex));
    return songsOfGroups(state, block ? block.groups : [groupIndex]);
  }

  // What the heading's progress cell says about all of the album's files.
  function blockProgress(groups) {
    const states = groups.map((group) => group.run_state || 'idle');
    if (states.every((s) => s === 'saved')) {
      return '<td class="num status saved">saved</td>';
    }
    if (states.includes('encoding')) {
      return '<td class="num status encoding">encoding</td>';
    }
    if (states.includes('queued')) {
      return '<td class="num status queued">queued</td>';
    }
    const done = states.filter((s) => s === 'saved').length;
    return done > 0 ? `<td class="num status saved">${done}/${states.length} saved</td>` : '<td class="num status"></td>';
  }

  // What an album heading's codec cell says: the format the files are read as, in brackets like
  // a single file's row (the songs under it show what they will be written as). Files in
  // different formats say so.
  function sourceLabel(groups) {
    const labels = [...new Set(groups.map((group) => group.codec_label))];
    return labels.length === 1 ? labels[0] : '(mixed)';
  }

  // An album heading and, unless folded away, the songs under it.
  function blockRowsHtml(state, block, collapsed) {
    const groups = block.groups.map((index) => state.groups[index]);
    const identity = groups[0].tracks[0].identity;
    const who = identity.album_artist || identity.artist;
    const name = who ? `${who} – ${identity.album}` : identity.album;
    const songs = { tracks: groups.flatMap((group) => group.tracks) };
    const included = includedState(songs);
    const checked = included === 'all' ? ' checked' : '';
    const folded = collapsed.has(block.key);
    const where = groups[0].folder ? ` title="${esc(groups[0].folder)}"` : '';
    let html =
      `<tr class="group album" data-groups="${block.groups.join(',')}" data-key="${esc(block.key)}">` +
      `<td class="name"${where}><button type="button" class="fold" title="${folded ? 'Show' : 'Hide'} the songs">${folded ? '▸' : '▾'}</button>` +
      `<input type="checkbox" class="include" data-groups="${block.groups.join(',')}" data-state="${included}"${checked} title="Include every song of this album in the run">` +
      `${esc(name)} <span class="count">(${songs.tracks.length} songs)</span></td>` +
      `<td class="codec">${esc(sourceLabel(groups))}</td>` +
      blockProgress(groups) +
      '<td class="num"></td><td class="num"></td><td class="num"></td><td class="tags"></td>' +
      '</tr>';
    if (!folded) {
      block.groups.forEach((index) => {
        const group = state.groups[index];
        html += groupRowsHtml(group, group.output_codec || state.output.codec, index, false);
      });
    }
    return html;
  }

  function tableBodyHtml(state, collapsed = new Set()) {
    return blocksOf(state)
      .map((block) => {
        if (block.key !== null) {
          return blockRowsHtml(state, block, collapsed);
        }
        const index = block.groups[0];
        const group = state.groups[index];
        return groupRowsHtml(group, group.output_codec || state.output.codec, index);
      })
      .join('');
  }

  // Every song with the same album name as this one (ignoring capitals and spaces), anywhere in
  // the list, as `{group, track}` pairs. Empty when the song has no album name.
  function sameAlbum(state, groupIndex, trackIndex) {
    const album = (state.groups[groupIndex]?.tracks[trackIndex]?.identity?.album || '').trim().toLowerCase();
    if (!album) {
      return [];
    }
    const picks = [];
    state.groups.forEach((group, g) => {
      group.tracks.forEach((track, t) => {
        if (((track.identity && track.identity.album) || '').trim().toLowerCase() === album) {
          picks.push({ group: g, track: t });
        }
      });
    });
    return picks;
  }

  // The one item for naming an album and its songs: the artist, album and album artist, and each
  // song's title, from the album's track list or typed. Always for the whole album.
  function albumNames(state, groupIndex) {
    return { action: 'album_names', label: 'Album & track names…', targets: albumScope(state, groupIndex) };
  }

  // What the right-click menu offers for a row. `trackIndex` is null on an album's own row.
  // `picks` is the selected songs as `{group, track}` pairs when several are selected and the
  // row is one of them: the menu then works on all of them. Items that work on songs carry
  // their `targets`; a target with no song means every song of that album.
  function menuItems(state, groupIndex, trackIndex, picks) {
    const group = state.groups[groupIndex];
    if (!group) {
      return [];
    }
    const song = trackIndex !== null && group.tracks[trackIndex];
    const items = [];
    const selectSame = { action: 'select_album', label: 'Select every song with this album', group: groupIndex, track: trackIndex };

    if (song && picks && picks.length > 1) {
      const n = picks.length;
      const everyOn = picks.every((p) => state.groups[p.group].tracks[p.track].included);
      items.push({
        action: 'include_many',
        label: everyOn ? `Leave out these ${n} songs` : `Include these ${n} songs`,
        targets: picks,
        on: !everyOn,
      });
      items.push({ action: 'add_tag', label: `Add a tag to these ${n} songs…`, targets: picks });
      items.push(albumNames(state, groupIndex));
      items.push({ action: 'open_song_page', label: `Open song tags pages on Last.fm for these ${n} songs`, targets: picks });
      items.push({ action: 'lastfm', label: 'Get tags from Last.fm for their albums', targets: picks });
      items.push(selectSame);
      return items;
    }

    const here = [{ group: groupIndex, track: song ? trackIndex : null }];
    if (song) {
      const on = group.tracks[trackIndex].included;
      items.push({
        action: 'toggle_track',
        label: on ? 'Leave this song out' : 'Include this song',
        group: groupIndex,
        track: trackIndex,
        on: !on,
      });
    }
    const all = includedState(group) === 'all';
    items.push({
      action: 'toggle_group',
      label: all ? 'Leave out every song of this album' : 'Include every song of this album',
      group: groupIndex,
      on: !all,
    });
    items.push({
      action: 'add_tag',
      label: song ? 'Add a tag to this song…' : 'Add a tag to every song of this album…',
      targets: here,
    });
    items.push(albumNames(state, groupIndex));
    if (song) {
      items.push({ action: 'open_song_page', label: 'Open song tags page on Last.fm', targets: here });
    }
    items.push({
      action: 'lastfm',
      label: 'Get tags from Last.fm for this album',
      targets: [{ group: groupIndex, track: null }],
    });
    if (song) {
      items.push(selectSame);
    }
    items.push({ action: 'remove', label: 'Remove from the list', group: groupIndex });
    return items;
  }

  // The menu of an album heading: it works on every file gathered under it. A target with no
  // song means every song of that file.
  function blockMenuItems(state, groupIndexes) {
    const groups = groupIndexes.map((index) => state.groups[index]).filter(Boolean);
    if (groups.length === 0) {
      return [];
    }
    const targets = groupIndexes.map((group) => ({ group, track: null }));
    const everyOn = includedState({ tracks: groups.flatMap((group) => group.tracks) }) === 'all';
    return [
      {
        action: 'include_many',
        label: everyOn ? 'Leave out every song of this album' : 'Include every song of this album',
        targets,
        on: !everyOn,
      },
      { action: 'add_tag', label: 'Add a tag to every song of this album…', targets },
      { action: 'album_names', label: 'Album & track names…', targets: songsOfGroups(state, groupIndexes) },
      { action: 'lastfm', label: 'Get tags from Last.fm for this album', targets },
      { action: 'remove_many', label: 'Remove this album from the list', groups: groupIndexes },
    ];
  }

  function menuHtml(items) {
    return items
      .map((item, index) => `<button type="button" role="menuitem" data-item="${index}">${esc(item.label)}</button>`)
      .join('');
  }

  // The variables a file name pattern can use, for the Settings window.
  function variablesHtml(variables) {
    return variables.map((v) => `<li><code>${esc(v.key)}</code> ${esc(v.meaning)}</li>`).join('');
  }

  // What to tell the user before a run that overwrites files or deletes originals.
  function confirmText(strategy) {
    const parts = [];
    if (strategy.conflict === 'overwrite') {
      parts.push('A file already at a new file’s path will be overwritten.');
    }
    if (strategy.delete_original) {
      parts.push(
        'Each original is deleted once every song from it has been written. An original is kept if any of its songs was left out.',
      );
    }
    return parts.join(' ');
  }

  // The line under the Run button.
  function statusText(state) {
    let text;
    if (state.running) {
      text = 'writing files…';
    } else if (state.total_files === 0) {
      text = 'no files yet';
    } else if (state.pending_files > 0) {
      text = `measuring ${state.pending_files} of ${state.total_files} file(s)…`;
    } else {
      text = 'all measured';
    }
    if (state.any_limited) {
      text += ' · * held back by peak protection';
    }
    return text;
  }

  function runLabel(state) {
    return `Run (${state.queued_songs})`;
  }

  function presetListHtml(presets) {
    return presets.names
      .map((name, index) => {
        const selected = presets.selected === index;
        const note = selected && presets.modified ? ' <em>modified</em>' : '';
        const cls = selected ? ' class="selected"' : '';
        return `<li role="option" data-index="${index}" aria-selected="${selected}"${cls}>${esc(name)}${note}</li>`;
      })
      .join('');
  }

  // The most recent lines are enough for the panel, and keep a long session light.
  function logHtml(lines) {
    return lines
      .slice(-500)
      .map((line) => `<li>- ${esc(line)}</li>`)
      .join('');
  }

  function optionsHtml(choices, selectedId) {
    return choices
      .map((c) => `<option value="${esc(c.id)}"${c.id === selectedId ? ' selected' : ''}>${esc(c.label)}</option>`)
      .join('');
  }

  return {
    esc,
    fixed,
    signed,
    chipsHtml,
    identityTitle,
    lastfmStatusText,
    trackRowHtml,
    groupRowsHtml,
    tableBodyHtml,
    includedState,
    menuItems,
    albumScope,
    sameAlbum,
    blocksOf,
    blockMenuItems,
    menuHtml,
    variablesHtml,
    confirmText,
    statusText,
    runLabel,
    presetListHtml,
    logHtml,
    optionsHtml,
  };
});
