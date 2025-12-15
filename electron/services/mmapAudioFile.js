"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || function (mod) {
    if (mod && mod.__esModule) return mod;
    var result = {};
    if (mod != null) for (var k in mod) if (k !== "default" && Object.prototype.hasOwnProperty.call(mod, k)) __createBinding(result, mod, k);
    __setModuleDefault(result, mod);
    return result;
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.MemoryMappedAudioFile = void 0;
const mmap = __importStar(require("mmap-io"));
const fs = __importStar(require("fs"));
const crypto_1 = require("crypto");
// Type assertion for mmap-io (TypeScript definitions may be incomplete)
const mmapLib = mmap;
/**
 * Memory-mapped audio file handler
 * Provides efficient access to large audio files without loading into RAM
 */
class MemoryMappedAudioFile {
    constructor(filePath) {
        this.fileSize = 0;
        this.isOpen = false;
        this.id = (0, crypto_1.randomUUID)();
        this.filePath = filePath;
        MemoryMappedAudioFile.openFiles.set(this.id, this);
    }
    async open() {
        if (this.isOpen) {
            throw new Error("File is already open");
        }
        try {
            // Get file stats
            const stats = await fs.promises.stat(this.filePath);
            this.fileSize = stats.size;
            // Open file descriptor
            this.fileDescriptor = fs.openSync(this.filePath, "r");
            // Create memory mapping
            this.mappedBuffer = mmap.map(this.fileSize, mmap.PROT_READ, mmap.MAP_SHARED, this.fileDescriptor, 0);
            this.isOpen = true;
            console.log(`Memory mapped file: ${this.filePath} (${this.fileSize} bytes)`);
        }
        catch (error) {
            this.cleanup();
            throw new Error(`Failed to memory map file: ${error}`);
        }
    }
    readSegment(start, length) {
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
    async close() {
        this.cleanup();
    }
    cleanup() {
        try {
            if (this.mappedBuffer) {
                // mmap-io automatically unmaps when buffer is garbage collected
                // Just clear our reference
                this.mappedBuffer = undefined;
            }
            if (this.fileDescriptor !== undefined) {
                fs.closeSync(this.fileDescriptor);
                this.fileDescriptor = undefined;
            }
            this.isOpen = false;
            MemoryMappedAudioFile.openFiles.delete(this.id);
            console.log(`Unmapped file: ${this.filePath}`);
        }
        catch (error) {
            console.error(`Error cleaning up memory map: ${error}`);
        }
    } // Getters
    getId() {
        return this.id;
    }
    getSize() {
        return this.fileSize;
    }
    getPath() {
        return this.filePath;
    }
    isFileOpen() {
        return this.isOpen;
    }
    // Static methods for managing multiple files
    static close(fileId) {
        const file = MemoryMappedAudioFile.openFiles.get(fileId);
        if (file) {
            file.close();
        }
    }
    static readSegment(fileId, start, length) {
        const file = MemoryMappedAudioFile.openFiles.get(fileId);
        if (!file) {
            throw new Error(`File with ID ${fileId} not found`);
        }
        return file.readSegment(start, length);
    }
    static getFile(fileId) {
        return MemoryMappedAudioFile.openFiles.get(fileId);
    }
    static closeAll() {
        for (const file of MemoryMappedAudioFile.openFiles.values()) {
            file.close();
        }
        MemoryMappedAudioFile.openFiles.clear();
    }
    static getOpenFileCount() {
        return MemoryMappedAudioFile.openFiles.size;
    }
    static getOpenFiles() {
        return Array.from(MemoryMappedAudioFile.openFiles.keys());
    }
}
exports.MemoryMappedAudioFile = MemoryMappedAudioFile;
MemoryMappedAudioFile.openFiles = new Map();
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
//# sourceMappingURL=mmapAudioFile.js.map