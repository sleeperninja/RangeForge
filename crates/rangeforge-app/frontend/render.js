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
    const star = track.limited ? '*' : '';
    const cls = track.limited ? 'num adjust limited' : 'num adjust';
    const title = track.limited ? ' title="Peak protection held this song back from the target"' : '';
    return `<td class="${cls}"${title}>${signed(track.adjust_db, 2)}${star}</td>`;
  }

  function trackRowHtml(track, outputCodec) {
    const mark = track.saved ? '✓' : '↳';
    const cls = track.saved ? 'track saved' : 'track';
    return (
      `<tr class="${cls}">` +
      `<td class="name"><span class="mark">${mark}</span> ${esc(track.label)}</td>` +
      `<td class="codec">${esc(outputCodec)}</td>` +
      `<td class="num">${fixed(track.loudness_lufs, 2)}</td>` +
      `<td class="num">${fixed(track.true_peak_dbtp, 2)}</td>` +
      adjustCell(track) +
      `<td class="num">${fixed(track.range_lu, 1)}</td>` +
      '<td class="tags"></td>' +
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

  function groupRowsHtml(group, outputCodec) {
    let html =
      `<tr class="group ${esc(group.kind)}">` +
      `<td class="name">${esc(group.name)}</td>` +
      `<td class="codec">${esc(group.codec_label)}</td>` +
      progressCell(group) +
      '<td class="num"></td><td class="num"></td><td class="num"></td><td class="tags"></td>' +
      '</tr>';
    if (group.failure) {
      html += `<tr class="message error"><td colspan="7">! ${esc(group.failure)}</td></tr>`;
    }
    if (group.run_failure) {
      html += `<tr class="message error"><td colspan="7">! could not write: ${esc(group.run_failure)}</td></tr>`;
    }
    for (const track of group.tracks) {
      html += trackRowHtml(track, outputCodec);
    }
    return html;
  }

  function tableBodyHtml(state) {
    return state.groups.map((group) => groupRowsHtml(group, state.output.codec)).join('');
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
    trackRowHtml,
    groupRowsHtml,
    tableBodyHtml,
    statusText,
    runLabel,
    presetListHtml,
    logHtml,
    optionsHtml,
  };
});
