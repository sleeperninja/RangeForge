import React from "react";
import { useAudio } from "../context/AudioContext";

export function ProgressMonitor() {
  const { state } = useAudio();

  if (!state.isProcessing && !state.progress) {
    return null;
  }

  const getProgressPercentage = () => {
    if (!state.progress) return 0;

    if (state.progress.percent) {
      return Math.round(state.progress.percent);
    }

    if (state.progress.currentFile && state.progress.totalFiles) {
      return Math.round(
        (state.progress.currentFile / state.progress.totalFiles) * 100
      );
    }

    return 0;
  };

  const getProgressMessage = () => {
    if (!state.progress) return "Processing...";

    return state.progress.message || "Processing...";
  };

  const formatFilePath = (filePath?: string) => {
    if (!filePath) return "";
    return filePath.split("\\").pop() || filePath.split("/").pop() || filePath;
  };

  return (
    <div className="component-container">
      <div className="component-header">
        <h2 className="component-title">Progress Monitor</h2>
        {state.isProcessing && (
          <span
            style={{ color: "#007bff", fontSize: "14px", fontWeight: "500" }}
          >
            Processing...
          </span>
        )}
      </div>

      {state.progress && (
        <div>
          <div className="progress">
            <div
              className="progress-bar"
              style={{ width: `${getProgressPercentage()}%` }}
            >
              {getProgressPercentage()}%
            </div>
          </div>

          <div style={{ marginTop: "10px", fontSize: "14px" }}>
            <div style={{ fontWeight: "500", marginBottom: "5px" }}>
              {getProgressMessage()}
            </div>

            {state.progress.filePath && (
              <div style={{ color: "#666", fontSize: "12px" }}>
                Current file: {formatFilePath(state.progress.filePath)}
              </div>
            )}

            {state.progress.currentFile && state.progress.totalFiles && (
              <div style={{ color: "#666", fontSize: "12px" }}>
                File {state.progress.currentFile} of {state.progress.totalFiles}
              </div>
            )}

            {state.progress.error && (
              <div
                style={{ color: "#dc3545", fontSize: "12px", marginTop: "5px" }}
              >
                Error: {state.progress.error}
              </div>
            )}
          </div>
        </div>
      )}

      {state.isProcessing && !state.progress && (
        <div style={{ textAlign: "center", padding: "20px", color: "#666" }}>
          <div style={{ marginBottom: "10px" }}>Starting processing...</div>
          <div className="progress">
            <div
              className="progress-bar"
              style={{
                width: "100%",
                animation: "pulse 1.5s ease-in-out infinite",
              }}
            >
              Processing
            </div>
          </div>
        </div>
      )}

      {/* Processing Statistics */}
      {state.files.length > 0 && (
        <div
          style={{
            marginTop: "20px",
            padding: "15px",
            backgroundColor: "#f8f9fa",
            borderRadius: "5px",
          }}
        >
          <h4 style={{ margin: "0 0 10px 0", fontSize: "14px", color: "#333" }}>
            Session Statistics
          </h4>
          <div style={{ fontSize: "12px", color: "#666" }}>
            <div>Total files: {state.files.length}</div>
            <div>
              Completed:{" "}
              {state.files.filter((f) => f.status === "complete").length}
            </div>
            <div>
              Processing:{" "}
              {state.files.filter((f) => f.status === "processing").length}
            </div>
            <div>
              Pending:{" "}
              {state.files.filter((f) => f.status === "pending").length}
            </div>
            <div>
              Errors: {state.files.filter((f) => f.status === "error").length}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
