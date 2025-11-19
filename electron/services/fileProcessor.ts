import { EventEmitter } from "events";
import * as fs from "fs";
import * as path from "path";
import * as mmap from "mmap-io";
import { Worker } from "worker_threads";

export interface FileProcessingOptions {
  operation: "analyze" | "convert" | "normalize";
  outputDirectory?: string;
  parallel?: boolean;
  maxWorkers?: number;
}

export interface MappedFileInfo {
  filePath: string;
  size: number;
  buffer: Buffer;
  mappedAt: Date;
}

export class FileProcessor extends EventEmitter {
  private workers: Worker[] = [];
  private mappedFiles: Map<string, MappedFileInfo> = new Map();
  private maxWorkers: number = 4;

  constructor() {
    super();
  }

  /**
   * Process multiple files in batch with optional parallel processing
   */
  async processBatch(
    filePaths: string[],
    options: FileProcessingOptions
  ): Promise<void> {
    const maxWorkers = options.maxWorkers || this.maxWorkers;
    const parallel = options.parallel !== false; // Default to true

    this.emit("progress", {
      type: "batch-start",
      totalFiles: filePaths.length,
      message: `Starting batch processing of ${filePaths.length} files`,
    });

    if (parallel) {
      await this.processFilesParallel(filePaths, options, maxWorkers);
    } else {
      await this.processFilesSequential(filePaths, options);
    }

    this.emit("progress", {
      type: "batch-complete",
      totalFiles: filePaths.length,
      message: "Batch processing completed",
    });
  }

  /**
   * Process files in parallel using worker threads
   */
  private async processFilesParallel(
    filePaths: string[],
    options: FileProcessingOptions,
    maxWorkers: number
  ): Promise<void> {
    const chunks = this.chunkArray(filePaths, maxWorkers);
    const workerPromises: Promise<void>[] = [];

    for (let i = 0; i < chunks.length; i++) {
      const chunk = chunks[i];
      const workerPromise = this.createWorkerForChunk(chunk, options, i);
      workerPromises.push(workerPromise);
    }

    await Promise.all(workerPromises);
  }

  /**
   * Process files sequentially
   */
  private async processFilesSequential(
    filePaths: string[],
    options: FileProcessingOptions
  ): Promise<void> {
    for (let i = 0; i < filePaths.length; i++) {
      const filePath = filePaths[i];

      this.emit("progress", {
        type: "file-start",
        currentFile: i + 1,
        totalFiles: filePaths.length,
        filePath,
        message: `Processing file ${i + 1}/${filePaths.length}: ${path.basename(
          filePath
        )}`,
      });

      try {
        await this.processFileInWorker(filePath, options);

        this.emit("progress", {
          type: "file-complete",
          currentFile: i + 1,
          totalFiles: filePaths.length,
          filePath,
          message: `Completed file ${i + 1}/${filePaths.length}`,
        });
      } catch (error) {
        this.emit("progress", {
          type: "file-error",
          currentFile: i + 1,
          totalFiles: filePaths.length,
          filePath,
          error: error instanceof Error ? error.message : String(error),
          message: `Error processing file ${i + 1}/${filePaths.length}`,
        });
      }
    }
  }

  /**
   * Create a worker for processing a chunk of files
   */
  private createWorkerForChunk(
    filePaths: string[],
    options: FileProcessingOptions,
    workerId: number
  ): Promise<void> {
    return new Promise((resolve, reject) => {
      const workerPath = path.join(
        __dirname,
        "../../dist/workers/fileWorker.js"
      );

      const worker = new Worker(workerPath, {
        workerData: {
          filePaths,
          options,
          workerId,
        },
      });

      this.workers.push(worker);

      worker.on("message", (message) => {
        switch (message.type) {
          case "progress":
            this.emit("progress", message.data);
            break;
          case "complete":
            resolve();
            break;
          case "error":
            reject(new Error(message.error));
            break;
        }
      });

      worker.on("error", (error) => {
        reject(error);
      });

      worker.on("exit", (code) => {
        // Remove worker from array
        const index = this.workers.indexOf(worker);
        if (index > -1) {
          this.workers.splice(index, 1);
        }

        if (code !== 0) {
          reject(
            new Error(`Worker ${workerId} stopped with exit code ${code}`)
          );
        }
      });
    });
  }

