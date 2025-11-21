import { promises as fs } from "fs";
import * as path from "path";
import { CueSheet, CueTrack, CueParseResult } from "../../src/types/cue";

/**
 * CUE File Parser Service
 * Handles parsing CUE sheet files and extracting track information
 * for album splitting operations
 */
export class CueParser {
  /**
   * Parse a CUE file from file path
   */
  static async parseCueFile(cueFilePath: string): Promise<CueParseResult> {
    try {
      const cueContent = await fs.readFile(cueFilePath, "utf-8");
      return this.parseCueContent(cueContent, path.dirname(cueFilePath));
    } catch (error) {
      return {
        isValid: false,
        error: `Failed to read CUE file: ${
          error instanceof Error ? error.message : "Unknown error"
        }`,
      };
    }
  }

  /**
   * Parse CUE content from string
   */
  static parseCueContent(cueContent: string, basePath: string): CueParseResult {
    try {
      const lines = cueContent.split("\n").map((line) => line.trim());
      const cueSheet: Partial<CueSheet> = {
        tracks: [],
      };

      let currentTrack: Partial<CueTrack> | null = null;
      let insideTrack = false;

      for (const line of lines) {
        // Skip empty lines and comments
        if (!line || line.startsWith("REM")) continue;

        // Parse global information
        if (line.startsWith("PERFORMER ")) {
          const performer = this.extractQuotedString(line);
          if (!insideTrack) {
            cueSheet.performer = performer;
          } else if (currentTrack) {
            currentTrack.performer = performer;
          }
        } else if (line.startsWith("TITLE ")) {
          const title = this.extractQuotedString(line);
          if (!insideTrack) {
            cueSheet.title = title;
          } else if (currentTrack) {
            currentTrack.title = title;
          }
        } else if (line.startsWith("FILE ")) {
          const fileMatch = line.match(/FILE\s+"(.+)"\s+(\w+)/);
          if (fileMatch) {
            cueSheet.file = fileMatch[1];
            cueSheet.fileType = fileMatch[2];
          }
        } else if (line.startsWith("TRACK ")) {
          // Save previous track
          if (currentTrack && currentTrack.trackNumber && currentTrack.title) {
            cueSheet.tracks!.push(currentTrack as CueTrack);
          }

          // Start new track
          const trackMatch = line.match(/TRACK\s+(\d+)\s+AUDIO/);
          if (trackMatch) {
            currentTrack = {
              trackNumber: parseInt(trackMatch[1], 10),
              indexTimeSeconds: 0,
            };
            insideTrack = true;
          }
        } else if (line.startsWith("INDEX 01 ") && currentTrack) {
          const timeMatch = line.match(/INDEX\s+01\s+(\d+):(\d+):(\d+)/);
          if (timeMatch) {
            const minutes = parseInt(timeMatch[1], 10);
            const seconds = parseInt(timeMatch[2], 10);
            const frames = parseInt(timeMatch[3], 10);

            currentTrack.indexTime = `${timeMatch[1]}:${timeMatch[2]}:${timeMatch[3]}`;
            // Convert to total seconds (75 frames per second in CD audio)
            currentTrack.indexTimeSeconds =
              minutes * 60 + seconds + frames / 75;
          }
        }
      }

      // Add the last track
      if (currentTrack && currentTrack.trackNumber && currentTrack.title) {
        cueSheet.tracks!.push(currentTrack as CueTrack);
      }

      // Validate required fields
      if (!cueSheet.file || !cueSheet.tracks || cueSheet.tracks.length === 0) {
        return {
          isValid: false,
          error: "Invalid CUE file: missing file reference or tracks",
        };
      }

      // Resolve audio file path
      const audioFilePath = path.resolve(basePath, cueSheet.file);

      return {
        isValid: true,
        cueSheet: cueSheet as CueSheet,
        audioFilePath,
      };
    } catch (error) {
      return {
        isValid: false,
        error: `Failed to parse CUE content: ${
          error instanceof Error ? error.message : "Unknown error"
        }`,
      };
    }
  }

  /**
   * Extract quoted string from CUE line
   */
  private static extractQuotedString(line: string): string {
    const match = line.match(/"(.+)"/);
    return match ? match[1] : "";
  }

  /**
   * Check if a file path appears to be a CUE file
   */
  static isCueFile(filePath: string): boolean {
    return path.extname(filePath).toLowerCase() === ".cue";
  }

  /**
   * Find potential audio files that match a CUE sheet
   */
  static async findMatchingAudioFile(
    cueFilePath: string
  ): Promise<string | null> {
    try {
      const parseResult = await this.parseCueFile(cueFilePath);
      if (!parseResult.isValid || !parseResult.audioFilePath) {
        return null;
      }

      // Check if the specified audio file exists
      try {
        await fs.access(parseResult.audioFilePath);
        return parseResult.audioFilePath;
      } catch {
        // If not found, try common audio extensions in the same directory
        const baseName = path.basename(cueFilePath, ".cue");
        const directory = path.dirname(cueFilePath);
        const commonExtensions = [
          ".flac",
          ".wav",
          ".ape",
          ".wv",
          ".m4a",
          ".mp3",
        ];

        for (const ext of commonExtensions) {
          const candidatePath = path.join(directory, baseName + ext);
          try {
            await fs.access(candidatePath);
            return candidatePath;
          } catch {
            // Continue searching
          }
        }
      }

      return null;
    } catch (error) {
      console.error("Error finding matching audio file:", error);
      return null;
    }
  }

  /**
   * Get track duration in seconds (approximate, based on next track start or file end)
   */
  static getTrackDuration(
    track: CueTrack,
    nextTrack: CueTrack | null,
    totalDuration?: number
  ): number {
    if (nextTrack) {
      return nextTrack.indexTimeSeconds - track.indexTimeSeconds;
    } else if (totalDuration) {
      return totalDuration - track.indexTimeSeconds;
    }
    return 0; // Unknown duration
  }

  /**
   * Format time in MM:SS format for display
   */
  static formatTime(seconds: number): string {
    const minutes = Math.floor(seconds / 60);
    const remainingSeconds = Math.floor(seconds % 60);
    return `${minutes}:${remainingSeconds.toString().padStart(2, "0")}`;
  }
}
