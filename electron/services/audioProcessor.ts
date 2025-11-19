import * as ffmpeg from "fluent-ffmpeg";
import { EventEmitter } from "events";
import * as path from "path";
import * as fs from "fs";
import { Worker } from "worker_threads";

export interface AudioAnalysisResult {
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

export interface VolumeAdjustmentOptions {
  targetLUFS?: number;
  normalize?: boolean;
  limiterThreshold?: number;
  outputPath?: string;
}

export class AudioProcessor extends EventEmitter {
  private workers: Map<string, Worker> = new Map();

  constructor() {
    super();
  }

  /**
   * Analyze an audio file for volume levels and dynamic range
   */
  async analyzeFile(filePath: string): Promise<AudioAnalysisResult> {
    return new Promise((resolve, reject) => {
      const result: Partial<AudioAnalysisResult> = {
        filePath,
        volumeStats: {},
        dynamicRange: {},
        metadata: {},
      } as AudioAnalysisResult;

      // Get basic metadata first
      ffmpeg.ffprobe(filePath, (err, metadata) => {
        if (err) {
          reject(new Error(`Failed to probe file: ${err.message}`));
          return;
        }

        const audioStream = metadata.streams.find(
          (stream) => stream.codec_type === "audio"
        );
        if (!audioStream) {
          reject(new Error("No audio stream found in file"));
          return;
        }

        result.duration = metadata.format.duration || 0;
        result.metadata = {
          format: metadata.format.format_name || "unknown",
          bitrate: parseInt(metadata.format.bit_rate || "0"),
          sampleRate: audioStream.sample_rate || 0,
          channels: audioStream.channels || 0,
        };

        // Analyze volume levels using ffmpeg volumedetect filter
        this.analyzeVolume(filePath)
          .then((volumeStats) => {
            result.volumeStats = volumeStats;
            return this.analyzeDynamicRange(filePath);
          })
          .then((dynamicRange) => {
            result.dynamicRange = dynamicRange;
            this.emit("analysis-complete", result);
            resolve(result as AudioAnalysisResult);
          })
          .catch(reject);
      });
    });
  }

  /**
   * Analyze volume statistics using FFmpeg volumedetect filter
   */
  private analyzeVolume(
    filePath: string
  ): Promise<AudioAnalysisResult["volumeStats"]> {
    return new Promise((resolve, reject) => {
      let volumeData = "";

      const command = ffmpeg(filePath)
        .audioFilters("volumedetect")
        .format("null")
        .output("-")
        .on("stderr", (stderrLine) => {
          volumeData += stderrLine + "\n";
        })
        .on("end", () => {
          try {
            const stats = this.parseVolumeData(volumeData);
            resolve(stats);
          } catch (error) {
            reject(error);
          }
        })
        .on("error", (err) => {
          reject(new Error(`Volume analysis failed: ${err.message}`));
        });

      command.run();
    });
  }

  /**
   * Analyze dynamic range using EBU R128 loudness measurement
   */
  private analyzeDynamicRange(
    filePath: string
  ): Promise<AudioAnalysisResult["dynamicRange"]> {
    return new Promise((resolve, reject) => {
      let loudnessData = "";

      const command = ffmpeg(filePath)
        .audioFilters("ebur128=framelog=verbose")
        .format("null")
        .output("-")
        .on("stderr", (stderrLine) => {
          loudnessData += stderrLine + "\n";
        })
        .on("end", () => {
          try {
            const dr = this.calculateDynamicRange(loudnessData);
            resolve(dr);
          } catch (error) {
            reject(error);
          }
        })
        .on("error", (err) => {
          reject(new Error(`Dynamic range analysis failed: ${err.message}`));
        });

      command.run();
    });
  }

