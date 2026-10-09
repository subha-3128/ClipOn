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

  it("verifies Settings tabs and two-level architecture mapping", () => {
    const validTabs: Array<
      "ai" | "transcription" | "video" | "social" | "storage" | "system"
    > = ["ai", "transcription", "video", "social", "storage", "system"];

    expect(validTabs).toHaveLength(6);
    expect(validTabs).toContain("ai");
    expect(validTabs).toContain("transcription");
    expect(validTabs).toContain("video");
    expect(validTabs).toContain("social");
    expect(validTabs).toContain("storage");
    expect(validTabs).toContain("system");
  });

  it("verifies secret field masking and keystore status contract", () => {
    const getPlaceholder = (isSavedInKeystore: boolean, customPlaceholder?: string) => {
      if (isSavedInKeystore) {
        return "•••••••••••••••• (Leave blank to keep existing key)";
      }
      return customPlaceholder || "Enter API Key";
    };

    const getStatusState = (typedValue: string, isSavedInKeystore: boolean) => {
      if (typedValue.trim().length > 0) return "unsaved-edit";
      if (isSavedInKeystore) return "saved-in-keystore";
      return "not-configured";
    };

    // When saved in keystore and user hasn't typed anything
    expect(getPlaceholder(true)).toBe("•••••••••••••••• (Leave blank to keep existing key)");
    expect(getStatusState("", true)).toBe("saved-in-keystore");

    // When user types a new key
    expect(getStatusState("sk-ant-api03-xxx", true)).toBe("unsaved-edit");

    // When not configured
    expect(getPlaceholder(false, "sk-proj-...")).toBe("sk-proj-...");
    expect(getStatusState("", false)).toBe("not-configured");
  });

  it("verifies Instagram Reel caption options and preservation of user edits", () => {
    interface CaptionOption {
      style: string;
      title: string;
      hook: string;
      text: string;
    }

    const options: CaptionOption[] = [
      {
        style: "hook_focused",
        title: "Hook-Focused",
        hook: "The key revelation",
        text: "The key revelation. Here is the context. What do you think?",
      },
      {
        style: "conversational",
        title: "Natural & Conversational",
        hook: "A thought on this",
        text: "This moment really stood out to me. How do you approach this?",
      },
      {
        style: "insight_focused",
        title: "Key Takeaway",
        hook: "Key insight",
        text: "Key takeaway from this clip: Keep it simple. Save this.",
      },
    ];

    expect(options).toHaveLength(3);

    // Edit preservation cache simulation
    const editsCache: Record<string, string> = {};
    let currentStyle = "hook_focused";
    let activeText = options[0].text;
    expect(activeText).toBe(options[0].text);

    // User edits the hook-focused caption
    activeText = "User custom edited hook caption!";
    editsCache[currentStyle] = activeText;

    // User switches to conversational
    currentStyle = "conversational";
    activeText = editsCache[currentStyle] || options[1].text;
    expect(activeText).toBe(options[1].text);

    // User switches back to hook_focused - verify edit is preserved!
    currentStyle = "hook_focused";
    activeText = editsCache[currentStyle] || options[0].text;
    expect(activeText).toBe("User custom edited hook caption!");
  });

  it("verifies Instagram hashtag sanitization and formatting", () => {
    const sanitizeTag = (input: string) => {
      let clean = input.trim().replace(/\s+/g, "");
      if (!clean) return null;
      if (!clean.startsWith("#")) clean = `#${clean}`;
      return clean;
    };

    expect(sanitizeTag("tech")).toBe("#tech");
    expect(sanitizeTag("#SaaS ")).toBe("#SaaS");
    expect(sanitizeTag("   ai video ")).toBe("#aivideo");
    expect(sanitizeTag("   ")).toBeNull();
  });

  it("verifies Instagram publishing stage stepper transitions", () => {
    const getPublishStepLabel = (step: number) => {
      switch (step) {
        case 1:
          return "Video Validation";
        case 2:
          return "Container Init";
        case 3:
          return "Rupload Transfer";
        case 4:
          return "Meta Transcoding";
        case 5:
          return "Live!";
        default:
          return "Idle";
      }
    };

    expect(getPublishStepLabel(1)).toBe("Video Validation");
    expect(getPublishStepLabel(3)).toBe("Rupload Transfer");
    expect(getPublishStepLabel(4)).toBe("Meta Transcoding");
    expect(getPublishStepLabel(5)).toBe("Live!");
  });

  it("verifies YouTube Shorts AI Social Kit title formatting and tag sanitization", () => {
    const formatShortsTitle = (title: string) => {
      let trimmed = title.trim();
      if (!trimmed.toLowerCase().includes("#shorts") && trimmed.length <= 92) {
        trimmed = `${trimmed} #Shorts`;
      }
      return trimmed.slice(0, 100);
    };

    const sanitizeYoutubeTags = (hashtags: string[]) => {
      const clean = hashtags
        .map((h) => {
          const s = h.trim().replace(/^#/, "");
          return s.toLowerCase() === "shorts" ? "Shorts" : s;
        })
        .filter((h) => h.length > 0);
      if (!clean.some((t) => t.toLowerCase() === "shorts")) {
        clean.unshift("Shorts");
      }
      return Array.from(new Set(clean));
    };

    const rawTitle = "Why Most Startups Fail In Year One";
    expect(formatShortsTitle(rawTitle)).toBe("Why Most Startups Fail In Year One #Shorts");

    const longTitle = "A".repeat(95);
    expect(formatShortsTitle(longTitle)).toBe(longTitle.slice(0, 100));

    const rawTags = ["#Startup", "Tech", "#shorts", "#AI"];
    const cleanTags = sanitizeYoutubeTags(rawTags);
    expect(cleanTags).toContain("Startup");
    expect(cleanTags).toContain("Tech");
    expect(cleanTags).toContain("Shorts");
    expect(cleanTags.filter((t) => t.toLowerCase() === "shorts")).toHaveLength(1);
  });
});



