// Global type declarations for Electron API

export interface ElectronAPI {
  // File system operations
  fs: {
    readFile: (
      filePath: string
    ) => Promise<{ success: boolean; content?: string; error?: string }>;
    stat: (
      filePath: string
    ) => Promise<{ success: boolean; stats?: any; error?: string }>;
  };

  // Memory mapping operations
  mmap: {
    openFile: (filePath: string) => Promise<{
      success: boolean;
      fileId?: string;
      size?: number;
      error?: string;
    }>;
    closeFile: (
      fileId: string
    ) => Promise<{ success: boolean; error?: string }>;
    readSegment: (
      fileId: string,
      start: number,
      length: number
    ) => Promise<{ success: boolean; data?: any; error?: string }>;
  };

  // CUE file operations
  cue: {
    parseFile: (cueFilePath: string) => Promise<any>;
    detectAndProcess: (
      filePaths: string[]
    ) => Promise<{ success: boolean; result?: any; error?: string }>;
  };

  // Audio processing operations
  audio: {
    analyze: (
      filePath: string
    ) => Promise<{ success: boolean; analysis?: any; error?: string }>;
    analyzeSegment: (
      fileId: string,
      startTime: number,
      duration: number
    ) => Promise<{ success: boolean; analysis?: any; error?: string }>;
    processTrack: (
      options: any
    ) => Promise<{ success: boolean; result?: any; error?: string }>;
  };

  // Dialog operations
  dialog: {
    showOpenDialog: (options: any) => Promise<any>;
    showSaveDialog: (options: any) => Promise<any>;
  };

  // Event listeners
  on: (channel: string, func: (...args: any[]) => void) => void;
  removeAllListeners: (channel: string) => void;
}

// Global window interface extension
declare global {
  interface Window {
    electronAPI?: ElectronAPI;
  }
}

interface AudioAnalysisResult {
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

declare global {
  interface Window {
    electronAPI: ElectronAPI;
  }
}

export {};