  /**
   * Adjust volume of an audio file
   */
  async adjustVolume(
    filePath: string,
    options: VolumeAdjustmentOptions
  ): Promise<string> {
    return new Promise((resolve, reject) => {
      const outputPath =
        options.outputPath || this.generateOutputPath(filePath);

      let command = ffmpeg(filePath);

      // Apply volume adjustments
      if (options.normalize) {
        command = command.audioFilters("loudnorm");
      }

      if (options.targetLUFS) {
        command = command.audioFilters(`loudnorm=I=${options.targetLUFS}`);
      }

      if (options.limiterThreshold) {
        command = command.audioFilters(
          `alimiter=limit=${options.limiterThreshold}`
        );
      }

      command
        .output(outputPath)
        .on("progress", (progress) => {
          this.emit("progress", {
            file: filePath,
            percent: progress.percent || 0,
            currentKbps: progress.currentKbps,
            targetSize: progress.targetSize,
            timemark: progress.timemark,
          });
        })
        .on("end", () => {
          resolve(outputPath);
        })
        .on("error", (err) => {
          reject(new Error(`Volume adjustment failed: ${err.message}`));
        })
        .run();
    });
  }

  /**
   * Process multiple files using worker threads
   */
  async processBatch(filePaths: string[], options: any): Promise<void> {
    const workerPromises = filePaths.map((filePath, index) => {
      return this.processFileInWorker(filePath, options, index);
    });

    await Promise.all(workerPromises);
  }

  /**
   * Process a single file in a worker thread
   */
  private processFileInWorker(
    filePath: string,
    options: any,
    workerId: number
  ): Promise<void> {
    return new Promise((resolve, reject) => {
      const workerPath = path.join(
        __dirname,
        "../../dist/workers/audioWorker.js"
      );
      const worker = new Worker(workerPath, {
        workerData: { filePath, options },
      });

      this.workers.set(`worker-${workerId}`, worker);

      worker.on("message", (message) => {
        if (message.type === "progress") {
          this.emit("progress", message.data);
        } else if (message.type === "complete") {
          resolve();
        } else if (message.type === "error") {
          reject(new Error(message.error));
        }
      });

      worker.on("error", reject);
      worker.on("exit", (code) => {
        this.workers.delete(`worker-${workerId}`);
        if (code !== 0) {
          reject(new Error(`Worker stopped with exit code ${code}`));
        }
      });
    });
  }

  /**
   * Parse volume detection output from FFmpeg
   */
  private parseVolumeData(
    volumeData: string
  ): AudioAnalysisResult["volumeStats"] {
    const meanVolumeMatch = volumeData.match(/mean_volume: ([-\d.]+) dB/);
    const maxVolumeMatch = volumeData.match(/max_volume: ([-\d.]+) dB/);

    return {
      meanVolume: meanVolumeMatch ? parseFloat(meanVolumeMatch[1]) : 0,
      maxVolume: maxVolumeMatch ? parseFloat(maxVolumeMatch[1]) : 0,
      minVolume: 0, // Will need additional processing for this
      rms: 0, // Will need additional processing for this
      peak: maxVolumeMatch ? parseFloat(maxVolumeMatch[1]) : 0,
    };
  }

  /**
   * Calculate dynamic range from EBU R128 data
   */
  private calculateDynamicRange(
    loudnessData: string
  ): AudioAnalysisResult["dynamicRange"] {
    // This is a simplified calculation - you may want to implement
    // a more sophisticated DR measurement algorithm
    const lufsMatches = loudnessData.match(/I:\s*([-\d.]+)/g);

    if (!lufsMatches || lufsMatches.length === 0) {
      return { dr: 0, drMeasurement: "N/A" };
    }

    const lufsValues = lufsMatches.map((match) => {
      const value = match.match(/([-\d.]+)/);
      return value ? parseFloat(value[1]) : 0;
    });

    const maxLufs = Math.max(...lufsValues);
    const minLufs = Math.min(...lufsValues);
    const dr = maxLufs - minLufs;

    let drMeasurement = "Poor";
    if (dr >= 20) drMeasurement = "Excellent";
    else if (dr >= 14) drMeasurement = "Good";
    else if (dr >= 8) drMeasurement = "Fair";

    return { dr, drMeasurement };
  }

  /**
   * Generate output file path for processed audio
   */
  private generateOutputPath(inputPath: string): string {
    const parsed = path.parse(inputPath);
    return path.join(parsed.dir, `${parsed.name}_processed${parsed.ext}`);
  }

  /**
   * Clean up all workers
   */
  dispose(): void {
    this.workers.forEach((worker) => {
      worker.terminate();
    });
    this.workers.clear();
  }
}
