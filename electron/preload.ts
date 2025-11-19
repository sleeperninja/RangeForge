import { contextBridge, ipcRenderer } from "electron";

// Define the API interface
interface ElectronAPI {
  selectAudioFiles: () => Promise<string[]>;
  analyzeAudio: (filePath: string) => Promise<any>;
  processFiles: (filePaths: string[], options: any) => Promise<any>;
  mmapFile: (filePath: string) => Promise<any>;

  // Event listeners
  onProcessingProgress: (callback: (data: any) => void) => void;
  onAnalysisComplete: (callback: (data: any) => void) => void;
  removeAllListeners: (channel: string) => void;
}

// Expose protected methods that allow the renderer process to use
// the ipcRenderer without exposing the entire object
contextBridge.exposeInMainWorld("electronAPI", {
  selectAudioFiles: () => ipcRenderer.invoke("select-audio-files"),
  analyzeAudio: (filePath: string) =>
    ipcRenderer.invoke("analyze-audio", filePath),
  processFiles: (filePaths: string[], options: any) =>
    ipcRenderer.invoke("process-files", filePaths, options),
  mmapFile: (filePath: string) => ipcRenderer.invoke("mmap-file", filePath),

  // Event listeners
  onProcessingProgress: (callback: (data: any) => void) => {
    ipcRenderer.on("processing-progress", (event, data) => callback(data));
  },
  onAnalysisComplete: (callback: (data: any) => void) => {
    ipcRenderer.on("analysis-complete", (event, data) => callback(data));
  },
  removeAllListeners: (channel: string) => {
    ipcRenderer.removeAllListeners(channel);
  },
} as ElectronAPI);

// Type declaration for TypeScript
declare global {
  interface Window {
    electronAPI: ElectronAPI;
  }
}
