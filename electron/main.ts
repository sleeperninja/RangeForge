import { app, BrowserWindow, ipcMain, dialog } from "electron";
import * as path from "path";
import * as isDev from "electron-is-dev";
import { Worker } from "worker_threads";
import { AudioProcessor } from "./services/audioProcessor";
import { FileProcessor } from "./services/fileProcessor";

class MainProcess {
  private mainWindow: BrowserWindow | null = null;
  private audioProcessor: AudioProcessor;
  private fileProcessor: FileProcessor;

  constructor() {
    this.audioProcessor = new AudioProcessor();
    this.fileProcessor = new FileProcessor();
    this.setupEventHandlers();
  }

  private setupEventHandlers(): void {
    app.whenReady().then(() => {
      this.createWindow();
      this.setupIpcHandlers();

      app.on("activate", () => {
        if (BrowserWindow.getAllWindows().length === 0) {
          this.createWindow();
        }
      });
    });

    app.on("window-all-closed", () => {
      if (process.platform !== "darwin") {
        app.quit();
      }
    });
  }

  private createWindow(): void {
    this.mainWindow = new BrowserWindow({
      width: 1200,
      height: 800,
      webPreferences: {
        nodeIntegration: false,
        contextIsolation: true,
        enableRemoteModule: false,
        preload: path.join(__dirname, "preload.js"),
      },
      show: false,
    });

    const startUrl = isDev
      ? "http://localhost:3000"
      : `file://${path.join(__dirname, "../build/index.html")}`;

    this.mainWindow.loadURL(startUrl);

    if (isDev) {
      this.mainWindow.webContents.openDevTools();
    }

    this.mainWindow.once("ready-to-show", () => {
      this.mainWindow?.show();
    });

    this.mainWindow.on("closed", () => {
      this.mainWindow = null;
    });
  }

  private setupIpcHandlers(): void {
    // File selection handler
    ipcMain.handle("select-audio-files", async () => {
      const result = await dialog.showOpenDialog(this.mainWindow!, {
        properties: ["openFile", "multiSelections"],
        filters: [
          {
            name: "Audio Files",
            extensions: ["mp3", "wav", "flac", "m4a", "aac", "ogg"],
          },
        ],
      });
      return result.filePaths;
    });

    // Audio analysis handler
    ipcMain.handle("analyze-audio", async (event, filePath: string) => {
      try {
        return await this.audioProcessor.analyzeFile(filePath);
      } catch (error) {
        console.error("Audio analysis failed:", error);
        throw error;
      }
    });

    // Batch processing handler
    ipcMain.handle(
      "process-files",
      async (event, filePaths: string[], options: any) => {
        try {
          return await this.fileProcessor.processBatch(filePaths, options);
        } catch (error) {
          console.error("Batch processing failed:", error);
          throw error;
        }
      }
    );

    // Memory-mapped file operations
    ipcMain.handle("mmap-file", async (event, filePath: string) => {
      try {
        return await this.fileProcessor.mapFile(filePath);
      } catch (error) {
        console.error("Memory mapping failed:", error);
        throw error;
      }
    });

    // Progress updates
    this.fileProcessor.on("progress", (data) => {
      this.mainWindow?.webContents.send("processing-progress", data);
    });

    this.audioProcessor.on("analysis-complete", (data) => {
      this.mainWindow?.webContents.send("analysis-complete", data);
    });
  }
}

// Initialize the main process
new MainProcess();
