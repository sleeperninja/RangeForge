// Global type declarations for Electron API

interface ElectronAPI {
  selectAudioFiles: () => Promise<string[]>;
  analyzeAudio: (filePath: string) => Promise<AudioAnalysisResult>;
  processFiles: (filePaths: string[], options: any) => Promise<any>;
  mmapFile: (filePath: string) => Promise<any>;

  // Event listeners
  onProcessingProgress: (callback: (data: any) => void) => void;
  onAnalysisComplete: (callback: (data: any) => void) => void;
  removeAllListeners: (channel: string) => void;
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
