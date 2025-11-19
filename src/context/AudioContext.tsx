import React, {
  createContext,
  useContext,
  useReducer,
  useEffect,
  ReactNode,
} from "react";

// Types
export interface AudioFile {
  id: string;
  path: string;
  name: string;
  size: number;
  status: "pending" | "processing" | "complete" | "error";
  analysisResult?: AudioAnalysisResult;
  error?: string;
}

export interface AudioAnalysisResult {
  filePath: string;
  duration: number;
  volumeStats: {
    meanVolume: number;
    maxVolume: number;
    minVolume: number;
    rms: number;
    peak: number;
  };
  dynamicRange: {
    dr: number;
    drMeasurement: string;
  };
  metadata: {
    format: string;
    bitrate: number;
    sampleRate: number;
    channels: number;
  };
}

export interface ProcessingProgress {
  type: string;
  currentFile?: number;
  totalFiles?: number;
  filePath?: string;
  percent?: number;
  message?: string;
  error?: string;
}

// State interface
interface AudioState {
  files: AudioFile[];
  isProcessing: boolean;
  progress: ProcessingProgress | null;
  selectedFiles: string[];
}

// Actions
type AudioAction =
  | { type: "ADD_FILES"; payload: AudioFile[] }
  | { type: "REMOVE_FILE"; payload: string }
  | {
      type: "UPDATE_FILE_STATUS";
      payload: { id: string; status: AudioFile["status"]; error?: string };
    }
  | {
      type: "SET_ANALYSIS_RESULT";
      payload: { id: string; result: AudioAnalysisResult };
    }
  | { type: "SET_PROCESSING"; payload: boolean }
  | { type: "SET_PROGRESS"; payload: ProcessingProgress | null }
  | { type: "SELECT_FILE"; payload: string }
  | { type: "DESELECT_FILE"; payload: string }
  | { type: "CLEAR_SELECTION" }
  | { type: "SELECT_ALL" }
  | { type: "CLEAR_FILES" };

// Initial state
const initialState: AudioState = {
  files: [],
  isProcessing: false,
  progress: null,
  selectedFiles: [],
};

// Reducer
function audioReducer(state: AudioState, action: AudioAction): AudioState {
  switch (action.type) {
    case "ADD_FILES":
      return {
        ...state,
        files: [...state.files, ...action.payload],
      };

    case "REMOVE_FILE":
      return {
        ...state,
        files: state.files.filter((file) => file.id !== action.payload),
        selectedFiles: state.selectedFiles.filter(
          (id) => id !== action.payload
        ),
      };

    case "UPDATE_FILE_STATUS":
      return {
        ...state,
        files: state.files.map((file) =>
          file.id === action.payload.id
            ? {
                ...file,
                status: action.payload.status,
                error: action.payload.error,
              }
            : file
        ),
      };

    case "SET_ANALYSIS_RESULT":
      return {
        ...state,
        files: state.files.map((file) =>
          file.id === action.payload.id
            ? {
                ...file,
                analysisResult: action.payload.result,
                status: "complete",
              }
            : file
        ),
      };

    case "SET_PROCESSING":
      return {
        ...state,
        isProcessing: action.payload,
      };

    case "SET_PROGRESS":
      return {
        ...state,
        progress: action.payload,
      };

    case "SELECT_FILE":
      return {
        ...state,
        selectedFiles: state.selectedFiles.includes(action.payload)
          ? state.selectedFiles
          : [...state.selectedFiles, action.payload],
      };

    case "DESELECT_FILE":
      return {
        ...state,
        selectedFiles: state.selectedFiles.filter(
          (id) => id !== action.payload
        ),
      };

    case "CLEAR_SELECTION":
      return {
        ...state,
        selectedFiles: [],
      };

    case "SELECT_ALL":
      return {
        ...state,
        selectedFiles: state.files.map((file) => file.id),
      };

    case "CLEAR_FILES":
      return {
        ...state,
        files: [],
        selectedFiles: [],
        progress: null,
      };

    default:
      return state;
  }
}

// Context
interface AudioContextType {
  state: AudioState;
  dispatch: React.Dispatch<AudioAction>;
  addFiles: (filePaths: string[]) => void;
  removeFile: (id: string) => void;
  analyzeSelectedFiles: () => Promise<void>;
  processSelectedFiles: (options: any) => Promise<void>;
  selectFile: (id: string) => void;
  deselectFile: (id: string) => void;
  toggleFileSelection: (id: string) => void;
  clearSelection: () => void;
  selectAll: () => void;
}

const AudioContext = createContext<AudioContextType | null>(null);

