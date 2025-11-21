import { CueParser } from "./cueParser";
import { AudioTrack, GenreTag } from "../../src/types/audio";
import * as path from "path";
import * as fs from "fs/promises";

/**
 * CUE Integration Service
 * Bridges CUE parsing with the existing audio track system
 */
export class CueIntegrationService {
  /**
   * Process a CUE file and generate AudioTrack entries for each track
   */
  static async processCueFile(cueFilePath: string): Promise<{
    success: boolean;
    tracks: AudioTrack[];
    error?: string;
  }> {
    try {
      // Parse the CUE file
      const parseResult = await CueParser.parseCueFile(cueFilePath);
      if (
        !parseResult.isValid ||
        !parseResult.cueSheet ||
        !parseResult.audioFilePath
      ) {
        return {
          success: false,
          tracks: [],
          error: parseResult.error || "Failed to parse CUE file",
        };
      }

      // Verify audio file exists
      try {
        await fs.access(parseResult.audioFilePath);
      } catch {
        return {
          success: false,
          tracks: [],
          error: `Audio file not found: ${parseResult.audioFilePath}`,
        };
      }

      // Get audio file stats for size
      const audioStats = await fs.stat(parseResult.audioFilePath);

      // Create AudioTrack entries for each CUE track
      const tracks: AudioTrack[] = parseResult.cueSheet.tracks.map(
        (cueTrack, index) => {
          const nextTrack = parseResult.cueSheet!.tracks[index + 1];
          const duration = nextTrack
            ? nextTrack.indexTimeSeconds - cueTrack.indexTimeSeconds
            : undefined; // Will need to get total duration from audio file

          // Generate unique ID for this CUE track
          const trackId = `cue_${path.basename(
            cueFilePath,
            ".cue"
          )}_${cueTrack.trackNumber.toString().padStart(2, "0")}`;

          // Create display name
          let trackName = cueTrack.title;
          if (
            cueTrack.performer &&
            cueTrack.performer !== parseResult.cueSheet!.performer
          ) {
            trackName = `${cueTrack.performer} - ${cueTrack.title}`;
          }

          // Determine codec from audio file extension
          const audioExt = path
            .extname(parseResult.audioFilePath!)
            .toLowerCase();
          const codecMap: Record<string, string> = {
            ".flac": "FLAC",
            ".wav": "WAV",
            ".ape": "APE",
            ".wv": "WavPack",
            ".m4a": "ALAC/AAC",
            ".mp3": "MP3",
          };
          const codec = codecMap[audioExt] || "Unknown";

          // Generate some basic genre tags based on album info
          const tags: GenreTag[] = [];
          if (parseResult.cueSheet!.performer) {
            tags.push({
              name: parseResult.cueSheet!.performer,
              color: "blue",
              source: "metadata",
            });
          }

          return {
            id: trackId,
            name: trackName,
            path: `${parseResult.audioFilePath}#${cueTrack.trackNumber}`, // Virtual path indicating CUE track
            codec,
            sampleRate: undefined, // Will be filled by audio analysis
            bitDepth: undefined,
            channels: undefined,
            duration,
            fileSize: Math.round(
              audioStats.size / parseResult.cueSheet!.tracks.length
            ), // Approximate size per track
            analysis: undefined,
            tags,
            isProcessing: false,
            // CUE-specific fields
            isCueTrack: true,
            cueTrackNumber: cueTrack.trackNumber,
            cueStartTime: cueTrack.indexTimeSeconds,
            cueDuration: duration,
            parentCueFile: cueFilePath,
            parentAudioFile: parseResult.audioFilePath,
          };
        }
      );

      return {
        success: true,
        tracks,
      };
    } catch (error) {
      return {
        success: false,
        tracks: [],
        error: `Failed to process CUE file: ${
          error instanceof Error ? error.message : "Unknown error"
        }`,
      };
    }
  }

  /**
   * Check if a file drop contains CUE files and process them
   */
  static async detectAndProcessCueFiles(filePaths: string[]): Promise<{
    cueFiles: string[];
    audioFiles: string[];
    cueTrackSets: AudioTrack[][];
  }> {
    const cueFiles: string[] = [];
    const audioFiles: string[] = [];
    const cueTrackSets: AudioTrack[][] = [];

    // Separate CUE files from audio files
    for (const filePath of filePaths) {
      if (CueParser.isCueFile(filePath)) {
        cueFiles.push(filePath);
      } else if (this.isAudioFile(filePath)) {
        audioFiles.push(filePath);
      }
    }

    // Process each CUE file
    for (const cueFile of cueFiles) {
      const result = await this.processCueFile(cueFile);
      if (result.success) {
        cueTrackSets.push(result.tracks);
      }
    }

    return {
      cueFiles,
      audioFiles,
      cueTrackSets,
    };
  }

  /**
   * Check if a file is a supported audio format
   */
  private static isAudioFile(filePath: string): boolean {
    const audioExtensions = [
      ".flac",
      ".wav",
      ".ape",
      ".wv",
      ".m4a",
      ".mp3",
      ".aac",
      ".ogg",
      ".opus",
    ];
    const ext = path.extname(filePath).toLowerCase();
    return audioExtensions.includes(ext);
  }

  /**
   * Get splitting parameters for FFmpeg to extract a CUE track
   */
  static getCueTrackSplitParams(track: AudioTrack): {
    startTime: string;
    duration?: string;
    inputFile: string;
  } | null {
    if (
      !track.isCueTrack ||
      !track.parentAudioFile ||
      track.cueStartTime === undefined
    ) {
      return null;
    }

    const startTime = this.secondsToTimeString(track.cueStartTime);
    const duration = track.cueDuration
      ? this.secondsToTimeString(track.cueDuration)
      : undefined;

    return {
      startTime,
      duration,
      inputFile: track.parentAudioFile,
    };
  }

  /**
   * Convert seconds to FFmpeg time format (HH:MM:SS.mmm)
   */
  private static secondsToTimeString(seconds: number): string {
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    const remainingSeconds = seconds % 60;

    return `${hours.toString().padStart(2, "0")}:${minutes
      .toString()
      .padStart(2, "0")}:${remainingSeconds.toFixed(3).padStart(6, "0")}`;
  }

  /**
   * Generate output filename for a CUE track
   */
  static generateCueTrackFilename(
    track: AudioTrack,
    outputFormat: string
  ): string {
    if (!track.isCueTrack || !track.cueTrackNumber) {
      return track.name;
    }

    const trackNumber = track.cueTrackNumber.toString().padStart(2, "0");
    const safeName = track.name.replace(/[<>:"/\\|?*]/g, "_"); // Remove invalid filename characters

    return `${trackNumber} - ${safeName}.${outputFormat}`;
  }
}
