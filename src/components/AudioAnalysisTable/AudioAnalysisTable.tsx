import React from "react";
import { AudioTrack, GenreTag } from "../../types/audio";
import "./AudioAnalysisTable.css";

interface AudioAnalysisTableProps {
  tracks: AudioTrack[];
  onTrackSelect: (trackId: string) => void;
  selectedTrackIds: string[];
  onRemoveTrack: (trackId: string) => void;
}

export const AudioAnalysisTable: React.FC<AudioAnalysisTableProps> = ({
  tracks,
  onTrackSelect,
  selectedTrackIds,
  onRemoveTrack,
}) => {
  const formatLufs = (lufs?: number): string => {
    if (lufs === undefined) return "--";
    return `${lufs.toFixed(1)} LUFS`;
  };

  const formatDbtp = (dbtp?: number): string => {
    if (dbtp === undefined) return "--";
    return `${dbtp.toFixed(1)} dBTP`;
  };

  const formatDr = (dr?: number): string => {
    if (dr === undefined) return "--";
    return `DR${dr}`;
  };

  const formatAdjustment = (adjustmentDb?: number): string => {
    if (adjustmentDb === undefined) return "--";
    const sign = adjustmentDb >= 0 ? "+" : "";
    return `${sign}${adjustmentDb.toFixed(1)}`;
  };

  const getGenreTagColor = (tag: GenreTag): string => {
    const colorMap: Record<GenreTag["color"], string> = {
      blue: "#3b82f6",
      purple: "#8b5cf6",
      green: "#10b981",
      orange: "#f59e0b",
      red: "#ef4444",
      yellow: "#eab308",
    };
    return colorMap[tag.color];
  };

  const getLufsColorClass = (lufs?: number): string => {
    if (lufs === undefined) return "";
    if (lufs > -14) return "lufs-loud";
    if (lufs < -30) return "lufs-quiet";
    return "lufs-normal";
  };

  const getDbdtpColorClass = (dbtp?: number): string => {
    if (dbtp === undefined) return "";
    if (dbtp > -0.1) return "dbtp-hot";
    if (dbtp < -3) return "dbtp-safe";
    return "dbtp-normal";
  };

  return (
    <div className="audio-analysis-table">
      <div className="table-header">
        <div className="header-cell name">Name</div>
        <div className="header-cell codec">Codec</div>
        <div className="header-cell loudness">Loudness (LUFS)</div>
        <div className="header-cell truepeak">True Peak (dBTP)</div>
        <div className="header-cell adjust">Adjust (LUFS)</div>
        <div className="header-cell range">Range (DR)</div>
        <div className="header-cell tags">Tags</div>
        <div className="header-cell actions">Actions</div>
      </div>

      <div className="table-body">
        {tracks.map((track) => (
          <div
            key={track.id}
            className={`table-row ${
              selectedTrackIds.includes(track.id) ? "selected" : ""
            } ${track.isProcessing ? "processing" : ""}`}
            onClick={() => onTrackSelect(track.id)}
          >
            <div className="cell name" title={track.name}>
              {track.name}
            </div>

            <div className="cell codec">{track.codec.toUpperCase()}</div>

            <div
              className={`cell loudness ${getLufsColorClass(
                track.analysis?.loudness.lufs
              )}`}
            >
              {formatLufs(track.analysis?.loudness.lufs)}
            </div>

            <div
              className={`cell truepeak ${getDbdtpColorClass(
                track.analysis?.truePeak.dbtp
              )}`}
            >
              {formatDbtp(track.analysis?.truePeak.dbtp)}
            </div>

            <div className="cell adjust">
              {formatAdjustment(track.analysis?.adjustment.adjustmentDb)}
            </div>

            <div className="cell range">
              {formatDr(track.analysis?.dynamicRange.dr)}
            </div>

            <div className="cell tags">
              <div className="tag-container">
                {track.tags.map((tag, index) => (
                  <span
                    key={index}
                    className="genre-tag"
                    style={{ backgroundColor: getGenreTagColor(tag) }}
                    title={`${tag.name} (${tag.source})`}
                  >
                    {tag.name}
                  </span>
                ))}
              </div>
            </div>

            <div className="cell actions">
              <button
                className="remove-btn"
                onClick={(e) => {
                  e.stopPropagation();
                  onRemoveTrack(track.id);
                }}
                disabled={track.isProcessing}
                title="Remove track"
              >
                ✕
              </button>
            </div>
          </div>
        ))}
      </div>

      {tracks.length === 0 && (
        <div className="empty-state">
          <p>Drop audio files or albums with CUE sheets to analyze</p>
        </div>
      )}
    </div>
  );
};
