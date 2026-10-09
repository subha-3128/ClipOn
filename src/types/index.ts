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
  activeSpeakerProvider?: string;
  activeSpeakerStatus?: string;
  hasInstagramToken: boolean;
  hasYoutubeConfig?: boolean;
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

export type CaptionOption = {
  style: "hook_focused" | "conversational" | "insight_focused" | string;
  title: string;
  hook: string;
  text: string;
};

export type SocialKit = {
  candidateId: string;
  titles: string[];
  description: string;
  hashtags: string[];
  callToAction: string;
  captionOptions?: CaptionOption[];
};

export type ClipQualityScore = {
  hook: number | null;
  coherence: number | null;
  contextIndependence: number | null;
  payoff: number | null;
  speechQuality: number | null;
  visualQuality: number | null;
  boundaryQuality: number | null;
  redundancyPenalty: number;
  riskPenalty: number;
  total: number;
  version: number;
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
  socialKit?: SocialKit | null;
  qualityScore?: ClipQualityScore | null;
};

export type CandidateFeedback = {
  id: string;
  candidateId: string;
  projectId: string;
  action:
    | "kept"
    | "rejected"
    | "boundary_edit"
    | "crop_edit"
    | "caption_edit"
    | "rating"
    | "published"
    | string;
  rating?: number | null;
  detailsJson?: string | null;
  createdAt: string;
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

export type YouTubePost = {
  id: string;
  candidateId: string;
  clipId: string | null;
  status: "queued" | "publishing" | "published" | "failed";
  title: string | null;
  videoId: string | null;
  videoUrl: string | null;
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
  youtubePosts?: YouTubePost[];
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
export type SettingsTab =
  "ai" | "transcription" | "video" | "social" | "storage" | "system";

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

// ===== Engine & Error Types =====

export type LlmEngine =
  "claude" | "deepseek" | "local" | "gemini" | "openai" | "openrouter" | "groq";

export type ErrorSeverity = "info" | "warning" | "error" | "fatal";

export type ErrorAction = {
  label: string;
  onClick: () => void;
};

export type AppError = {
  id: string;
  code: string;
  message: string;
  details?: string;
  severity: ErrorSeverity;
  recoverable: boolean;
  action?: ErrorAction;
  timestamp: number;
};
