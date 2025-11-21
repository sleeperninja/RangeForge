import React, { useCallback, useState, useRef } from "react";
import { AudioTrack } from "../../types/audio";
import "./FileDropZone.css";

interface FileDropZoneProps {
  onFilesLoaded: (tracks: AudioTrack[]) => void;
  onError: (error: string) => void;
  disabled?: boolean;
}

interface DropState {
  isDragOver: boolean;
  isProcessing: boolean;
}

export const FileDropZone: React.FC<FileDropZoneProps> = ({
  onFilesLoaded,
  onError,
  disabled = false,
}) => {
  const [dropState, setDropState] = useState<DropState>({
    isDragOver: false,
    isProcessing: false,
  });
  const fileInputRef = useRef<HTMLInputElement>(null);

  // Handle file processing - simplified for now
  const processFiles = useCallback(
    async (files: FileList) => {
      if (disabled) return;

      setDropState({ isDragOver: false, isProcessing: true });

      try {
        // For now, create basic AudioTrack objects from the files
        const tracks: AudioTrack[] = Array.from(files)
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

            return {
              id: `file_${Date.now()}_${index}`,
              name: fileName,
              path: file.name, // In Electron this would be file.path
              codec: codecMap[fileExt] || "Unknown",
              fileSize: file.size,
              tags: [],
              isProcessing: false,
              isCueTrack: false,
            };
          })
          .filter((track) => track.codec !== "Unknown"); // Only include supported files

        if (tracks.length === 0) {
          onError("No supported audio files found");
        } else {
          onFilesLoaded(tracks);
        }
      } catch (error) {
        onError(
          `Failed to process files: ${
            error instanceof Error ? error.message : "Unknown error"
          }`
        );
      } finally {
        setDropState({ isDragOver: false, isProcessing: false });
      }
    },
    [disabled, onFilesLoaded, onError]
  );

  // Drag and drop handlers
  const handleDragOver = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (!disabled) {
        setDropState((prev) => ({ ...prev, isDragOver: true }));
      }
    },
    [disabled]
  );

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setDropState((prev) => ({ ...prev, isDragOver: false }));
  }, []);

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      e.stopPropagation();

      if (disabled || !e.dataTransfer.files) return;

      processFiles(e.dataTransfer.files);
    },
    [disabled, processFiles]
  );

  // File input handler for click-to-browse
  const handleFileSelect = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      if (e.target.files && e.target.files.length > 0) {
        processFiles(e.target.files);
      }
    },
    [processFiles]
  );

  const handleClick = useCallback(() => {
    if (!disabled && fileInputRef.current) {
      fileInputRef.current.click();
    }
  }, [disabled]);

  // CSS classes based on state
  const dropZoneClasses = [
    "file-drop-zone",
    dropState.isDragOver ? "drag-over" : "",
    dropState.isProcessing ? "processing" : "",
    disabled ? "disabled" : "",
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <div
      className={dropZoneClasses}
      onDragOver={handleDragOver}
      onDragLeave={handleDragLeave}
      onDrop={handleDrop}
      onClick={handleClick}
      role="button"
      tabIndex={disabled ? -1 : 0}
      aria-label="Drop audio files here"
    >
      <input
        ref={fileInputRef}
        type="file"
        multiple
        accept=".flac,.wav,.mp3,.m4a,.aac,.ogg,.opus,.ape,.wv"
        onChange={handleFileSelect}
        style={{ display: "none" }}
      />

      <div className="drop-zone-content">
        {dropState.isProcessing ? (
          <div className="processing-state">
            <div className="loading-spinner"></div>
            <div className="drop-zone-text">
              <h3>Processing Files...</h3>
              <p>Analyzing audio files</p>
            </div>
          </div>
        ) : (
          <div className="idle-state">
            <div className="drop-zone-icon">📁</div>
            <div className="drop-zone-text">
              <h3>Drop Files Here</h3>
              <p>
                Drag & drop audio files
                <br />
                <span className="supported-formats">
                  Supports: FLAC, WAV, MP3, M4A, AAC, OGG, Opus, APE, WavPack
                </span>
              </p>
              <button className="browse-button" type="button">
                Browse Files
              </button>
            </div>
          </div>
        )}
      </div>

      {dropState.isDragOver && (
        <div className="drop-overlay">
          <div className="drop-overlay-text">Release to add files</div>
        </div>
      )}
    </div>
  );
};
