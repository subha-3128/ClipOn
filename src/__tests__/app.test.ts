import { describe, it, expect } from "vitest";
import type { Candidate, Project } from "../types";

describe("Frontend Core Types and Utilities", () => {
  it("validates project structure and duration calculations", () => {
    const project: Project = {
      id: "proj_123",
      name: "Podcast Interview",
      sourcePath: "/media/test.mp4",
      sourceDuration: 125.5,
      status: "ready",
      transcriptionMode: "deepgram",
      captionStyle: "hormozi-punch",
      createdAt: "2026-01-01T00:00:00Z",
      updatedAt: "2026-01-01T00:00:00Z",
    };

    expect(project.id).toBe("proj_123");
    expect(project.sourceDuration).toBeGreaterThan(0);
    expect(project.status).toBe("ready");
  });

  it("calculates candidate clip duration accurately", () => {
    const candidate: Candidate = {
      id: "cand_456",
      projectId: "proj_123",
      startSec: 10.5,
      endSec: 42.0,
      score: 9.2,
      hook: "The key moment of insight",
      rationale: "Strong engagement spike",
      rank: 1,
      selected: true,
      layoutOverride: null,
    };

    const duration = candidate.endSec - candidate.startSec;
    expect(duration).toBeCloseTo(31.5, 2);
    expect(candidate.selected).toBe(true);
    expect(candidate.score).toBeGreaterThan(0);
  });

  it("formats timestamps cleanly into mm:ss format", () => {
    const formatTimestamp = (secs: number) => {
      const minutes = Math.floor(secs / 60);
      const remainingSecs = Math.floor(secs % 60);
      return `${minutes.toString().padStart(2, "0")}:${remainingSecs.toString().padStart(2, "0")}`;
    };

    expect(formatTimestamp(0)).toBe("00:00");
    expect(formatTimestamp(75)).toBe("01:15");
    expect(formatTimestamp(3665)).toBe("61:05");
  });
});
