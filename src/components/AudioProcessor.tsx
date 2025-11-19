import React from "react";
import { useAudio } from "../context/AudioContext";

export function AudioProcessor() {
  const { state, processSelectedFiles } = useAudio();

  const [processingOptions, setProcessingOptions] = React.useState({
    operation: "analyze",
    normalize: false,
    targetLUFS: -23,
    limiterThreshold: -1,
    outputDirectory: "",
  });

  const handleProcessFiles = async () => {
    if (state.selectedFiles.length === 0) {
      alert("Please select files to process");
      return;
    }

    try {
      await processSelectedFiles(processingOptions);
    } catch (error) {
      console.error("Processing failed:", error);
      alert("Processing failed. Please check the console for details.");
    }
  };

  const handleOptionChange = (key: string, value: any) => {
    setProcessingOptions((prev) => ({
      ...prev,
      [key]: value,
    }));
  };

  return (
    <div className="component-container">
      <div className="component-header">
        <h2 className="component-title">Audio Processing</h2>
        <span style={{ color: "#666", fontSize: "14px" }}>
          {state.selectedFiles.length} file(s) selected
        </span>
      </div>

      <div className="form-group">
        <label className="form-label">Operation</label>
        <select
          className="form-control"
          value={processingOptions.operation}
          onChange={(e) => handleOptionChange("operation", e.target.value)}
        >
          <option value="analyze">Analyze Only</option>
          <option value="normalize">Normalize Audio</option>
          <option value="convert">Convert Format</option>
        </select>
      </div>

      {processingOptions.operation === "normalize" && (
        <>
          <div className="form-group">
            <label className="form-label">
              <input
                type="checkbox"
                checked={processingOptions.normalize}
                onChange={(e) =>
                  handleOptionChange("normalize", e.target.checked)
                }
                style={{ marginRight: "8px" }}
              />
              Enable Loudness Normalization
            </label>
          </div>

          {processingOptions.normalize && (
            <div className="form-group">
              <label className="form-label">Target LUFS</label>
              <input
                type="number"
                className="form-control"
                value={processingOptions.targetLUFS}
                onChange={(e) =>
                  handleOptionChange("targetLUFS", parseFloat(e.target.value))
                }
                min={-40}
                max={0}
                step={0.1}
              />
              <small style={{ color: "#666", fontSize: "12px" }}>
                Recommended: -23 LUFS for broadcast, -16 LUFS for streaming
              </small>
            </div>
          )}

          <div className="form-group">
            <label className="form-label">Limiter Threshold (dB)</label>
            <input
              type="number"
              className="form-control"
              value={processingOptions.limiterThreshold}
              onChange={(e) =>
                handleOptionChange(
                  "limiterThreshold",
                  parseFloat(e.target.value)
                )
              }
              min={-10}
              max={0}
              step={0.1}
            />
          </div>
        </>
      )}

      <div className="form-group">
        <label className="form-label">Output Directory (optional)</label>
        <input
          type="text"
          className="form-control"
          value={processingOptions.outputDirectory}
          onChange={(e) =>
            handleOptionChange("outputDirectory", e.target.value)
          }
          placeholder="Leave empty to use source directory"
        />
      </div>

      <div style={{ display: "flex", gap: "10px", marginTop: "20px" }}>
        <button
          className="btn btn-primary"
          onClick={handleProcessFiles}
          disabled={state.selectedFiles.length === 0 || state.isProcessing}
        >
          {state.isProcessing ? "Processing..." : "Start Processing"}
        </button>

        <button
          className="btn btn-secondary"
          disabled={state.isProcessing}
          onClick={() =>
            setProcessingOptions({
              operation: "analyze",
              normalize: false,
              targetLUFS: -23,
              limiterThreshold: -1,
              outputDirectory: "",
            })
          }
        >
          Reset Options
        </button>
      </div>

      {state.selectedFiles.length > 0 && (
        <div
          style={{
            marginTop: "20px",
            padding: "15px",
            backgroundColor: "#f8f9fa",
            borderRadius: "5px",
          }}
        >
          <h4 style={{ margin: "0 0 10px 0", fontSize: "14px", color: "#333" }}>
            Selected Files Summary
          </h4>
          <ul
            style={{
              listStyle: "none",
              padding: 0,
              margin: 0,
              fontSize: "12px",
              color: "#666",
            }}
          >
            {state.files
              .filter((file) => state.selectedFiles.includes(file.id))
              .map((file) => (
                <li key={file.id} style={{ marginBottom: "5px" }}>
                  {file.name}
                  {file.analysisResult && (
                    <span style={{ marginLeft: "10px" }}>
                      (DR: {file.analysisResult.dynamicRange.dr.toFixed(1)},
                      Peak: {file.analysisResult.volumeStats.peak.toFixed(1)}dB)
                    </span>
                  )}
                </li>
              ))}
          </ul>
        </div>
      )}
    </div>
  );
}
