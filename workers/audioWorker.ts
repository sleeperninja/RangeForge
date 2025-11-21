import { parentPort, workerData } from "worker_threads";
import {
  AudioProcessor,
  ProcessingOptions,
} from "../electron/services/audioProcessor";

interface WorkerData {
  filePath: string;
  options: ProcessingOptions;
}

async function processAudio() {
  if (!parentPort) {
    throw new Error("Worker must be run in worker thread");
  }

  const { filePath, options }: WorkerData = workerData;
  const audioProcessor = new AudioProcessor();

  try {
    // Analyze the audio file
    const analysisResult = await audioProcessor.analyzeFile(filePath);

    parentPort.postMessage({
      type: "analysis-complete",
      data: analysisResult,
    });

    // If processing options are provided, process the track
    if (options.outputPath) {
      const processingOptions = {
        ...options,
        inputPath: filePath,
        onProgress: (progress: number) => {
          parentPort?.postMessage({
            type: "progress",
            data: {
              progress,
              workerId: process.pid,
              file: filePath,
            },
          });
        },
      };

      const result = await audioProcessor.processTrack(processingOptions);

      parentPort.postMessage({
        type: "processing-complete",
        data: result,
      });
    }

    parentPort.postMessage({
      type: "complete",
      data: {
        filePath,
        message: "Audio processing completed successfully",
      },
    });
  } catch (error) {
    parentPort.postMessage({
      type: "error",
      error: error instanceof Error ? error.message : String(error),
      filePath,
    });
  }
}

// Start processing
processAudio().catch((error) => {
  parentPort?.postMessage({
    type: "error",
    error: error instanceof Error ? error.message : String(error),
  });
});
