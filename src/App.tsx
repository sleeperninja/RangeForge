import React, { useState, useCallback, useEffect } from "react";
import "./App.css";
import { AudioAnalysisTable } from "./components/AudioAnalysisTable";
import { ProcessingControls } from "./components/ProcessingControls";
import { AudioTrack, ProcessingProfile, GenreTag } from "./types/audio";

// CUE file metadata interfaces
interface CueMetadata {
  genre?: string;
  date?: string;
  discId?: string;
  comment?: string;
  performer: string;
  title: string;
  file: string;
  tracks: CueTrack[];
}

interface CueTrack {
  number: string;
  title: string;
  performer: string;
  indexPositions: string[];
  startTime: number; // in seconds
  endTime?: number; // in seconds
}

interface Settings {
  targetLufs: number;
  autoMode: boolean;
  metadataApi: boolean;
  selectedProfile: string;
  namingConvention: string;
}

// Convert CUE time format (MM:SS:FF) to seconds
const parseTimeToSeconds = (timeStr: string): number => {
  const [minutes, seconds, frames] = timeStr.split(":").map(Number);
  return minutes * 60 + seconds + frames / 75;
};

// Parse CUE file content into structured metadata
const parseCueFile = (content: string): CueMetadata => {
  const lines = content.split("\n").map((line) => line.trim());
  const metadata: CueMetadata = {
    performer: "",
    title: "",
    file: "",
    tracks: [],
  };

  let currentTrack: Partial<CueTrack> | null = null;

  for (const line of lines) {
    // Global metadata
    if (line.startsWith("REM GENRE")) {
      metadata.genre = line.match(/REM GENRE (.+)/)?.[1];
    } else if (line.startsWith("REM DATE")) {
      metadata.date = line.match(/REM DATE (.+)/)?.[1];
    } else if (line.startsWith("REM DISCID")) {
      metadata.discId = line.match(/REM DISCID (.+)/)?.[1];
    } else if (line.startsWith("REM COMMENT")) {
      metadata.comment = line.match(/REM COMMENT "([^"]+)"/)?.[1];
    } else if (line.startsWith("PERFORMER") && !currentTrack) {
      metadata.performer = line.match(/PERFORMER "([^"]+)"/)?.[1] || "";
    } else if (line.startsWith("TITLE") && !currentTrack) {
      metadata.title = line.match(/TITLE "([^"]+)"/)?.[1] || "";
    } else if (line.startsWith("FILE")) {
      metadata.file = line.match(/FILE "([^"]+)"/)?.[1] || "";
    }

    // Track parsing
    if (line.match(/TRACK \\d+ AUDIO/)) {
      // Save previous track if exists
      if (currentTrack && currentTrack.number) {
        metadata.tracks.push(currentTrack as CueTrack);
      }

      const trackNumber = line.match(/TRACK (\\d+)/)?.[1];
      currentTrack = {
        number: trackNumber || "",
        title: "",
        performer: metadata.performer,
        indexPositions: [],
        startTime: 0,
      };
    } else if (currentTrack) {
      if (line.startsWith("TITLE")) {
        currentTrack.title = line.match(/TITLE "([^"]+)"/)?.[1] || "";
      } else if (line.startsWith("PERFORMER")) {
        currentTrack.performer =
          line.match(/PERFORMER "([^"]+)"/)?.[1] || metadata.performer;
      } else if (line.startsWith("INDEX")) {
        const indexMatch = line.match(/INDEX (\\d+) ([\\d:]+)/);
        if (indexMatch) {
          currentTrack.indexPositions?.push(indexMatch[2]);
          if (indexMatch[1] === "01") {
            // INDEX 01 is the track start
            currentTrack.startTime = parseTimeToSeconds(indexMatch[2]);
          }
        }
      }
    }
  }

  // Add the last track
  if (currentTrack && currentTrack.number) {
    metadata.tracks.push(currentTrack as CueTrack);
  }

  // Calculate end times
  for (let i = 0; i < metadata.tracks.length; i++) {
    if (i < metadata.tracks.length - 1) {
      metadata.tracks[i].endTime = metadata.tracks[i + 1].startTime;
    }
    // Last track end time will be determined during processing
  }

  return metadata;
};

