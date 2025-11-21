import { contextBridge, ipcRenderer, IpcRendererEvent } from "electron";
import type { ElectronAPI } from "../src/types/electron";

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
