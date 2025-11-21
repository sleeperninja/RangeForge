/**
 * Electron API integration for React frontend
 * Handles detection of Electron environment and provides fallbacks
 */

import type { ElectronAPI } from "../types/electron";

/**
 * Check if running in Electron environment
 */
export const isElectron = (): boolean => {
  return typeof window !== "undefined" && window.electronAPI !== undefined;
};

/**
 * Safe wrapper for Electron API calls with fallbacks
 */
export class ElectronBridge {
  /**
   * Check if Electron APIs are available
   */
  static isAvailable(): boolean {
    return isElectron();
  }

  /**
   * Memory map a large audio file (Electron only)
   */
  static async mmapFile(filePath: string): Promise<string | null> {
    if (!isElectron() || !window.electronAPI) {
      console.warn("Memory mapping not available in browser environment");
      return null;
    }

    try {
      const result = await window.electronAPI.mmap.openFile(filePath);
      if (result.success && result.fileId) {
        return result.fileId;
      } else {
        throw new Error(result.error || "Failed to memory map file");
      }
    } catch (error) {
      console.error("Failed to memory map file:", error);
      return null;
    }
  }

  /**
   * Close memory mapped file
   */
  static async closeMmapFile(fileId: string): Promise<boolean> {
    if (!isElectron() || !window.electronAPI) {
      return false;
    }

    try {
      const result = await window.electronAPI.mmap.closeFile(fileId);
      return result.success;
    } catch (error) {
      console.error("Failed to close memory mapped file:", error);
      return false;
    }
  }

  /**
   * Analyze audio segment using memory mapping (Electron only)
   */
  static async analyzeAudioSegment(
    fileId: string,
    startTime: number,
    duration: number
  ): Promise<any> {
    if (!isElectron() || !window.electronAPI) {
      throw new Error("Audio segment analysis requires Electron environment");
    }

    try {
      const result = await window.electronAPI.audio.analyzeSegment(
        fileId,
        startTime,
        duration
      );
      if (result.success) {
        return result.analysis;
      } else {
        throw new Error(result.error || "Failed to analyze audio segment");
      }
    } catch (error) {
      console.error("Audio segment analysis failed:", error);
      throw error;
    }
  }

  /**
   * Process CUE file using Electron backend
   */
  static async processCueFile(cueFilePath: string): Promise<any> {
    if (!isElectron() || !window.electronAPI) {
      console.warn("Using frontend CUE parsing (limited functionality)");
      return null;
    }

    try {
      const result = await window.electronAPI.cue.parseFile(cueFilePath);
      return result;
    } catch (error) {
      console.error("CUE file processing failed:", error);
      return null;
    }
  }

  /**
   * Analyze audio file with full metadata (Electron only)
   */
  static async analyzeAudioFile(filePath: string): Promise<any> {
    if (!isElectron() || !window.electronAPI) {
      throw new Error("Full audio analysis requires Electron environment");
    }

    try {
      const result = await window.electronAPI.audio.analyze(filePath);
      if (result.success) {
        return result.analysis;
      } else {
        throw new Error(result.error || "Failed to analyze audio file");
      }
    } catch (error) {
      console.error("Audio analysis failed:", error);
      throw error;
    }
  }

  /**
   * Show file open dialog (Electron only)
   */
  static async showOpenDialog(options: any): Promise<string[] | null> {
    if (!isElectron() || !window.electronAPI) {
      console.warn(
        "File dialog not available in browser, use drag & drop instead"
      );
      return null;
    }

    try {
      const result = await window.electronAPI.dialog.showOpenDialog(options);
      return result.canceled ? null : result.filePaths;
    } catch (error) {
      console.error("Failed to show open dialog:", error);
      return null;
    }
  }

  /**
   * Listen to Electron events
   */
  static on(channel: string, callback: (...args: any[]) => void): void {
    if (isElectron() && window.electronAPI) {
      window.electronAPI.on(channel, callback);
    }
  }

  /**
   * Remove Electron event listeners
   */
  static removeAllListeners(channel: string): void {
    if (isElectron() && window.electronAPI) {
      window.electronAPI.removeAllListeners(channel);
    }
  }
}

/**
 * Enhanced CUE processing with Electron backend support
 */
export class CueProcessor {
  /**
   * Process CUE file with memory mapping support
   */
  static async processCueWithMemoryMapping(
    cueFile: File,
    audioFile: File,
    onProgress?: (message: string) => void
  ): Promise<any> {
    if (!ElectronBridge.isAvailable()) {
      onProgress?.("⚠️ Running in browser mode - memory mapping unavailable");
      return null;
    }

    try {
      onProgress?.("🗂️ Memory mapping large audio file...");

      // Note: In real Electron environment, we'd have actual file paths
      // For now, this is the structure for when file paths are available
      const audioFilePath = (audioFile as any).path || audioFile.name;
      const cueFilePath = (cueFile as any).path || cueFile.name;

      // Memory map the audio file
      const fileId = await ElectronBridge.mmapFile(audioFilePath);
      if (!fileId) {
        throw new Error("Failed to memory map audio file");
      }

      onProgress?.("📄 Processing CUE file...");

      // Process CUE file
      const cueData = await ElectronBridge.processCueFile(cueFilePath);
      if (!cueData || !cueData.isValid) {
        throw new Error("Failed to parse CUE file");
      }

      onProgress?.("🔍 Analyzing track segments...");

      // Analyze each track segment
      const trackAnalyses = [];
      for (const track of cueData.cueSheet.tracks) {
        onProgress?.(`🎵 Analyzing track ${track.number}: ${track.title}`);

        const startTime = track.startTime;
        const duration = track.endTime ? track.endTime - track.startTime : 300; // Default 5 min

        try {
          const analysis = await ElectronBridge.analyzeAudioSegment(
            fileId,
            startTime,
            duration
          );
          trackAnalyses.push({
            track: track,
            analysis: analysis,
          });
        } catch (error) {
          console.warn(`Failed to analyze track ${track.number}:`, error);
        }
      }

      onProgress?.("✅ CUE processing complete!");

      // Clean up memory mapping
      await ElectronBridge.closeMmapFile(fileId);

      return {
        cueData,
        trackAnalyses,
        totalTracks: cueData.cueSheet.tracks.length,
      };
    } catch (error) {
      onProgress?.(`❌ Error: ${error}`);
      throw error;
    }
  }
}

export default ElectronBridge;
