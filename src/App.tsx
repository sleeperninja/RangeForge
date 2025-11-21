import React, { useState, useCallback } from "react";
import "./App.css";
import { AudioAnalysisTable } from "./components/AudioAnalysisTable";
import { ProcessingControls } from "./components/ProcessingControls";
import { AudioTrack, ProcessingProfile, GenreTag } from "./types/audio";

function App() {
  const [tracks, setTracks] = useState<AudioTrack[]>([]);
  const [selectedTrackIds, setSelectedTrackIds] = useState<string[]>([]);
  const [autoMode, setAutoMode] = useState(false);
  const [metadataApi, setMetadataApi] = useState(true);
  const [isProcessing, setIsProcessing] = useState(false);

  // Default profile
  const [selectedProfile, setSelectedProfile] = useState<ProcessingProfile>({
    id: "default",
    name: "Default",
    targetLufs: -22,
    peakProtection: -1.0,
    outputFormat: "flac",
    sampleRate: "original",
    bitDepth: "original",
    channels: "original",
    fileStrategy: "serialize",
  });

  const profiles: ProcessingProfile[] = [selectedProfile];

  const handleTrackSelect = useCallback((trackId: string) => {
    setSelectedTrackIds((prev) =>
      prev.includes(trackId)
        ? prev.filter((id) => id !== trackId)
        : [...prev, trackId]
    );
  }, []);

  const handleRemoveTrack = useCallback((trackId: string) => {
    setTracks((prev) => prev.filter((track) => track.id !== trackId));
    setSelectedTrackIds((prev) => prev.filter((id) => id !== trackId));
  }, []);

  const handleRunProcessing = useCallback(() => {
    setIsProcessing(true);
    // TODO: Implement actual processing
    console.log("Starting processing for tracks:", selectedTrackIds);

    // Simulate processing
    setTimeout(() => {
      setIsProcessing(false);
    }, 3000);
  }, [selectedTrackIds]);

  const handleToggleAutoMode = useCallback(() => {
    setAutoMode((prev) => !prev);
  }, []);

  const handleToggleMetadataApi = useCallback(() => {
    setMetadataApi((prev) => !prev);
  }, []);

  const handleProfileSelect = useCallback((profileId: string) => {
    // TODO: Implement profile selection
    console.log("Selected profile:", profileId);
  }, []);

  const handleProfileChange = useCallback(
    (changes: Partial<ProcessingProfile>) => {
      setSelectedProfile((prev) => ({ ...prev, ...changes }));
    },
    []
  );

  const handleSettingsClick = useCallback(() => {
    // TODO: Open settings dialog
    console.log("Settings clicked");
  }, []);

  // File handling functions
  const handleFilesLoaded = useCallback((newTracks: AudioTrack[]) => {
    setTracks((prev) => [...prev, ...newTracks]);
  }, []);

  const handleFileError = useCallback((error: string) => {
    console.error("File loading error:", error);
    // TODO: Show user-friendly error message
  }, []);

  // Simple file drop functionality
  const handleFilesDrop = useCallback(
    (files: FileList) => {
      const validTracks = Array.from(files)
        .map((file, index) => {
          const fileName = file.name;
          const fileExt = fileName.split(".").pop()?.toLowerCase() || "";

          // Basic codec detection from extension
          const codecMap: Record<string, string> = {
            flac: "FLAC",
            wav: "WAV",
            mp3: "MP3",
            m4a: "ALAC/AAC",
            aac: "AAC",
            ogg: "OGG Vorbis",
            opus: "Opus",
            ape: "Monkey's Audio",
            wv: "WavPack",
          };

          const codec = codecMap[fileExt];
          if (!codec) return null;

          // Create basic genre tags
          const tags: GenreTag[] = [
            {
              name: fileExt.toUpperCase(),
              color: "blue",
              source: "metadata",
            },
          ];

          const track: AudioTrack = {
            id: `file_${Date.now()}_${index}`,
            name: fileName,
            path: fileName, // In Electron this would be file.path
            codec,
            fileSize: file.size,
            sampleRate: undefined,
            bitDepth: undefined,
            channels: undefined,
            duration: undefined,
            analysis: undefined,
            tags,
            isProcessing: false,
            error: undefined,
            isCueTrack: false,
          };

          return track;
        })
        .filter((track): track is AudioTrack => track !== null);

      if (validTracks.length > 0) {
        handleFilesLoaded(validTracks);
      } else {
        handleFileError("No supported audio files found");
      }
    },
    [handleFilesLoaded, handleFileError]
  );

  return (
    <div className="App">
      <header className="App-header">
        <h1>RangeForge</h1>
        <p>Swiss Army Knife for Lossless Audio Processing</p>
      </header>

      <main className="App-main">
        {tracks.length === 0 && (
          <div
            className="file-drop-zone"
            onDragOver={(e) => e.preventDefault()}
            onDrop={(e) => {
              e.preventDefault();
              if (e.dataTransfer.files) {
                handleFilesDrop(e.dataTransfer.files);
              }
            }}
          >
            <div className="drop-zone-content">
              <div className="drop-zone-icon">📁</div>
              <div className="drop-zone-text">
                <h3>Drop Audio Files Here</h3>
                <p>
                  Drag & drop audio files to get started
                  <br />
                  <span className="supported-formats">
                    Supports: FLAC, WAV, MP3, M4A, AAC, OGG, Opus, APE, WavPack
                  </span>
                </p>
              </div>
            </div>
          </div>
        )}

        <div
          className="analysis-section"
          onDragOver={(e) => e.preventDefault()}
          onDrop={(e) => {
            e.preventDefault();
            if (e.dataTransfer.files) {
              handleFilesDrop(e.dataTransfer.files);
            }
          }}
        >
          <AudioAnalysisTable
            tracks={tracks}
            onTrackSelect={handleTrackSelect}
            selectedTrackIds={selectedTrackIds}
            onRemoveTrack={handleRemoveTrack}
          />
        </div>

        <div className="controls-section">
          <ProcessingControls
            queueCount={selectedTrackIds.length || tracks.length}
            autoMode={autoMode}
            metadataApi={metadataApi}
            selectedProfile={selectedProfile}
            profiles={profiles}
            onRunProcessing={handleRunProcessing}
            onToggleAutoMode={handleToggleAutoMode}
            onToggleMetadataApi={handleToggleMetadataApi}
            onProfileSelect={handleProfileSelect}
            onProfileChange={handleProfileChange}
            onSettingsClick={handleSettingsClick}
            isProcessing={isProcessing}
          />
        </div>
      </main>
    </div>
  );
}

export default App;