// Provider component
interface AudioProviderProps {
  children: ReactNode;
}

export function AudioProvider({ children }: AudioProviderProps) {
  const [state, dispatch] = useReducer(audioReducer, initialState);

  // Set up Electron IPC listeners
  useEffect(() => {
    if (!window.electronAPI) return;

    const handleProgress = (data: ProcessingProgress) => {
      dispatch({ type: "SET_PROGRESS", payload: data });
    };

    const handleAnalysisComplete = (data: AudioAnalysisResult) => {
      const file = state.files.find((f) => f.path === data.filePath);
      if (file) {
        dispatch({
          type: "SET_ANALYSIS_RESULT",
          payload: { id: file.id, result: data },
        });
      }
    };

    window.electronAPI.onProcessingProgress(handleProgress);
    window.electronAPI.onAnalysisComplete(handleAnalysisComplete);

    return () => {
      window.electronAPI.removeAllListeners("processing-progress");
      window.electronAPI.removeAllListeners("analysis-complete");
    };
  }, [state.files]);

  // Helper functions
  const addFiles = (filePaths: string[]) => {
    const newFiles: AudioFile[] = filePaths.map((path) => ({
      id: `${Date.now()}-${Math.random().toString(36).substr(2, 9)}`,
      path,
      name: path.split("\\").pop() || path.split("/").pop() || path,
      size: 0, // Will be populated when file is analyzed
      status: "pending",
    }));

    dispatch({ type: "ADD_FILES", payload: newFiles });
  };

  const removeFile = (id: string) => {
    dispatch({ type: "REMOVE_FILE", payload: id });
  };

  const analyzeSelectedFiles = async () => {
    if (!window.electronAPI) {
      console.error("Electron API not available");
      return;
    }

    const selectedFiles = state.files.filter((file) =>
      state.selectedFiles.includes(file.id)
    );

    if (selectedFiles.length === 0) {
      console.warn("No files selected for analysis");
      return;
    }

    dispatch({ type: "SET_PROCESSING", payload: true });

    try {
      for (const file of selectedFiles) {
        dispatch({
          type: "UPDATE_FILE_STATUS",
          payload: { id: file.id, status: "processing" },
        });

        try {
          const result = await window.electronAPI.analyzeAudio(file.path);
          dispatch({
            type: "SET_ANALYSIS_RESULT",
            payload: { id: file.id, result },
          });
        } catch (error) {
          dispatch({
            type: "UPDATE_FILE_STATUS",
            payload: {
              id: file.id,
              status: "error",
              error: error instanceof Error ? error.message : String(error),
            },
          });
        }
      }
    } finally {
      dispatch({ type: "SET_PROCESSING", payload: false });
      dispatch({ type: "SET_PROGRESS", payload: null });
    }
  };

  const processSelectedFiles = async (options: any) => {
    if (!window.electronAPI) {
      console.error("Electron API not available");
      return;
    }

    const selectedFiles = state.files.filter((file) =>
      state.selectedFiles.includes(file.id)
    );

    if (selectedFiles.length === 0) {
      console.warn("No files selected for processing");
      return;
    }

    dispatch({ type: "SET_PROCESSING", payload: true });

    try {
      const filePaths = selectedFiles.map((file) => file.path);
      await window.electronAPI.processFiles(filePaths, options);
    } catch (error) {
      console.error("Processing failed:", error);
    } finally {
      dispatch({ type: "SET_PROCESSING", payload: false });
      dispatch({ type: "SET_PROGRESS", payload: null });
    }
  };

  const selectFile = (id: string) => {
    dispatch({ type: "SELECT_FILE", payload: id });
  };

  const deselectFile = (id: string) => {
    dispatch({ type: "DESELECT_FILE", payload: id });
  };

  const toggleFileSelection = (id: string) => {
    if (state.selectedFiles.includes(id)) {
      deselectFile(id);
    } else {
      selectFile(id);
    }
  };

  const clearSelection = () => {
    dispatch({ type: "CLEAR_SELECTION" });
  };

  const selectAll = () => {
    dispatch({ type: "SELECT_ALL" });
  };

  const value: AudioContextType = {
    state,
    dispatch,
    addFiles,
    removeFile,
    analyzeSelectedFiles,
    processSelectedFiles,
    selectFile,
    deselectFile,
    toggleFileSelection,
    clearSelection,
    selectAll,
  };

  return (
    <AudioContext.Provider value={value}>{children}</AudioContext.Provider>
  );
}

// Custom hook
export function useAudio() {
  const context = useContext(AudioContext);
  if (!context) {
    throw new Error("useAudio must be used within an AudioProvider");
  }
  return context;
}
