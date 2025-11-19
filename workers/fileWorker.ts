import { parentPort, workerData } from "worker_threads";
import * as path from "path";
import * as fs from "fs";

interface WorkerData {
  filePaths: string[];
  options: {
    operation: "analyze" | "convert" | "normalize";
    outputDirectory?: string;
  };
  workerId: number;
}

async function processFiles() {
  if (!parentPort) {
    throw new Error("Worker must be run in worker thread");
  }

  const { filePaths, options, workerId }: WorkerData = workerData;

  try {
    parentPort.postMessage({
      type: "progress",
      data: {
        workerId,
        type: "worker-start",
        message: `Worker ${workerId} started processing ${filePaths.length} files`,
      },
    });

    for (let i = 0; i < filePaths.length; i++) {
      const filePath = filePaths[i];

      parentPort.postMessage({
        type: "progress",
        data: {
          workerId,
          type: "file-start",
          filePath,
          currentFile: i + 1,
          totalFiles: filePaths.length,
          message: `Processing: ${path.basename(filePath)}`,
        },
      });

      try {
        await processFile(filePath, options);

        parentPort.postMessage({
          type: "progress",
          data: {
            workerId,
            type: "file-complete",
            filePath,
            currentFile: i + 1,
            totalFiles: filePaths.length,
            message: `Completed: ${path.basename(filePath)}`,
          },
        });
      } catch (fileError) {
        parentPort.postMessage({
          type: "progress",
          data: {
            workerId,
            type: "file-error",
            filePath,
            currentFile: i + 1,
            totalFiles: filePaths.length,
            error:
              fileError instanceof Error
                ? fileError.message
                : String(fileError),
            message: `Error processing: ${path.basename(filePath)}`,
          },
        });
      }
    }

    parentPort.postMessage({
      type: "complete",
      data: {
        workerId,
        processedFiles: filePaths.length,
        message: `Worker ${workerId} completed processing`,
      },
    });
  } catch (error) {
    parentPort.postMessage({
      type: "error",
      error: error instanceof Error ? error.message : String(error),
      workerId,
    });
  }
}

async function processFile(
  filePath: string,
  options: WorkerData["options"]
): Promise<void> {
  // Simulate file processing based on operation type
  switch (options.operation) {
    case "analyze":
      await analyzeFile(filePath);
      break;
    case "convert":
      await convertFile(filePath, options.outputDirectory);
      break;
    case "normalize":
      await normalizeFile(filePath, options.outputDirectory);
      break;
    default:
      throw new Error(`Unknown operation: ${options.operation}`);
  }
}

async function analyzeFile(filePath: string): Promise<void> {
  // Simulate file analysis
  const stats = await fs.promises.stat(filePath);

  // Simulate processing time
  await new Promise((resolve) =>
    setTimeout(resolve, 100 + Math.random() * 500)
  );

  // Emit analysis results
  parentPort?.postMessage({
    type: "progress",
    data: {
      type: "analysis-result",
      filePath,
      size: stats.size,
      modified: stats.mtime,
      message: "File analysis completed",
    },
  });
}

async function convertFile(
  filePath: string,
  outputDirectory?: string
): Promise<void> {
  // Simulate file conversion
  const outputDir = outputDirectory || path.dirname(filePath);
  const parsedPath = path.parse(filePath);
  const outputPath = path.join(
    outputDir,
    `${parsedPath.name}_converted${parsedPath.ext}`
  );

  // Simulate processing time
  await new Promise((resolve) =>
    setTimeout(resolve, 1000 + Math.random() * 2000)
  );

  // Simulate file creation (just copy for now)
  await fs.promises.copyFile(filePath, outputPath);

  parentPort?.postMessage({
    type: "progress",
    data: {
      type: "conversion-result",
      inputPath: filePath,
      outputPath,
      message: "File conversion completed",
    },
  });
}

async function normalizeFile(
  filePath: string,
  outputDirectory?: string
): Promise<void> {
  // Simulate audio normalization
  const outputDir = outputDirectory || path.dirname(filePath);
  const parsedPath = path.parse(filePath);
  const outputPath = path.join(
    outputDir,
    `${parsedPath.name}_normalized${parsedPath.ext}`
  );

  // Simulate processing time
  await new Promise((resolve) =>
    setTimeout(resolve, 1500 + Math.random() * 2500)
  );

  // Simulate file creation (just copy for now)
  await fs.promises.copyFile(filePath, outputPath);

  parentPort?.postMessage({
    type: "progress",
    data: {
      type: "normalization-result",
      inputPath: filePath,
      outputPath,
      message: "Audio normalization completed",
    },
  });
}

// Start processing
processFiles().catch((error) => {
  parentPort?.postMessage({
    type: "error",
    error: error instanceof Error ? error.message : String(error),
  });
});
