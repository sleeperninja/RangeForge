import React from "react";
import { useAudio } from "../context/AudioContext";

export function FileManager() {
  const {
    state,
    addFiles,
    removeFile,
    toggleFileSelection,
    clearSelection,
    selectAll,
    analyzeSelectedFiles,
  } = useAudio();

  const handleSelectFiles = async () => {
    try {
      if (window.electronAPI) {
        const filePaths = await window.electronAPI.selectAudioFiles();
        if (filePaths.length > 0) {
          addFiles(filePaths);
        }
      }
    } catch (error) {
      console.error("Failed to select files:", error);
    }
  };

  const handleAnalyzeSelected = async () => {
    if (state.selectedFiles.length === 0) {
      alert("Please select files to analyze");
      return;
    }

    try {
      await analyzeSelectedFiles();
    } catch (error) {
      console.error("Analysis failed:", error);
      alert("Analysis failed. Please check the console for details.");
    }
  };

  const formatFileSize = (bytes: number) => {
    if (bytes === 0) return "0 Bytes";
    const k = 1024;
    const sizes = ["Bytes", "KB", "MB", "GB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
  };

  const formatDuration = (seconds: number) => {
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    const secs = Math.floor(seconds % 60);

    if (hours > 0) {
      return `${hours}:${minutes.toString().padStart(2, "0")}:${secs
        .toString()
        .padStart(2, "0")}`;
    }
    return `${minutes}:${secs.toString().padStart(2, "0")}`;
  };

  return (
    <div className="component-container">
      <div className="component-header">
        <h2 className="component-title">File Manager</h2>
        <div style={{ display: "flex", gap: "10px" }}>
          <button className="btn btn-primary" onClick={handleSelectFiles}>
            Add Files
          </button>
          {state.files.length > 0 && (
            <button
              className="btn btn-success"
              onClick={handleAnalyzeSelected}
              disabled={state.selectedFiles.length === 0 || state.isProcessing}
            >
              Analyze Selected
            </button>
          )}
        </div>
      </div>

      {state.files.length === 0 ? (
        <div style={{ textAlign: "center", padding: "40px", color: "#666" }}>
          <p>No audio files loaded</p>
          <p>Click "Add Files" to get started</p>
        </div>
      ) : (
        <>
          <div
            style={{
              marginBottom: "15px",
              display: "flex",
              gap: "10px",
              alignItems: "center",
            }}
          >
            <span style={{ color: "#666", fontSize: "14px" }}>
              {state.files.length} file(s) | {state.selectedFiles.length}{" "}
              selected
            </span>
            <button
              className="btn btn-secondary"
              onClick={selectAll}
              style={{ padding: "5px 10px", fontSize: "12px" }}
            >
              Select All
            </button>
            <button
              className="btn btn-secondary"
              onClick={clearSelection}
              style={{ padding: "5px 10px", fontSize: "12px" }}
            >
              Clear Selection
            </button>
          </div>

          <ul className="file-list">
            {state.files.map((file) => (
              <li key={file.id} className="file-item">
                <div
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: "10px",
                    flex: 1,
                  }}
                >
                  <input
                    type="checkbox"
                    checked={state.selectedFiles.includes(file.id)}
                    onChange={() => toggleFileSelection(file.id)}
                  />
                  <div className="file-info">
                    <div className="file-name">{file.name}</div>
                    <div className="file-details">
                      {file.analysisResult && (
                        <>
                          Duration:{" "}
                          {formatDuration(file.analysisResult.duration)} | DR:{" "}
                          {file.analysisResult.dynamicRange.dr.toFixed(1)} (
                          {file.analysisResult.dynamicRange.drMeasurement}) |
                          Peak:{" "}
                          {file.analysisResult.volumeStats.peak.toFixed(1)} dB |
                          Format:{" "}
                          {file.analysisResult.metadata.format.toUpperCase()}
                        </>
                      )}
                      {!file.analysisResult &&
                        file.status === "pending" &&
                        "Ready for analysis"}
                      {file.status === "processing" && "Analyzing..."}
                      {file.status === "error" && file.error}
                    </div>
                  </div>
                </div>

                <div className="file-actions">
                  <span className={`status status-${file.status}`}>
                    {file.status}
                  </span>
                  <button
                    className="btn btn-danger"
                    onClick={() => removeFile(file.id)}
                    style={{ padding: "5px 10px", fontSize: "12px" }}
                  >
                    Remove
                  </button>
                </div>
              </li>
            ))}
          </ul>
        </>
      )}
    </div>
  );
}
