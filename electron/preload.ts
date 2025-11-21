import { contextBridge, ipcRenderer, IpcRendererEvent } from "electron";

// Define the API interface
interface ElectronAPI {
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
    openFile: (
      filePath: string
    ) => Promise<{
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

// Expose the API to the renderer process
const electronAPI: ElectronAPI = {
  fs: {
    readFile: (filePath: string) => ipcRenderer.invoke("fs:readFile", filePath),
    stat: (filePath: string) => ipcRenderer.invoke("fs:stat", filePath),
  },

  mmap: {
    openFile: (filePath: string) =>
      ipcRenderer.invoke("mmap:openFile", filePath),
    closeFile: (fileId: string) => ipcRenderer.invoke("mmap:closeFile", fileId),
    readSegment: (fileId: string, start: number, length: number) =>
      ipcRenderer.invoke("mmap:readSegment", fileId, start, length),
  },

  cue: {
    parseFile: (cueFilePath: string) =>
      ipcRenderer.invoke("cue:parseFile", cueFilePath),
    detectAndProcess: (filePaths: string[]) =>
      ipcRenderer.invoke("cue:detectAndProcess", filePaths),
  },

  audio: {
    analyze: (filePath: string) =>
      ipcRenderer.invoke("audio:analyze", filePath),
    analyzeSegment: (fileId: string, startTime: number, duration: number) =>
      ipcRenderer.invoke("audio:analyzeSegment", fileId, startTime, duration),
    processTrack: (options: any) =>
      ipcRenderer.invoke("audio:processTrack", options),
  },

  dialog: {
    showOpenDialog: (options: any) =>
      ipcRenderer.invoke("dialog:showOpenDialog", options),
    showSaveDialog: (options: any) =>
      ipcRenderer.invoke("dialog:showSaveDialog", options),
  },

  on: (channel: string, func: (...args: any[]) => void) => {
    const validChannels = [
      "files:opened",
      "processing:progress",
      "processing:complete",
    ];
    if (validChannels.includes(channel)) {
      ipcRenderer.on(channel, (_event: IpcRendererEvent, ...args: any[]) =>
        func(...args)
      );
    }
  },

  removeAllListeners: (channel: string) => {
    ipcRenderer.removeAllListeners(channel);
  },
};

// Expose the API
contextBridge.exposeInMainWorld("electronAPI", electronAPI);

// Type declaration for TypeScript
declare global {
  interface Window {
    electronAPI: ElectronAPI;
  }
}
