"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.AudioProcessor = void 0;
const child_process_1 = require("child_process");
const path_1 = require("path");
const mmapAudioFile_1 = require("./mmapAudioFile");
/**
 * Audio processing service using FFmpeg with memory-mapped file support
 * Handles LUFS analysis, track splitting, and format conversion
 */
class AudioProcessor {
    constructor() {
        // Determine FFmpeg paths based on platform
        const isWindows = process.platform === "win32";
        const binPath = (0, path_1.join)(__dirname, "../bin", isWindows ? "win" : "linux");
        this.ffmpegPath = (0, path_1.join)(binPath, isWindows ? "ffmpeg.exe" : "ffmpeg");
        this.ffprobePath = (0, path_1.join)(binPath, isWindows ? "ffprobe.exe" : "ffprobe");
    }
    /**
     * Analyze audio file for LUFS, true peak, and dynamic range
     */
    async analyzeFile(filePath) {
        return new Promise((resolve, reject) => {
            const args = [
                "-i",
                filePath,
                "-af",
                "ebur128=peak=true:metadata=1:framelog=quiet,astats=metadata=1:reset=1",
                "-f",
                "null",
                "-",
            ];
            let errorOutput = "";
            const process = (0, child_process_1.spawn)(this.ffmpegPath, args, {
                stdio: ["ignore", "pipe", "pipe"],
            });
            process.stderr.on("data", (data) => {
                errorOutput += data.toString();
            });
            process.on("close", (code) => {
                if (code !== 0) {
                    reject(new Error(`FFmpeg failed with code ${code}: ${errorOutput}`));
                    return;
                }
                try {
                    const analysis = this.parseFFmpegOutput(errorOutput);
                    resolve(analysis);
                }
                catch (error) {
                    reject(new Error(`Failed to parse FFmpeg output: ${error}`));
                }
            });
            process.on("error", (error) => {
                reject(new Error(`Failed to spawn FFmpeg: ${error}`));
            });
        });
    }
    /**
     * Analyze a segment of memory-mapped audio file
     */
    async analyzeMemorySegment(fileId, startTime, duration) {
        const mmapFile = mmapAudioFile_1.MemoryMappedAudioFile.getFile(fileId);
        if (!mmapFile) {
            throw new Error(`Memory-mapped file with ID ${fileId} not found`);
        }
        const filePath = mmapFile.getPath();
        return new Promise((resolve, reject) => {
            const args = [
                "-ss",
                startTime.toString(),
                "-t",
                duration.toString(),
                "-i",
                filePath,
                "-af",
                "ebur128=peak=true:metadata=1:framelog=quiet,astats=metadata=1:reset=1",
                "-f",
                "null",
                "-",
            ];
            let errorOutput = "";
            const process = (0, child_process_1.spawn)(this.ffmpegPath, args, {
                stdio: ["ignore", "pipe", "pipe"],
            });
            process.stderr.on("data", (data) => {
                errorOutput += data.toString();
            });
            process.on("close", (code) => {
                if (code !== 0) {
                    reject(new Error(`FFmpeg failed with code ${code}: ${errorOutput}`));
                    return;
                }
                try {
                    const analysis = this.parseFFmpegOutput(errorOutput);
                    resolve(analysis);
                }
                catch (error) {
                    reject(new Error(`Failed to parse FFmpeg output: ${error}`));
                }
            });
            process.on("error", (error) => {
                reject(new Error(`Failed to spawn FFmpeg: ${error}`));
            });
        });
    }
    /**
     * Process/convert audio track with specific parameters
     */
    async processTrack(options) {
        return new Promise((resolve, reject) => {
            const args = this.buildFFmpegArgs(options);
            let errorOutput = "";
            let progress = 0;
            const process = (0, child_process_1.spawn)(this.ffmpegPath, args, {
                stdio: ["ignore", "pipe", "pipe"],
            });
            process.stderr.on("data", (data) => {
                const chunk = data.toString();
                errorOutput += chunk;
                // Parse progress from FFmpeg output
                const timeMatch = chunk.match(/time=(\d{2}):(\d{2}):(\d{2}\.\d{2})/);
                if (timeMatch && options.duration) {
                    const [, hours, minutes, seconds] = timeMatch;
                    const currentTime = parseInt(hours) * 3600 +
                        parseInt(minutes) * 60 +
                        parseFloat(seconds);
                    progress = Math.min((currentTime / options.duration) * 100, 100);
                    // Emit progress event
                    if (options.onProgress) {
                        options.onProgress(progress);
                    }
                }
            });
            process.on("close", (code) => {
                if (code !== 0) {
                    reject(new Error(`FFmpeg failed with code ${code}: ${errorOutput}`));
                    return;
                }
                resolve({
                    success: true,
                    outputPath: options.outputPath,
                    progress: 100,
                });
            });
            process.on("error", (error) => {
                reject(new Error(`Failed to spawn FFmpeg: ${error}`));
            });
        });
    }
    /**
     * Get audio file metadata using ffprobe
     */
    async getFileMetadata(filePath) {
        return new Promise((resolve, reject) => {
            const args = [
                "-v",
                "quiet",
                "-print_format",
                "json",
                "-show_format",
                "-show_streams",
                filePath,
            ];
            let output = "";
            const process = (0, child_process_1.spawn)(this.ffprobePath, args, {
                stdio: ["ignore", "pipe", "pipe"],
            });
            process.stdout.on("data", (data) => {
                output += data.toString();
            });
            process.on("close", (code) => {
                if (code !== 0) {
                    reject(new Error(`FFprobe failed with code ${code}`));
                    return;
                }
                try {
                    const metadata = JSON.parse(output);
                    resolve(this.parseMetadata(metadata));
                }
                catch (error) {
                    reject(new Error(`Failed to parse metadata: ${error}`));
                }
            });
            process.on("error", (error) => {
                reject(new Error(`Failed to spawn FFprobe: ${error}`));
            });
        });
    }
    parseFFmpegOutput(output) {
        const lines = output.split("\n");
        let integratedLoudness = 0;
        let truePeak = 0;
        let loudnessRange = 0;
        for (const line of lines) {
            if (line.includes("Integrated loudness:")) {
                const match = line.match(/([-\d.]+) LUFS/);
                if (match)
                    integratedLoudness = parseFloat(match[1]);
            }
            else if (line.includes("True peak:")) {
                const match = line.match(/([-\d.]+) dBTP/);
                if (match)
                    truePeak = parseFloat(match[1]);
            }
            else if (line.includes("LRA:")) {
                const match = line.match(/([\d.]+) LU/);
                if (match)
                    loudnessRange = parseFloat(match[1]);
            }
        }
        return {
            lufs: integratedLoudness,
            truePeak,
            dynamicRange: loudnessRange,
            timestamp: new Date().toISOString(),
        };
    }
    parseMetadata(ffprobeOutput) {
        const format = ffprobeOutput.format;
        const audioStream = ffprobeOutput.streams.find((s) => s.codec_type === "audio");
        return {
            duration: parseFloat(format.duration),
            bitrate: parseInt(format.bit_rate),
            sampleRate: parseInt(audioStream?.sample_rate || 0),
            channels: parseInt(audioStream?.channels || 0),
            codec: audioStream?.codec_name || "unknown",
            bitDepth: parseInt(audioStream?.bits_per_sample || 0),
            size: parseInt(format.size || 0),
        };
    }
    buildFFmpegArgs(options) {
        const args = [];
        // Input
        if (options.startTime) {
            args.push("-ss", options.startTime.toString());
        }
        if (options.duration) {
            args.push("-t", options.duration.toString());
        }
        args.push("-i", options.inputPath);
        // Audio filters
        const filters = [];
        if (options.targetLufs) {
            filters.push(`loudnorm=I=${options.targetLufs}:TP=${options.truePeakTarget || -1}:LRA=${options.loudnessRange || 11}`);
        }
        if (filters.length > 0) {
            args.push("-af", filters.join(","));
        }
        // Codec and quality
        if (options.codec) {
            args.push("-c:a", options.codec);
        }
        if (options.bitrate) {
            args.push("-b:a", `${options.bitrate}k`);
        }
        if (options.sampleRate) {
            args.push("-ar", options.sampleRate.toString());
        }
        // Output
        args.push("-y"); // Overwrite output files
        args.push(options.outputPath);
        return args;
    }
}
exports.AudioProcessor = AudioProcessor;
//# sourceMappingURL=audioProcessor.js.map