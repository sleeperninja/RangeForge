// scripts/prepare-electron.js
const fs = require("fs");
const path = require("path");
const fse = require("fs-extra");

// Paths
const DIST = path.resolve(__dirname, "../dist");
const ELECTRON_DIST = path.join(DIST, "electron");
const ELECTRON_SOURCES = path.join(DIST, "electronSources");
const ELECTRON_SRC = path.resolve(__dirname, "../electron");
const SRC_TYPES = path.resolve(__dirname, "../src/types");

// 1. Clean old dist
if (fs.existsSync(DIST)) {
  fse.removeSync(DIST);
}

// 2. Create folders
fse.ensureDirSync(ELECTRON_DIST);
fse.ensureDirSync(ELECTRON_SOURCES);

// 3. Copy electron source files to electronSources
fse.copySync(ELECTRON_SRC, ELECTRON_SOURCES);

// 4. Copy src/types to electronSources/types
fse.ensureDirSync(path.join(ELECTRON_SOURCES, "types"));
fse.copySync(SRC_TYPES, path.join(ELECTRON_SOURCES, "types"));

// 5. Optionally rename main.ts → electron.js if needed
const mainTs = path.join(ELECTRON_SOURCES, "main.ts");
const mainJs = path.join(ELECTRON_SOURCES, "electron.js");
if (fs.existsSync(mainTs)) {
  fs.renameSync(mainTs, mainJs);
  console.log("Renamed main.ts → electron.js");
}

console.log("✅ Electron sources staged at:", ELECTRON_SOURCES);
