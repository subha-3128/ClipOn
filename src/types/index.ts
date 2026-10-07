// ===== Core Application Types =====

export type EnvironmentStatus = {
  dataDir: string;
  hasFfmpeg: boolean;
  hasFfprobe: boolean;
  hasDeepgramKey: boolean;
  hasAnthropicKey: boolean;
  hasDeepseekKey: boolean;
  hasGeminiKey: boolean;
  hasOpenaiKey: boolean;
  hasOpenrouterKey: boolean;
  hasGroqKey: boolean;
  hasNvidiaKey?: boolean;
  hasNvidiaFunctionId?: boolean;
  hasInstagramToken: boolean;
  llmProvider: string;
  hasLocalWhisperModel: boolean;
  hasOllama: boolean;
  hasYtdlp: boolean;
  hasHardwareAccel?: boolean;
  instagramAccountId?: string;
  platform?: string;
  localWhisperSupported?: boolean;
  ollamaSupported?: boolean;
  ollamaInstallSupported?: boolean;
  faceTrackingSupported?: boolean;
  hardwareEncoderSupported?: boolean;
};

export type Project = {
  id: string;
  name: string | null;
  sourcePath: string;
  sourceDuration: number | null;
  status: string;
  transcriptionMode: string;
  captionStyle?: string | null;
  createdAt: string;
  updatedAt: string;
};

export type Transcript = {
  id: string;
  projectId: string;
  engine: string;
  rawJson: string;
  language: string | null;
  createdAt: string;
};

export type SocialKit = {
  candidateId: string;
  titles: string[];
  description: string;
  hashtags: string[];
  callToAction: string;
};

export type Candidate = {
  id: string;
  projectId: string;
  startSec: number;
  endSec: number;
  score: number;
  hook: string;
  rationale: string;
  rank: number;
  selected: boolean;
  layoutOverride?: string | null;
};

export type Clip = {
  id: string;
  candidateId: string;
  status: string;
  outputPath: string | null;
  faceTrackJson: string | null;
  captionAssPath: string | null;
  renderLog: string | null;
};

export type InstagramPost = {
  id: string;
  candidateId: string;
  clipId: string | null;
  status: "queued" | "publishing" | "published" | "failed";
  caption: string | null;
  postUrl: string | null;
  errorMessage: string | null;
  createdAt: string;
  publishedAt: string | null;
};

export type ProjectDetail = {
  project: Project;
  transcript: Transcript | null;
  candidates: Candidate[];
  clips: Clip[];
  instagramPosts?: InstagramPost[];
};

export type NormalizedTranscriptSegment = {
  start: number;
  end: number;
  speaker: string | null;
  text: string;
};

export type NormalizedTranscript = {
  language: string;
  duration: number;
  speakers: string[];
  segments: NormalizedTranscriptSegment[];
};

export type BusyState =
  | "idle"
  | "import"
  | "transcribe"
  | "demoTranscript"
  | "moments"
  | "clipCount"
  | "cut";

export type ReframeMode = "vertical_crop" | "smart_face_track" | "original";
export type SettingsTab = "ai" | "storage" | "export" | "system";

// ===== Job System Types =====

export type JobState =
  | "Queued"
  | "Analyzing"
  | "Processing"
  | "Encoding"
  | "Completed"
  | "Failed"
  | "Cancelled";

export type JobInfo = {
  id: string;
  project_id: string;
  state: JobState;
  progress: number;
  stage: string;
  error?: string | null;
  created_at_ms: number;
  completed_at_ms?: number | null;
};

// ===== Export Preset Types =====

export type ExportPresetPlatform =
  "instagram_reels" | "youtube_shorts" | "tiktok" | "custom";

export type ExportPresetConfig = {
  platform: ExportPresetPlatform;
  label: string;
  width: number;
  height: number;
  aspectRatio: string;
  fps: number;
  bitrateKbps: number;
  iconName: string;
};
