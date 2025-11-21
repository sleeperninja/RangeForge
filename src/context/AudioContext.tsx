import React, { createContext, useContext } from "react";

// Stub implementation to prevent compilation errors
const AudioContext = createContext(null);

export const useAudio = () => {
  return {
    state: {
      files: [],
      selectedFiles: [],
      isProcessing: false,
      isAnalyzing: false,
      progress: null,
      analysisProgress: 0,
      processingProgress: 0,
      error: undefined,
    },
    addFiles: () => {},
    removeFile: () => {},
    toggleFileSelection: () => {},
    clearSelection: () => {},
    selectAll: () => {},
    analyzeSelectedFiles: () => Promise.resolve(),
    processSelectedFiles: () => Promise.resolve(),
  };
};

export const AudioProvider: React.FC<{ children: React.ReactNode }> = ({
  children,
}) => {
  return <AudioContext.Provider value={null}>{children}</AudioContext.Provider>;
};
