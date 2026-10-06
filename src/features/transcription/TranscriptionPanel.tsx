import { useState, useMemo } from "react";
import { AudioLines, Loader2, Search, X } from "lucide-react";
import { BusyState, NormalizedTranscript } from "../../types";

interface TranscriptionPanelProps {
  transcript: NormalizedTranscript | null;
  busy: BusyState;
  canTranscribe: boolean;
  transcriptionEngine: "deepgram" | "local";
  onTranscribe: () => void;
  formatTime: (sec: number) => string;
}

export function TranscriptionPanel({
  transcript,
  busy,
  canTranscribe,
  transcriptionEngine,
  onTranscribe,
  formatTime,
}: TranscriptionPanelProps) {
  const [transcriptSearch, setTranscriptSearch] = useState("");

  const filteredSegments = useMemo(() => {
    if (!transcript) return [];
    if (!transcriptSearch.trim()) return transcript.segments;
    const query = transcriptSearch.toLowerCase();
    return transcript.segments.filter((s) =>
      s.text.toLowerCase().includes(query)
    );
  }, [transcript, transcriptSearch]);

  return (
    <section className="studio-panel transcript-studio">
      <div className="panel-header">
        <div>
          <h3>Interactive Transcript</h3>
          <p>
            {transcript
              ? `${transcript.segments.length} dialogue segments`
              : "Audio not transcribed yet"}
          </p>
        </div>

        <button
          className="studio-btn primary"
          onClick={onTranscribe}
          disabled={busy !== "idle" || !canTranscribe}
        >
          {busy === "transcribe" ? (
            <Loader2 className="spin" size={14} />
          ) : (
            <AudioLines size={14} />
          )}
          {transcript ? "Re-transcribe" : "Transcribe"}
        </button>
      </div>

      {transcript && (
        <div className="panel-search-bar">
          <Search size={14} className="search-icon" />
          <input
            type="text"
            placeholder="Search transcript text..."
            value={transcriptSearch}
            onChange={(e) => setTranscriptSearch(e.target.value)}
          />
          {transcriptSearch && (
            <button
              onClick={() => setTranscriptSearch("")}
              className="clear-search"
            >
              <X size={13} />
            </button>
          )}
        </div>
      )}

      <div className="transcript-scroll-area">
        {filteredSegments.length > 0 ? (
          filteredSegments.map((seg, idx) => (
            <div
              key={`${seg.start}-${idx}`}
              className="transcript-segment-card"
            >
              <div className="segment-meta">
                <span className="segment-time">{formatTime(seg.start)}</span>
                {seg.speaker && (
                  <span className="segment-speaker">{seg.speaker}</span>
                )}
              </div>
              <p className="segment-text">{seg.text}</p>
            </div>
          ))
        ) : transcript ? (
          <div className="empty-panel-state">
            <Search size={28} />
            <p>No matching transcript lines found</p>
          </div>
        ) : (
          <div className="empty-panel-state">
            <AudioLines size={36} />
            <h4>No Transcript Available</h4>
            <p>
              Run transcription to enable AI moment detection and automated
              captions.
            </p>
            <button
              className="studio-btn primary"
              onClick={onTranscribe}
              disabled={busy !== "idle" || !canTranscribe}
              style={{ marginTop: "12px" }}
            >
              {busy === "transcribe" ? (
                <Loader2 className="spin" size={14} />
              ) : (
                <AudioLines size={14} />
              )}
              Transcribe Video (
              {transcriptionEngine === "local"
                ? "Whisper Offline"
                : "Deepgram Cloud"}
              )
            </button>
          </div>
        )}
      </div>
    </section>
  );
}
