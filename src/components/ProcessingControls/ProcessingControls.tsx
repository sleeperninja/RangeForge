import React, { useState } from "react";
import { ProcessingProfile } from "../../types/audio";
import "./ProcessingControls.css";

interface ProcessingControlsProps {
  queueCount: number;
  autoMode: boolean;
  metadataApi: boolean;
  selectedProfile: ProcessingProfile | null;
  profiles: ProcessingProfile[];
  onRunProcessing: () => void;
  onToggleAutoMode: () => void;
  onToggleMetadataApi: () => void;
  onProfileSelect: (profileId: string) => void;
  onProfileChange: (profile: Partial<ProcessingProfile>) => void;
  onSettingsClick: () => void;
  isProcessing: boolean;
  targetLufs: number;
  onTargetLufsChange: (lufs: number) => void;
}

export const ProcessingControls: React.FC<ProcessingControlsProps> = ({
  queueCount,
  autoMode,
  metadataApi,
  selectedProfile,
  profiles,
  onRunProcessing,
  onToggleAutoMode,
  onToggleMetadataApi,
  onProfileSelect,
  onProfileChange,
  onSettingsClick,
  isProcessing,
  targetLufs,
  onTargetLufsChange,
}) => {
  const [customLocation, setCustomLocation] = useState("");

  const formatOptions = [
    { value: "original", label: "ORIGINAL" },
    { value: "flac", label: "FLAC" },
    { value: "wav", label: "WAV" },
    { value: "alac", label: "ALAC" },
    { value: "aac", label: "AAC" },
    { value: "mp3", label: "MP3" },
  ];

  // Fallback format options (no "original" option)
  const fallbackFormatOptions = [
    { value: "flac", label: "FLAC" },
    { value: "wav", label: "WAV" },
    { value: "alac", label: "ALAC" },
    { value: "aac", label: "AAC" },
    { value: "mp3", label: "MP3" },
  ];

  const sampleRateOptions = [
    { value: "original", label: "ORIGINAL" },
    { value: 44100, label: "44.1 kHz" },
    { value: 48000, label: "48 kHz" },
    { value: 88200, label: "88.2 kHz" },
    { value: 96000, label: "96 kHz" },
    { value: 176400, label: "176.4 kHz" },
    { value: 192000, label: "192 kHz" },
  ];

  const bitDepthOptions = [
    { value: "original", label: "ORIGINAL" },
    { value: 16, label: "16-bit" },
    { value: 24, label: "24-bit" },
    { value: 32, label: "32-bit" },
  ];

  const channelOptions = [
    { value: "original", label: "ORIGINAL" },
    { value: 1, label: "Mono" },
    { value: 2, label: "Stereo" },
    { value: 6, label: "5.1 Surround" },
    { value: 8, label: "7.1 Surround" },
  ];

  return (
    <div className="processing-controls">
      {/* Left Panel: Run Controls */}
      <div className="control-panel left-panel">
        <div className="run-section">
          <button
            className={`run-btn ${isProcessing ? "processing" : ""}`}
            onClick={onRunProcessing}
            disabled={queueCount === 0 || isProcessing}
          >
            {isProcessing ? "Processing..." : "Run"} ({queueCount})
          </button>

          <div className="toggle-group">
            <label className="toggle-item">
              <input
                type="checkbox"
                checked={autoMode}
                onChange={onToggleAutoMode}
                className="toggle-checkbox"
              />
              <span className="toggle-slider"></span>
              Auto Mode
            </label>

            <label className="toggle-item">
              <input
                type="checkbox"
                checked={metadataApi}
                onChange={onToggleMetadataApi}
                className="toggle-checkbox"
              />
              <span className="toggle-slider"></span>
              Metadata API
            </label>
          </div>

          <button className="settings-btn" onClick={onSettingsClick}>
            ⚙️
          </button>
        </div>
      </div>

      {/* Center Panel: Format Controls */}
      <div className="control-panel center-panel">
        <div className="format-section">
          <div className="format-row">
            <div className="format-group">
              <label>Format</label>
              <select
                value={selectedProfile?.outputFormat || "original"}
                onChange={(e) =>
                  onProfileChange({ outputFormat: e.target.value as any })
                }
                className="format-select"
              >
                {formatOptions.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            </div>

            <div className="format-group">
              <label>Fallback Format</label>
              <select className="format-select fallback">
                {fallbackFormatOptions.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            </div>
          </div>

          <div className="format-row">
            <div className="format-group">
              <label>Sample Rate</label>
              <select
                value={selectedProfile?.sampleRate || "original"}
                onChange={(e) =>
                  onProfileChange({
                    sampleRate:
                      e.target.value === "original"
                        ? "original"
                        : Number(e.target.value),
                  })
                }
                className="format-select"
              >
                {sampleRateOptions.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            </div>

            <div className="format-group">
              <label>Fallback Sample Rate</label>
              <select className="format-select fallback">
                <option value="88200">88.2 kHz</option>
                <option value="96000">96 kHz</option>
                <option value="48000">48 kHz</option>
              </select>
            </div>
          </div>

          <div className="format-row">
            <div className="format-group">
              <label>Bit Depth</label>
              <select
                value={selectedProfile?.bitDepth || "original"}
                onChange={(e) =>
                  onProfileChange({
                    bitDepth:
                      e.target.value === "original"
                        ? "original"
                        : Number(e.target.value),
                  })
                }
                className="format-select"
              >
                {bitDepthOptions.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            </div>

            <div className="format-group">
              <label>Fallback Bit Depth</label>
              <select className="format-select fallback">
                <option value="24">24-bit</option>
                <option value="16">16-bit</option>
                <option value="32">32-bit</option>
              </select>
            </div>
          </div>

          <div className="format-row">
            <div className="format-group">
              <label>Channels</label>
              <select
                value={selectedProfile?.channels || "original"}
                onChange={(e) =>
                  onProfileChange({
                    channels:
                      e.target.value === "original"
                        ? "original"
                        : Number(e.target.value),
                  })
                }
                className="format-select"
              >
                {channelOptions.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            </div>

            <div className="format-group">
              <label>Fallback Channels</label>
              <select className="format-select fallback">
                <option value="6">5.1 Surround</option>
                <option value="2">Stereo</option>
                <option value="8">7.1 Surround</option>
              </select>
            </div>
          </div>
        </div>
      </div>

      {/* Right Panel: Volume & Output Controls */}
      <div className="control-panel right-panel">
        <div className="volume-section">
          <div className="volume-adjustment">
            <label>Volume Adjustment</label>
            <div className="lufs-input">
              <span>Target Loudness</span>
              <input
                type="number"
                value={targetLufs}
                onChange={(e) => onTargetLufsChange(Number(e.target.value))}
                step="0.1"
                min="-60"
                max="0"
                className="lufs-number"
              />
              <span>LUFS</span>
            </div>
          </div>

          <div className="peak-protection">
            <label>
              <input
                type="checkbox"
                checked={
                  selectedProfile ? selectedProfile.peakProtection < 0 : true
                }
                onChange={(e) =>
                  onProfileChange({ peakProtection: e.target.checked ? -1 : 0 })
                }
              />
              Peak Protection
            </label>
            <div className="dbtp-input">
              <input
                type="number"
                value={selectedProfile?.peakProtection || 0}
                onChange={(e) =>
                  onProfileChange({ peakProtection: Number(e.target.value) })
                }
                step="0.1"
                min="-6"
                max="0"
                className="dbtp-number"
              />
              <span>dBTP</span>
            </div>
          </div>

          <div className="file-strategy">
            <label>File Strategy</label>
            <div className="strategy-options">
              <label className="radio-option">
                <input
                  type="radio"
                  name="fileStrategy"
                  value="serialize"
                  checked={selectedProfile?.fileStrategy === "serialize"}
                  onChange={(e) =>
                    onProfileChange({ fileStrategy: e.target.value as any })
                  }
                />
                Serialize new filename
                <input
                  type="text"
                  placeholder="00"
                  className="serialize-input"
                />
              </label>

              <label className="radio-option">
                <input
                  type="radio"
                  name="fileStrategy"
                  value="rename"
                  checked={selectedProfile?.fileStrategy === "rename"}
                  onChange={(e) =>
                    onProfileChange({ fileStrategy: e.target.value as any })
                  }
                />
                Rename old file
                <input
                  type="text"
                  placeholder="_old"
                  className="rename-input"
                />
              </label>

              <label className="radio-option">
                <input
                  type="radio"
                  name="fileStrategy"
                  value="replace"
                  checked={selectedProfile?.fileStrategy === "replace"}
                  onChange={(e) =>
                    onProfileChange({ fileStrategy: e.target.value as any })
                  }
                />
                Replace file
              </label>

              <label className="radio-option">
                <input
                  type="radio"
                  name="fileStrategy"
                  value="location"
                  checked={selectedProfile?.fileStrategy === "location"}
                  onChange={(e) =>
                    onProfileChange({ fileStrategy: e.target.value as any })
                  }
                />
                Save to location
                <div className="location-input">
                  <input
                    type="text"
                    value={customLocation}
                    onChange={(e) => setCustomLocation(e.target.value)}
                    placeholder="userFolder\\Downloads\\NewAudio"
                    className="location-path"
                  />
                  <button className="browse-btn">Browse</button>
                </div>
              </label>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
