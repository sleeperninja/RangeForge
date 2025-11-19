import { parentPort, workerData } from "worker_threads";
import { AudioProcessor } from "../electron/services/audioProcessor";

interface WorkerData {
  filePath: string;
  options: any;
}

async function processAudio() {
  if (!parentPort) {
    throw new Error("Worker must be run in worker thread");
  }

  const { filePath, options }: WorkerData = workerData;
  const audioProcessor = new AudioProcessor();

  try {
    // Set up progress forwarding
    audioProcessor.on("progress", (progressData) => {
      parentPort?.postMessage({
        type: "progress",
        data: {
          ...progressData,
          workerId: process.pid,
          file: filePath,
        },
      });
    });

    // Analyze the audio file
    const analysisResult = await audioProcessor.analyzeFile(filePath);

    parentPort.postMessage({
      type: "analysis-complete",
      data: analysisResult,
    });

    // If volume adjustment is requested, perform it
    if (options.adjustVolume) {
      const outputPath = await audioProcessor.adjustVolume(
        filePath,
        options.volumeOptions || {}
      );

      parentPort.postMessage({
        type: "volume-adjusted",
        data: {
          inputPath: filePath,
          outputPath,
          options: options.volumeOptions,
        },
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
  } finally {
    audioProcessor.dispose();
  }
}

// Start processing
processAudio().catch((error) => {
  parentPort?.postMessage({
    type: "error",
    error: error instanceof Error ? error.message : String(error),
  });
});
