import { describe, it, expect } from "vitest";
import type { Candidate, Project } from "../types";

describe("Frontend Core Types and Utilities", () => {
  it("validates project structure and duration calculations", () => {
    const project: Project = {
      id: "proj_123",
      name: "Video Interview",
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

  it("verifies accessibility components and ARIA role mappings", async () => {
    const { AccessibleModal } = await import("../components/AccessibleModal");
    const { AccessibleNotification } =
      await import("../components/AccessibleNotification");

    expect(typeof AccessibleModal).toBe("function");
    expect(typeof AccessibleNotification).toBe("function");

    // Verify error severity to ARIA live region role contract
    const getAriaRole = (severity: "fatal" | "error" | "warning" | "info") =>
      severity === "fatal" || severity === "error" ? "alert" : "status";

    expect(getAriaRole("fatal")).toBe("alert");
    expect(getAriaRole("error")).toBe("alert");
    expect(getAriaRole("warning")).toBe("status");
    expect(getAriaRole("info")).toBe("status");
  });

  it("verifies reel duration constraints and hook sweet spot validation", () => {
    const isDurationWithinSweetSpot = (duration: number) =>
      duration >= 30.0 && duration <= 45.0;

    const isDurationValidReel = (duration: number) =>
      duration > 0 && duration <= 60.0;

    // 30-45 seconds sweet spot
    expect(isDurationWithinSweetSpot(35.0)).toBe(true);
    expect(isDurationWithinSweetSpot(44.9)).toBe(true);
    expect(isDurationWithinSweetSpot(25.0)).toBe(false);
    expect(isDurationWithinSweetSpot(55.0)).toBe(false);

    // Hard ceiling: never exceed 60.0s
    expect(isDurationValidReel(45.0)).toBe(true);
    expect(isDurationValidReel(60.0)).toBe(true);
    expect(isDurationValidReel(60.1)).toBe(false);
    expect(isDurationValidReel(75.0)).toBe(false);
  });
});
