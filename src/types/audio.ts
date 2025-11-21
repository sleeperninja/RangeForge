export interface AudioFileInfo {
  id: string;
  name: string;
  path: string;
  codec: string;
  sampleRate?: number;
  bitDepth?: number;
  channels?: number;
  duration?: number;
  fileSize: number;
}

export interface AudioAnalysisResult {
  fileId: string;
  loudness: {
    lufs: number;
    range: number;
  };
  truePeak: {
    dbtp: number;
  };
  dynamicRange: {
    dr: number;
  };
  adjustment: {
    targetLufs: number;
    adjustmentDb: number;
  };
}

export interface GenreTag {
  name: string;
  color: "blue" | "purple" | "green" | "orange" | "red" | "yellow";
  source: "lastfm" | "user" | "metadata";
}

export interface AudioTrack extends AudioFileInfo {
  analysis?: AudioAnalysisResult;
  tags: GenreTag[];
  isProcessing: boolean;
  error?: string;
  // CUE-related fields for tracks extracted from CUE sheets
  isCueTrack?: boolean;
  cueTrackNumber?: number;
  cueStartTime?: number; // in seconds
  cueDuration?: number; // in seconds
  parentCueFile?: string;
  parentAudioFile?: string;
  cueData?: {
    startTime: number;
    endTime?: number;
    trackNumber: string;
    performer: string;
    title: string;
  };
}

export interface CueFile {
  path: string;
  audioFile: string;
  tracks: CueTrack[];
}

export interface CueTrack {
  index: number;
  title: string;
  artist?: string;
  startTime: number; // in seconds
  endTime?: number; // in seconds
}

export interface ProcessingProfile {
  id: string;
  name: string;
  targetLufs: number;
  peakProtection: number; // dBTP
  outputFormat: "flac" | "wav" | "alac" | "aac" | "mp3";
  sampleRate: number | "original";
  bitDepth: number | "original";
  channels: number | "original";
  fileStrategy: "serialize" | "rename" | "replace" | "location";
  customLocation?: string;
}

export interface ProcessingQueue {
  tracks: AudioTrack[];
  profile: ProcessingProfile;
  autoMode: boolean;
  metadataApi: boolean;
}
