import React, { useState, useCallback, useEffect } from "react";
import "./App.css";
import { AudioAnalysisTable } from "./components/AudioAnalysisTable";
import { ProcessingControls } from "./components/ProcessingControls";
import { AudioTrack, ProcessingProfile, GenreTag } from "./types/audio";

function App() {
  const [tracks, setTracks] = useState<AudioTrack[]>([]);
  const [selectedTrackIds, setSelectedTrackIds] = useState<string[]>([]);
  const [autoMode, setAutoMode] = useState(false);
  const [metadataApi, setMetadataApi] = useState(false);
  const [isProcessing, setIsProcessing] = useState(false);
  const [showLogWindow, setShowLogWindow] = useState(false);
  const [processingLogs, setProcessingLogs] = useState<string[]>([]);
  const [targetLufs, setTargetLufs] = useState(-18); // Default to -18 LUFS

  // Default profile
  const [selectedProfile, setSelectedProfile] = useState<ProcessingProfile>({
    id: "default",
    name: "Default",
    targetLufs: -18,
    peakProtection: 0.0,
    outputFormat: "flac",
    sampleRate: "original",
    bitDepth: "original",
    channels: "original",
    fileStrategy: "serialize",
  });

  // Load settings on startup
  useEffect(() => {
    const savedSettings = localStorage.getItem("rangeforge-settings");
    if (savedSettings) {
      try {
        const settings = JSON.parse(savedSettings);
        setTargetLufs(settings.targetLufs || -18);
        setAutoMode(settings.autoMode || false);
        setMetadataApi(settings.metadataApi || false);
        setSelectedProfile((prev) => ({ ...prev, ...settings.profile }));
      } catch (error) {
        console.log("Error loading settings:", error);
      }
    }
  }, []);

  // Save settings when they change
  useEffect(() => {
    const settings = {
      targetLufs,
      autoMode,
      metadataApi,
      profile: selectedProfile,
    };
    localStorage.setItem("rangeforge-settings", JSON.stringify(settings));
  }, [targetLufs, autoMode, metadataApi, selectedProfile]);

  // Sync targetLufs with profile
  useEffect(() => {
    setSelectedProfile((prev) => ({ ...prev, targetLufs: targetLufs }));
  }, [targetLufs]);

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
    setShowLogWindow(true);

    const selectedTracks =
      selectedTrackIds.length > 0
        ? tracks.filter((track) => selectedTrackIds.includes(track.id))
        : tracks;

    const logMessage = `Starting processing for ${selectedTracks.length} track(s)...`;
    setProcessingLogs((prev) => [
      ...prev,
      `[${new Date().toLocaleTimeString()}] ${logMessage}`,
    ]);

    console.log(
      "Starting processing for tracks:",
      selectedTracks.map((t) => t.name)
    );

    // Simulate processing with visual feedback
    selectedTracks.forEach((track, index) => {
      setTimeout(() => {
        const msg = `Processing: ${track.name}`;
        setProcessingLogs((prev) => [
          ...prev,
          `[${new Date().toLocaleTimeString()}] ${msg}`,
        ]);
      }, (index + 1) * 500);
    });

    setTimeout(() => {
      setIsProcessing(false);
      setProcessingLogs((prev) => [
        ...prev,
        `[${new Date().toLocaleTimeString()}] Processing complete!`,
      ]);
    }, (selectedTracks.length + 1) * 500);
  }, [selectedTrackIds, tracks]);

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
    setShowLogWindow(true);

    // Log file loading
    const loadMessage = `Loaded ${newTracks.length} audio file(s) for analysis`;
    setProcessingLogs((prev) => [
      ...prev,
      `[${new Date().toLocaleTimeString()}] ${loadMessage}`,
    ]);

    // Start simulated analysis for each new track
    newTracks.forEach((track, index) => {
      // Log start of analysis
      const startMessage = `Starting audio analysis: ${track.name} (${track.codec})`;
      setProcessingLogs((prev) => [
        ...prev,
        `[${new Date().toLocaleTimeString()}] ${startMessage}`,
      ]);

      if (track.codec === "FLAC" || track.codec === "WAV") {
        setTimeout(() => {
          const lufs = -18.5 + Math.random() * 10;
          const truePeak = -0.5 - Math.random() * 2;
          const dr = 8 + Math.random() * 6;
          const adjustmentDb = targetLufs - lufs;

          setTracks((prevTracks) =>
            prevTracks.map((t) =>
              t.id === track.id
                ? {
                    ...t,
                    analysis: {
                      fileId: t.id,
                      loudness: {
                        lufs: lufs,
                        range: dr, // Use DR as loudness range for now
                      },
                      truePeak: {
                        dbtp: truePeak,
                      },
                      dynamicRange: {
                        dr: dr,
                      },
                      adjustment: {
                        targetLufs: targetLufs,
                        adjustmentDb: adjustmentDb,
                      },
                    },
                  }
                : t
            )
          );

          // Log analysis results
          const resultsMessage = `Analysis complete: ${track.name}`;
          setProcessingLogs((prev) => [
            ...prev,
            `[${new Date().toLocaleTimeString()}] ${resultsMessage}`,
          ]);

          const lufsMessage = `  LUFS: ${lufs.toFixed(
            1
          )}, True Peak: ${truePeak.toFixed(2)} dBTP, DR: ${Math.round(dr)}`;
          setProcessingLogs((prev) => [
            ...prev,
            `[${new Date().toLocaleTimeString()}] ${lufsMessage}`,
          ]);

          const adjustMessage = `  Volume adjustment: ${
            adjustmentDb >= 0 ? "+" : ""
          }${adjustmentDb.toFixed(1)} dB (target: ${targetLufs} LUFS)`;
          setProcessingLogs((prev) => [
            ...prev,
            `[${new Date().toLocaleTimeString()}] ${adjustMessage}`,
          ]);
        }, 1000 + Math.random() * 2000 + index * 500); // Stagger analysis timing
      } else {
        // Log unsupported format
        const unsupportedMessage = `Unsupported format for analysis: ${track.codec}`;
        setProcessingLogs((prev) => [
          ...prev,
          `[${new Date().toLocaleTimeString()}] ${unsupportedMessage}`,
        ]);
      }
    });
  }, []);

  // CUE file processing helper
  const processCueFileForAudio = useCallback(async (audioFile: File) => {
    try {
      // For browser environment, we can't directly read directory
      // But we can check if user drops both files together
      // For now, log that we would check for CUE files
      const audioFileName = audioFile.name;
      const baseName = audioFileName.replace(/\.[^.]+$/, ""); // Remove extension

      const cueMessage = `Checking for CUE files for: ${audioFileName}`;
      setProcessingLogs((prev) => [
        ...prev,
        `[${new Date().toLocaleTimeString()}] ${cueMessage}`,
      ]);

      // In a real implementation, we would:
      // 1. Check directory for .cue files
      // 2. Read each CUE file
      // 3. Parse FILE directive
      // 4. If FILE matches our audio file, process tracks

      const noCueMessage = `No matching CUE file found for: ${audioFileName}`;
      setProcessingLogs((prev) => [
        ...prev,
        `[${new Date().toLocaleTimeString()}] ${noCueMessage}`,
      ]);

      return false; // No CUE processing needed
    } catch (error) {
      const errorMessage = `Error checking for CUE file: ${error}`;
      setProcessingLogs((prev) => [
        ...prev,
        `[${new Date().toLocaleTimeString()}] ${errorMessage}`,
      ]);
      return false;
    }
  }, []);

  const handleTargetLufsChange = useCallback((newTargetLufs: number) => {
    setTargetLufs(newTargetLufs);
    setSelectedProfile((prev) => ({ ...prev, targetLufs: newTargetLufs }));
  }, []);

  const handleFileError = useCallback((error: string) => {
    console.error("File loading error:", error);
    // TODO: Show user-friendly error message
  }, []);

  // Enhanced file drop functionality with CUE processing
  const handleFilesDrop = useCallback(
    async (files: FileList) => {
      const fileArray = Array.from(files);
      const audioFiles = fileArray.filter((file) => {
        const ext = file.name.split(".").pop()?.toLowerCase();
        return [
          "flac",
          "wav",
          "mp3",
          "m4a",
          "aac",
          "ogg",
          "opus",
          "ape",
          "wv",
        ].includes(ext || "");
      });

      const cueFiles = fileArray.filter((file) =>
        file.name.toLowerCase().endsWith(".cue")
      );

      // Process each audio file
      for (const audioFile of audioFiles) {
        let matchingCue: File | null = null;

        if (cueFiles.length > 0) {
          const cueCheckMessage = `Found ${cueFiles.length} CUE file(s), checking for matches with ${audioFile.name}`;
          setProcessingLogs((prev) => [
            ...prev,
            `[${new Date().toLocaleTimeString()}] ${cueCheckMessage}`,
          ]);

          // Check each CUE file to see if it references this audio file
          for (const cueFile of cueFiles) {
            try {
              const cueContent = await cueFile.text();

              // Parse FILE directive from CUE
              const fileMatch = cueContent.match(
                /FILE\\s+[\"']?([^\"'\\r\\n]+)[\"']?\\s+/i
              );
              if (fileMatch) {
                const cueReferencedFile = fileMatch[1].trim();
                const cueFileName =
                  cueReferencedFile.split(/[/\\\\]/).pop() || cueReferencedFile;

                const parseMessage = `CUE file ${cueFile.name} references: ${cueFileName}`;
                setProcessingLogs((prev) => [
                  ...prev,
                  `[${new Date().toLocaleTimeString()}] ${parseMessage}`,
                ]);

                // Check if the CUE references our audio file
                if (
                  cueFileName.toLowerCase() === audioFile.name.toLowerCase()
                ) {
                  matchingCue = cueFile;
                  const matchMessage = `✓ Match found! ${cueFile.name} references ${audioFile.name}`;
                  setProcessingLogs((prev) => [
                    ...prev,
                    `[${new Date().toLocaleTimeString()}] ${matchMessage}`,
                  ]);
                  break;
                }
              }
            } catch (error) {
              const errorMessage = `Error reading CUE file ${cueFile.name}: ${error}`;
              setProcessingLogs((prev) => [
                ...prev,
                `[${new Date().toLocaleTimeString()}] ${errorMessage}`,
              ]);
            }
          }
        }

        if (matchingCue) {
          // Process as CUE + Audio combination
          const cueProcessMessage = `Processing ${audioFile.name} with CUE sheet ${matchingCue.name}`;
          setProcessingLogs((prev) => [
            ...prev,
            `[${new Date().toLocaleTimeString()}] ${cueProcessMessage}`,
          ]);

          // TODO: Extract individual tracks from CUE sheet
          // For now, just process the main audio file and note CUE presence
        }
      }

      const validTracks = audioFiles
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
        <div className="header-content">
          <div className="header-title">
            <h1>RangeForge</h1>
            <p>Swiss Army Knife for Lossless Audio Processing</p>
          </div>
          <button
            className="header-devlog-btn"
            onClick={() => setShowLogWindow(!showLogWindow)}
            title={showLogWindow ? "Hide DevLog" : "Show DevLog"}
          >
            DevLog {showLogWindow ? "›" : "‹"}
          </button>
        </div>
      </header>

      <main className="App-main">
        {/* Left Column: Main Content */}
        <div
          className={`main-content ${showLogWindow ? "with-log-panel" : ""}`}
        >
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
            {/* Always show table structure */}
            <AudioAnalysisTable
              tracks={tracks}
              onTrackSelect={handleTrackSelect}
              selectedTrackIds={selectedTrackIds}
              onRemoveTrack={handleRemoveTrack}
              selectedProfile={selectedProfile}
            />

            {/* Drop zone overlay when no files */}
            {tracks.length === 0 && (
              <div className="drop-zone-overlay">
                <div className="drop-zone-content">
                  <p>Drop audio files or albums with CUE sheets to analyze</p>
                </div>
              </div>
            )}

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
                targetLufs={targetLufs}
                onTargetLufsChange={handleTargetLufsChange}
              />
            </div>
          </div>
        </div>

        {/* Floating DevLog Toggle (when panel is closed) */}
        {!showLogWindow && (
          <button
            className="floating-devlog-btn"
            onClick={() => setShowLogWindow(true)}
            title="Show DevLog"
          >
            ‹ DevLog
          </button>
        )}

        {/* Right Column: Collapsible Process Log */}
        <div className={`log-panel ${showLogWindow ? "log-panel-open" : ""}`}>
          <div className="log-panel-header">
            <h3>DevLog</h3>
            <button
              className="log-toggle-btn"
              onClick={() => setShowLogWindow(!showLogWindow)}
              title={showLogWindow ? "Hide Log" : "Show Log"}
            >
              {showLogWindow ? "›" : "‹"}
            </button>
          </div>

          {showLogWindow && (
            <div className="log-panel-content">
              {processingLogs.length === 0 ? (
                <div className="log-empty">No processing activity yet</div>
              ) : (
                <div className="log-entries">
                  {processingLogs.map((log, index) => (
                    <div key={index} className="log-entry">
                      {log}
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}
        </div>
      </main>
    </div>
  );
}

export default App;
