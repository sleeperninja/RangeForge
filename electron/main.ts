import { app, BrowserWindow, ipcMain, Menu, dialog } from "electron";
import { join } from "path";
import * as fs from "fs";
import { CueParser } from "./services/cueParser";
import { CueIntegrationService } from "./services/cueIntegration";
import { AudioProcessor } from "./services/audioProcessor";
import { MemoryMappedAudioFile } from "./services/mmapAudioFile";

// Get directory name for CommonJS compatibility
const __dirname = __filename ? join(__filename, "..") : process.cwd();

class ElectronApp {
  private mainWindow: BrowserWindow | null = null;
  private isDevelopment = process.env.NODE_ENV === "development";

  constructor() {
    this.setupApp();
    this.setupIPC();
  }

  private setupApp() {
    // Handle app ready
    app.whenReady().then(() => {
      this.createWindow();
      this.setupMenu();

      app.on("activate", () => {
        if (BrowserWindow.getAllWindows().length === 0) {
          this.createWindow();
        }
      });
    });

    // Handle app quit
    app.on("window-all-closed", () => {
      if (process.platform !== "darwin") {
        app.quit();
      }
    });
  }

  private createWindow() {
    this.mainWindow = new BrowserWindow({
      width: 1400,
      height: 900,
      minWidth: 800,
      minHeight: 600,
      show: false,
      icon: join(__dirname, "../assets/icon.png"),
      webPreferences: {
        nodeIntegration: false,
        contextIsolation: true,
        preload: join(__dirname, "preload.js"),
      },
    });

    // Load the app
    if (this.isDevelopment) {
      this.mainWindow.loadURL("http://localhost:3000");
      this.mainWindow.webContents.openDevTools();
    } else {
      this.mainWindow.loadFile(join(__dirname, "../dist/index.html"));
    }

    // Show when ready
    this.mainWindow.once("ready-to-show", () => {
      this.mainWindow?.show();

      if (this.isDevelopment) {
        this.mainWindow?.webContents.openDevTools();
      }
    });

    // Handle window closed
    this.mainWindow.on("closed", () => {
      this.mainWindow = null;
    });
  }

  private setupMenu() {
    const template: Electron.MenuItemConstructorOptions[] = [
      {
        label: "File",
        submenu: [
          {
            label: "Open Audio Files",
            accelerator: "CmdOrCtrl+O",
            click: async () => {
              const result = await dialog.showOpenDialog(this.mainWindow!, {
                properties: ["openFile", "multiSelections"],
                filters: [
                  {
                    name: "Audio Files",
                    extensions: ["flac", "wav", "ape", "m4a", "mp3", "ogg"],
                  },
                  { name: "CUE Files", extensions: ["cue"] },
                  { name: "All Files", extensions: ["*"] },
                ],
              });

              if (!result.canceled) {
                this.handleFilesOpened(result.filePaths);
              }
            },
          },
          { type: "separator" },
          {
            label: "Exit",
            accelerator: process.platform === "darwin" ? "Cmd+Q" : "Ctrl+Q",
            click: () => {
              app.quit();
            },
          },
        ],
      },
      {
        label: "View",
        submenu: [
          { role: "reload" },
          { role: "forceReload" },
          { role: "toggleDevTools" },
          { type: "separator" },
          { role: "resetZoom" },
          { role: "zoomIn" },
          { role: "zoomOut" },
          { type: "separator" },
          { role: "togglefullscreen" },
        ],
      },
    ];

    const menu = Menu.buildFromTemplate(template);
    Menu.setApplicationMenu(menu);
  }