  /**
   * Process a single file in a worker thread
   */
  private processFileInWorker(
    filePath: string,
    options: FileProcessingOptions
  ): Promise<void> {
    return new Promise((resolve, reject) => {
      const workerPath = path.join(
        __dirname,
        "../../dist/workers/fileWorker.js"
      );

      const worker = new Worker(workerPath, {
        workerData: {
          filePaths: [filePath],
          options,
          workerId: Date.now(),
        },
      });

      worker.on("message", (message) => {
        if (message.type === "complete") {
          resolve();
        } else if (message.type === "error") {
          reject(new Error(message.error));
        }
      });

      worker.on("error", reject);
      worker.on("exit", (code) => {
        if (code !== 0) {
          reject(new Error(`Worker stopped with exit code ${code}`));
        }
      });
    });
  }

  /**
   * Memory-map a file for efficient access
   */
  async mapFile(filePath: string): Promise<MappedFileInfo> {
    try {
      // Check if file is already mapped
      if (this.mappedFiles.has(filePath)) {
        return this.mappedFiles.get(filePath)!;
      }

      // Get file stats
      const stats = await fs.promises.stat(filePath);

      // Open file descriptor
      const fd = await fs.promises.open(filePath, "r");

      // Create memory mapping
      const buffer = mmap.map(
        stats.size,
        mmap.PROT_READ,
        mmap.MAP_SHARED,
        fd.fd,
        0
      );

      // Close file descriptor (mapping persists)
      await fd.close();

      const mappedFileInfo: MappedFileInfo = {
        filePath,
        size: stats.size,
        buffer,
        mappedAt: new Date(),
      };

      this.mappedFiles.set(filePath, mappedFileInfo);

      this.emit("progress", {
        type: "file-mapped",
        filePath,
        size: stats.size,
        message: `Memory-mapped file: ${path.basename(
          filePath
        )} (${this.formatFileSize(stats.size)})`,
      });

      return mappedFileInfo;
    } catch (error) {
      const errorMessage =
        error instanceof Error ? error.message : String(error);
      this.emit("progress", {
        type: "mapping-error",
        filePath,
        error: errorMessage,
        message: `Failed to memory-map file: ${path.basename(filePath)}`,
      });
      throw new Error(`Failed to map file ${filePath}: ${errorMessage}`);
    }
  }

  /**
   * Unmap a memory-mapped file
   */
  async unmapFile(filePath: string): Promise<void> {
    const mappedFile = this.mappedFiles.get(filePath);

    if (mappedFile) {
      try {
        mmap.unmap(mappedFile.buffer);
        this.mappedFiles.delete(filePath);

        this.emit("progress", {
          type: "file-unmapped",
          filePath,
          message: `Unmapped file: ${path.basename(filePath)}`,
        });
      } catch (error) {
        const errorMessage =
          error instanceof Error ? error.message : String(error);
        throw new Error(`Failed to unmap file ${filePath}: ${errorMessage}`);
      }
    }
  }

  /**
   * Get information about a mapped file
   */
  getMappedFileInfo(filePath: string): MappedFileInfo | undefined {
    return this.mappedFiles.get(filePath);
  }

  /**
   * Get all currently mapped files
   */
  getAllMappedFiles(): MappedFileInfo[] {
    return Array.from(this.mappedFiles.values());
  }

  /**
   * Read a chunk of data from a memory-mapped file
   */
  readMappedChunk(
    filePath: string,
    offset: number,
    length: number
  ): Buffer | null {
    const mappedFile = this.mappedFiles.get(filePath);

    if (!mappedFile) {
      return null;
    }

    if (offset + length > mappedFile.size) {
      throw new Error("Read beyond file boundary");
    }

    return mappedFile.buffer.subarray(offset, offset + length);
  }

  /**
   * Utility function to chunk an array
   */
  private chunkArray<T>(array: T[], chunkSize: number): T[][] {
    const chunks: T[][] = [];
    for (let i = 0; i < array.length; i += chunkSize) {
      chunks.push(array.slice(i, i + chunkSize));
    }
    return chunks;
  }

  /**
   * Format file size for display
   */
  private formatFileSize(bytes: number): string {
    if (bytes === 0) return "0 Bytes";

    const k = 1024;
    const sizes = ["Bytes", "KB", "MB", "GB", "TB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));

    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
  }

  /**
   * Clean up all workers and mapped files
   */
  async dispose(): Promise<void> {
    // Terminate all workers
    const workerTerminations = this.workers.map((worker) => {
      return worker.terminate();
    });

    await Promise.all(workerTerminations);
    this.workers = [];

    // Unmap all files
    const unmapPromises = Array.from(this.mappedFiles.keys()).map(
      (filePath) => {
        return this.unmapFile(filePath).catch((error) => {
          console.error(`Failed to unmap ${filePath}:`, error);
        });
      }
    );

    await Promise.all(unmapPromises);
  }
}