// Apply naming convention with variable substitution
const applyNamingConvention = (
  convention: string,
  variables: {
    artist: string;
    album: string;
    trackName: string;
    trackNumber: string;
  }
): string => {
  let result = convention;

  // Handle padded track numbers like {trackNumber:02d}
  const trackNumberPadMatch = result.match(/\\{trackNumber:(\\d+)d\\}/);
  if (trackNumberPadMatch) {
    const padding = parseInt(trackNumberPadMatch[1]);
    const paddedNumber = variables.trackNumber.padStart(padding, "0");
    result = result.replace(trackNumberPadMatch[0], paddedNumber);
  }

  // Replace simple variables
  result = result.replace("{artist}", variables.artist);
  result = result.replace("{album}", variables.album);
  result = result.replace("{trackName}", variables.trackName);
  result = result.replace("{trackNumber}", variables.trackNumber);

  // Clean up any remaining brackets or invalid characters
  result = result.replace(/[<>:"/\\\\|?*]/g, "_"); // Windows-safe filename

  return result;
};

function App() {
  const [tracks, setTracks] = useState<AudioTrack[]>([]);
  const [selectedTrackIds, setSelectedTrackIds] = useState<string[]>([]);
  const [autoMode, setAutoMode] = useState(false);
  const [metadataApi, setMetadataApi] = useState(false);
  const [isProcessing, setIsProcessing] = useState(false);
  const [showLogWindow, setShowLogWindow] = useState(false);
  const [processingLogs, setProcessingLogs] = useState<string[]>([]);
  const [targetLufs, setTargetLufs] = useState(-18); // Default to -18 LUFS
  const [showSettings, setShowSettings] = useState(false);

  // Settings state
  const [settings, setSettings] = useState<Settings>({
    targetLufs: -18,
    autoMode: false,
    metadataApi: false,
    selectedProfile: "ALAC",
    namingConvention: "{trackNumber:02d} - {artist} - {trackName}",
  });

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
        const loadedSettings = JSON.parse(savedSettings);
        setSettings({
          targetLufs: loadedSettings.targetLufs || -18,
          autoMode: loadedSettings.autoMode || false,
          metadataApi: loadedSettings.metadataApi || false,
          selectedProfile: loadedSettings.selectedProfile || "ALAC",
          namingConvention:
            loadedSettings.namingConvention ||
            "{trackNumber:02d} - {artist} - {trackName}",
        });
        setTargetLufs(loadedSettings.targetLufs || -18);
        setAutoMode(loadedSettings.autoMode || false);
        setMetadataApi(loadedSettings.metadataApi || false);
        setSelectedProfile((prev) => ({ ...prev, ...loadedSettings.profile }));
      } catch (error) {
        console.log("Error loading settings:", error);
      }
    }
  }, []);

  // Save settings when they change
  useEffect(() => {
    const settingsToSave = {
      targetLufs: settings.targetLufs,
      autoMode: settings.autoMode,
      metadataApi: settings.metadataApi,
      selectedProfile: settings.selectedProfile,
      namingConvention: settings.namingConvention,
      profile: selectedProfile,
    };
    localStorage.setItem("rangeforge-settings", JSON.stringify(settingsToSave));
  }, [settings, selectedProfile]);

  // Sync targetLufs with profile
  useEffect(() => {
    setSelectedProfile((prev) => ({ ...prev, targetLufs: targetLufs }));
  }, [targetLufs]);

  // Helper to add timestamped logs
  const addToLog = useCallback((message: string) => {
    setProcessingLogs((prev) => [
      ...prev,
      `[${new Date().toLocaleTimeString()}] ${message}`,
    ]);
  }, []);

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
    setShowSettings(true);
  }, []);

  const handleNamingConventionChange = useCallback((newConvention: string) => {
    setSettings((prev) => ({ ...prev, namingConvention: newConvention }));
  }, []);

  const handleSettingsClose = useCallback(() => {
    setShowSettings(false);
  }, []);

  // Real audio analysis using Electron API
  const analyzeTrackAudio = useCallback(
    async (track: AudioTrack) => {
      addToLog(`🔍 Starting audio analysis: ${track.name}`);

      try {
        let analysisResult;

        if (track.isCueTrack && track.cueData) {
          // For CUE tracks, analyze the specific segment
          const mmapResult = await window.electronAPI?.mmap.openFile(
            track.path.split("#")[0]
          );
          if (mmapResult?.success) {
            const duration = track.cueData.endTime
              ? track.cueData.endTime - track.cueData.startTime
              : 60; // Default 60 seconds if no end time
            analysisResult = await window.electronAPI?.audio.analyzeSegment(
              mmapResult.fileId!,
              track.cueData.startTime,
              duration
            );
            await window.electronAPI?.mmap.closeFile(mmapResult.fileId!);
          }
        } else {
          // For regular files, analyze the whole file
          analysisResult = await window.electronAPI?.audio.analyze(track.path);
        }

        if (analysisResult?.success) {
          const { lufs, truePeak, dynamicRange } = analysisResult.analysis;
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
                        range: dynamicRange,
                      },
                      truePeak: {
                        dbtp: truePeak,
                      },
                      dynamicRange: {
                        dr: dynamicRange,
                      },
                      adjustment: {
                        targetLufs: targetLufs,
                        adjustmentDb: adjustmentDb,
                      },
                    },
                    isProcessing: false,
                  }
                : t
            )
          );

          addToLog(`✅ Analysis complete: ${track.name}`);
          addToLog(
            `   LUFS: ${lufs.toFixed(1)}, True Peak: ${truePeak.toFixed(
              2
            )} dBTP, DR: ${Math.round(dynamicRange)}`
          );
          addToLog(
            `   Adjustment needed: ${
              adjustmentDb >= 0 ? "+" : ""
            }${adjustmentDb.toFixed(1)} dB`
          );
        } else {
          throw new Error(analysisResult?.error || "Analysis failed");
        }
      } catch (error) {
        const errorMessage =
          error instanceof Error ? error.message : String(error);
        addToLog(`❌ Analysis failed: ${track.name} - ${errorMessage}`);
        setTracks((prevTracks) =>
          prevTracks.map((t) =>
            t.id === track.id
              ? { ...t, isProcessing: false, error: errorMessage }
              : t
          )
        );
      }
    },
    [targetLufs, addToLog]
  );

  // File handling functions
  const handleFilesLoaded = useCallback(
    (newTracks: AudioTrack[]) => {
      setTracks((prev) => [...prev, ...newTracks]);
      setShowLogWindow(true);

      addToLog(`📁 Loaded ${newTracks.length} audio file(s) for analysis`);

      // Start real analysis for each new track (staggered to prevent overload)
      newTracks.forEach((track, index) => {
        setTimeout(async () => {
          await analyzeTrackAudio(track);
        }, index * 200); // Stagger analysis calls
      });
    },
    [analyzeTrackAudio, addToLog]
  );

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
    addToLog(`❌ File loading error: ${error}`);
  }, []);

  // Enhanced file drop functionality with comprehensive CUE processing using Electron API
  const handleFilesDrop = useCallback(
    async (files: FileList) => {
      const fileArray = Array.from(files);
      const filePaths = fileArray.map((file: any) => file.path || file.name);

      addToLog(`📁 Processing ${filePaths.length} dropped files...`);

      try {
        // Use Electron API to detect and process CUE files with associated audio
        const cueResult = await window.electronAPI?.cue.detectAndProcess(
          filePaths
        );

        if (
          cueResult?.success &&
          cueResult.tracks &&
          cueResult.tracks.length > 0
        ) {
          addToLog(
            `🎵 Found ${cueResult.tracks.length} tracks from CUE processing`
          );

          // Convert CUE tracks to AudioTrack format and analyze
          const cueBasedTracks: AudioTrack[] = [];

          for (const track of cueResult.tracks) {
            addToLog(`📝 Processing CUE track: ${track.name}`);

            // Create AudioTrack from CUE track data
            const audioTrack: AudioTrack = {
              id: track.id,
              name: track.name,
              path: track.path,
              codec: track.codec,
              fileSize: track.fileSize,
              sampleRate: track.sampleRate,
              bitDepth: track.bitDepth,
              channels: track.channels,
              duration: track.duration,
              analysis: undefined,
              tags: track.tags || [],
              isProcessing: true,
              isCueTrack: track.isCueTrack || true,
              cueData: {
                startTime: track.cueData?.startTime || 0,
                endTime: track.cueData?.endTime,
                trackNumber: track.cueData?.trackNumber || "1",
                performer: track.cueData?.performer || "Unknown",
                title: track.cueData?.title || track.name,
              },
            };

            cueBasedTracks.push(audioTrack);
          }

          handleFilesLoaded(cueBasedTracks);
          return;
        }
      } catch (error) {
        addToLog(`❌ Error processing CUE files: ${error}`);
      }

      // Fallback: Process individual audio files
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
            path: (file as any).path || fileName, // Use actual file path in Electron
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

      {/* Settings Dialog */}
      {showSettings && (
        <div className="settings-overlay" onClick={handleSettingsClose}>
          <div className="settings-dialog" onClick={(e) => e.stopPropagation()}>
            <div className="settings-header">
              <h3>Settings</h3>
              <button className="close-btn" onClick={handleSettingsClose}>
                ×
              </button>
            </div>
            <div className="settings-content">
              <div className="setting-group">
                <label htmlFor="naming-convention">
                  File Naming Convention:
                </label>
                <input
                  id="naming-convention"
                  type="text"
                  value={settings.namingConvention}
                  onChange={(e) => handleNamingConventionChange(e.target.value)}
                  className="naming-input"
                />
                <div className="naming-help">
                  <p>Available variables:</p>
                  <ul>
                    <li>
                      <code>{"{artist}"}</code> - PERFORMER from CUE file
                    </li>
                    <li>
                      <code>{"{album}"}</code> - TITLE from CUE file
                    </li>
                    <li>
                      <code>{"{trackName}"}</code> - Track TITLE
                    </li>
                    <li>
                      <code>{"{trackNumber}"}</code> - Track number
                    </li>
                    <li>
                      <code>{"{trackNumber:02d}"}</code> - Zero-padded track
                      number
                    </li>
                  </ul>
                  <p>
                    Example:{" "}
                    <code>{"{trackNumber:02d} - {artist} - {trackName}"}</code>
                  </p>
                  <p>
                    Result: <code>01 - Ministry - Reload</code>
                  </p>
                </div>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default App;
