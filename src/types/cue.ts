export interface CueTrack {
  trackNumber: number;
  title: string;
  performer?: string;
  indexTime: string; // MM:SS:FF format
  indexTimeSeconds: number; // Total seconds for easier calculation
}

export interface CueSheet {
  performer?: string;
  title?: string;
  file: string;
  fileType: string;
  tracks: CueTrack[];
}

export interface CueParseResult {
  isValid: boolean;
  cueSheet?: CueSheet;
  audioFilePath?: string;
  error?: string;
}