  private setupIPC() {
    // File system operations
    ipcMain.handle("fs:readFile", async (_, filePath: string) => {
      try {
        const content = await fs.promises.readFile(filePath, "utf-8");
        return { success: true, content };
      } catch (error) {
        return {
          success: false,
          error: error instanceof Error ? error.message : "Unknown error",
        };
      }
    });

    ipcMain.handle("fs:stat", async (_, filePath: string) => {
      try {
        const stats = await fs.promises.stat(filePath);
        return {
          success: true,
          stats: {
            size: stats.size,
            isFile: stats.isFile(),
            isDirectory: stats.isDirectory(),
            mtime: stats.mtime,
            ctime: stats.ctime,
          },
        };
      } catch (error) {
        return {
          success: false,
          error: error instanceof Error ? error.message : "Unknown error",
        };
      }
    });

    // Memory mapping operations
    ipcMain.handle("mmap:openFile", async (_, filePath: string) => {
      try {
        const mmapFile = new MemoryMappedAudioFile(filePath);
        await mmapFile.open();
        return {
          success: true,
          fileId: mmapFile.getId(),
          size: mmapFile.getSize(),
        };
      } catch (error) {
        return {
          success: false,
          error: error instanceof Error ? error.message : "Unknown error",
        };
      }
    });

    ipcMain.handle("mmap:closeFile", async (_, fileId: string) => {
      try {
        MemoryMappedAudioFile.close(fileId);
        return { success: true };
      } catch (error) {
        return {
          success: false,
          error: error instanceof Error ? error.message : "Unknown error",
        };
      }
    });

    ipcMain.handle(
      "mmap:readSegment",
      async (_, fileId: string, start: number, length: number) => {
        try {
          const data = MemoryMappedAudioFile.readSegment(fileId, start, length);
          return { success: true, data };
        } catch (error) {
          return {
            success: false,
            error: error instanceof Error ? error.message : "Unknown error",
          };
        }
      }
    );

    // CUE file operations
    ipcMain.handle("cue:parseFile", async (_, cueFilePath: string) => {
      try {
        const result = await CueParser.parseCueFile(cueFilePath);
        return result;
      } catch (error) {
        return {
          isValid: false,
          error: error instanceof Error ? error.message : "Unknown error",
        };
      }
    });

    ipcMain.handle("cue:detectAndProcess", async (_, filePaths: string[]) => {
      try {
        const allTracks: any[] = [];
        const cueFiles = filePaths.filter((path) =>
          path.toLowerCase().endsWith(".cue")
        );

        for (const cueFile of cueFiles) {
          const result = await CueIntegrationService.processCueFile(cueFile);
          if (result.success) {
            allTracks.push(...result.tracks);
          }
        }

        return {
          success: allTracks.length > 0,
          tracks: allTracks,
          error:
            allTracks.length === 0 ? "No valid CUE tracks found" : undefined,
        };
      } catch (error) {
        return {
          success: false,
          tracks: [],
          error: error instanceof Error ? error.message : "Unknown error",
        };
      }
    });

    // Audio processing operations
    ipcMain.handle("audio:analyze", async (_, filePath: string) => {
      try {
        const processor = new AudioProcessor();
        const analysis = await processor.analyzeFile(filePath);
        return { success: true, analysis };
      } catch (error) {
        return {
          success: false,
          error: error instanceof Error ? error.message : "Unknown error",
        };
      }
    });

    ipcMain.handle(
      "audio:analyzeSegment",
      async (_, fileId: string, startTime: number, duration: number) => {
        try {
          const processor = new AudioProcessor();
          const analysis = await processor.analyzeMemorySegment(
            fileId,
            startTime,
            duration
          );
          return { success: true, analysis };
        } catch (error) {
          return {
            success: false,
            error: error instanceof Error ? error.message : "Unknown error",
          };
        }
      }
    );

    ipcMain.handle("audio:processTrack", async (_, options: any) => {
      try {
        const processor = new AudioProcessor();
        const result = await processor.processTrack(options);
        return { success: true, result };
      } catch (error) {
        return {
          success: false,
          error: error instanceof Error ? error.message : "Unknown error",
        };
      }
    });

    // Dialog operations
    ipcMain.handle("dialog:showOpenDialog", async (_, options: any) => {
      const result = await dialog.showOpenDialog(this.mainWindow!, options);
      return result;
    });

    ipcMain.handle("dialog:showSaveDialog", async (_, options: any) => {
      const result = await dialog.showSaveDialog(this.mainWindow!, options);
      return result;
    });
  }

  private async handleFilesOpened(filePaths: string[]) {
    // Send file paths to renderer process
    this.mainWindow?.webContents.send("files:opened", filePaths);
  }
}

// Create the application
new ElectronApp();

// Handle protocol for development
if (process.defaultApp) {
  if (process.argv.length >= 2) {
    app.setAsDefaultProtocolClient("rangeforge", process.execPath, [
      join(__dirname, "../"),
    ]);
  }
} else {
  app.setAsDefaultProtocolClient("rangeforge");
}
