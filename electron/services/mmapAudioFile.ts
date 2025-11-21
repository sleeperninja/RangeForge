import * as mmap from "mmap-io";
import * as fs from "fs";
import { randomUUID } from "crypto";

/**
 * Memory-mapped audio file handler
 * Provides efficient access to large audio files without loading into RAM
 */
export class MemoryMappedAudioFile {
  private static openFiles = new Map<string, MemoryMappedAudioFile>();

  private id: string;
  private filePath: string;
  private fileDescriptor?: number;
  private mappedBuffer?: Buffer;
  private fileSize: number = 0;
  private isOpen: boolean = false;

  constructor(filePath: string) {
    this.id = randomUUID();
    this.filePath = filePath;
    MemoryMappedAudioFile.openFiles.set(this.id, this);
  }

  async open(): Promise<void> {
    if (this.isOpen) {
      throw new Error("File is already open");
    }

    try {
      // Get file stats
      const stats = await fs.promises.stat(this.filePath);
      this.fileSize = stats.size;

      // Open file descriptor
      this.fileDescriptor = await fs.promises.open(this.filePath, "r");

      // Create memory mapping
      this.mappedBuffer = mmap.map(
        this.fileSize,
        mmap.PROT_READ,
        mmap.MAP_SHARED,
        this.fileDescriptor.fd,
        0
      );

      this.isOpen = true;

      console.log(
        `Memory mapped file: ${this.filePath} (${this.fileSize} bytes)`
      );
    } catch (error) {
      this.cleanup();
      throw new Error(`Failed to memory map file: ${error}`);
    }
  }

  readSegment(start: number, length: number): Buffer {
    if (!this.isOpen || !this.mappedBuffer) {
      throw new Error("File is not open");
    }

    if (start < 0 || start >= this.fileSize) {
      throw new Error("Start position is out of bounds");
    }

    if (start + length > this.fileSize) {
      length = this.fileSize - start;
    }

    return this.mappedBuffer.slice(start, start + length);
  }

  async close(): Promise<void> {
    this.cleanup();
  }

  private cleanup(): void {
    try {
      if (this.mappedBuffer) {
        mmap.unmap(this.mappedBuffer);
        this.mappedBuffer = undefined;
      }

      if (this.fileDescriptor) {
        fs.closeSync(this.fileDescriptor.fd);
        this.fileDescriptor = undefined;
      }

      this.isOpen = false;
      MemoryMappedAudioFile.openFiles.delete(this.id);

      console.log(`Unmapped file: ${this.filePath}`);
    } catch (error) {
      console.error(`Error cleaning up memory map: ${error}`);
    }
  }

  // Getters
  getId(): string {
    return this.id;
  }

  getSize(): number {
    return this.fileSize;
  }

  getPath(): string {
    return this.filePath;
  }

  isFileOpen(): boolean {
    return this.isOpen;
  }

  // Static methods for managing multiple files
  static close(fileId: string): void {
    const file = MemoryMappedAudioFile.openFiles.get(fileId);
    if (file) {
      file.close();
    }
  }

  static readSegment(fileId: string, start: number, length: number): Buffer {
    const file = MemoryMappedAudioFile.openFiles.get(fileId);
    if (!file) {
      throw new Error(`File with ID ${fileId} not found`);
    }
    return file.readSegment(start, length);
  }

  static getFile(fileId: string): MemoryMappedAudioFile | undefined {
    return MemoryMappedAudioFile.openFiles.get(fileId);
  }

  static closeAll(): void {
    for (const file of MemoryMappedAudioFile.openFiles.values()) {
      file.close();
    }
    MemoryMappedAudioFile.openFiles.clear();
  }

  static getOpenFileCount(): number {
    return MemoryMappedAudioFile.openFiles.size;
  }

  static getOpenFiles(): string[] {
    return Array.from(MemoryMappedAudioFile.openFiles.keys());
  }
}

// Handle process cleanup
process.on("exit", () => {
  MemoryMappedAudioFile.closeAll();
});

process.on("SIGINT", () => {
  MemoryMappedAudioFile.closeAll();
  process.exit(0);
});

process.on("SIGTERM", () => {
  MemoryMappedAudioFile.closeAll();
  process.exit(0);
});
