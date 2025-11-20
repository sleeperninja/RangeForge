import React, { useState, useCallback } from "react";
import "./App.css";
import { AudioAnalysisTable } from "./components/AudioAnalysisTable";
import { ProcessingControls } from "./components/ProcessingControls";
import { AudioTrack, ProcessingProfile } from "./types/audio";

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

  return (
    <div className="App">
      <header className="App-header">
        <h1>RangeForge</h1>
        <p>Swiss Army Knife for Lossless Audio Processing</p>
      </header>

      <main className="App-main">
        <div className="analysis-section">
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
