import React from "react";
import "./App.css";
import { AudioProcessor } from "./components/AudioProcessor";
import { FileManager } from "./components/FileManager";
import { ProgressMonitor } from "./components/ProgressMonitor";

function App() {
  return (
    <div className="App">
      <header className="App-header">
        <h1>RangeForge</h1>
        <p>Professional Audio Dynamic Range and Volume Analysis Tool</p>
      </header>

      <main className="App-main">
        <div className="app-container">
          <div className="left-panel">
            <FileManager />
          </div>

          <div className="right-panel">
            <AudioProcessor />
            <ProgressMonitor />
          </div>
        </div>
      </main>
    </div>
  );
}

export default App;
