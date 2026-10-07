use crate::models::{CandidateDraft, NormalizedTranscript, TranscriptWord};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

// =========================================================================
// 1. SENTENCE & CONTEXT BOUNDARY SNAPPING ENGINE
// =========================================================================

/// Comprehensive list of conversational fillers, discourse markers, and greetings
/// that weaken opening hooks and should be stripped so the reel starts on the actual hook.
fn clean_leading_filler_and_greetings(
    words: &[TranscriptWord],
    mut start_idx: usize,
    max_advance: usize,
) -> usize {
    if words.is_empty() || start_idx >= words.len() {
        return start_idx;
    }

    let filler_words = [
        // Conjunctions & discourse markers
        "and", "so", "but", "because", "like", "well", "then", "or", "now", "plus",
        "also", "anyway", "actually", "basically", "honestly", "literally",
        // Conversational softeners & agreement
        "right", "yeah", "yes", "yep", "sure", "definitely", "absolutely", "exactly",
        "okay", "ok", "alright", "uh", "um", "ah",
        // Greetings (dead air on short-form reels)
        "hey", "hi", "hello", "welcome", "yo", "everyone", "guys", "folks",
    ];

    let stop_idx = (start_idx + max_advance).min(words.len() - 1);
    while start_idx < stop_idx {
        let first_text = words[start_idx].text.trim().to_lowercase();
        let cleaned = first_text.trim_matches(|c: char| !c.is_alphabetic());

        // Check single filler word
        if filler_words.contains(&cleaned) {
            let gap = words[start_idx + 1].start - words[start_idx].end;
            if gap < 0.85 {
                start_idx += 1;
                continue;
            }
        }

        // Check common two-word conversational preamble: "you know", "i mean", "like i", "so like"
        if start_idx + 1 < words.len() {
            let second_text = words[start_idx + 1].text.trim().to_lowercase();
            let second_cleaned = second_text.trim_matches(|c: char| !c.is_alphabetic());
            if (cleaned == "you" && second_cleaned == "know")
                || (cleaned == "i" && second_cleaned == "mean")
                || (cleaned == "i" && second_cleaned == "think")
                || (cleaned == "let" && second_cleaned == "me")
            {
                start_idx += 2;
                continue;
            }
        }

        break;
    }

    start_idx
}

/// Clean dangling discourse markers that weaken hooks (kept for backwards compatibility).
#[allow(dead_code)]
fn clean_leading_discourse_marker(words: &[TranscriptWord], start_idx: usize) -> usize {
    clean_leading_filler_and_greetings(words, start_idx, 1)
}

/// Matches the first 2-5 significant words of the candidate's hook in the transcript words
/// around draft.start (searching from draft.start - 3.5s to draft.start + 12.0s).
/// Returns the word index where the hook actually begins.
fn find_hook_start_in_transcript(
    words: &[TranscriptWord],
    draft_start: f64,
    hook_text: &str,
) -> Option<usize> {
    let clean_hook_words: Vec<String> = hook_text
        .split_whitespace()
        .map(|w| {
            w.to_lowercase()
                .trim_matches(|c: char| !c.is_alphanumeric())
                .to_string()
        })
        .filter(|w| !w.is_empty())
        .collect();

    if clean_hook_words.is_empty() {
        return None;
    }

    // Use up to the first 4 words of the hook as the search key
    let search_len = clean_hook_words.len().min(4);
    let search_key = &clean_hook_words[..search_len];

    // Find candidate word indices within time window [draft_start - 3.5, draft_start + 12.0]
    let window_start = (draft_start - 3.5).max(0.0);
    let window_end = draft_start + 12.0;

    let mut best_match_idx = None;
    let mut min_time_diff = f64::MAX;

    for i in 0..words.len() {
        if words[i].start < window_start {
            continue;
        }
        if words[i].start > window_end {
            break;
        }

        // Check if words starting at i match search_key
        if i + search_len <= words.len() {
            let mut matches = true;
            for j in 0..search_len {
                let w_clean = words[i + j]
                    .text
                    .to_lowercase()
                    .trim_matches(|c: char| !c.is_alphanumeric())
                    .to_string();
                if w_clean != search_key[j] {
                    matches = false;
                    break;
                }
            }

            if matches {
                let diff = (words[i].start - draft_start).abs();
                if diff < min_time_diff {
                    min_time_diff = diff;
                    best_match_idx = Some(i);
                }
            }
        }
    }

    best_match_idx
}

/// Snap candidate start and end to real spoken sentence / clause boundaries,
/// pinning start strictly to the hook and enforcing 30-45s sweet spot with a 60s hard ceiling.
pub fn snap_candidates_to_boundaries(
    drafts: &[CandidateDraft],
    transcript: &NormalizedTranscript,
) -> Vec<CandidateDraft> {
    if transcript.words.is_empty() {
        return drafts.to_vec();
    }

    let words = &transcript.words;

    drafts
        .iter()
        .map(|draft| {
            let mut snapped = draft.clone();

            // 1. Locate the exact opening moment of the hook
            let mut best_start_idx = if let Some(hook_idx) =
                find_hook_start_in_transcript(words, draft.start, &draft.hook)
            {
                hook_idx
            } else {
                // Fallback: find nearest word index to draft.start
                let mut start_idx = 0;
                let mut min_diff = f64::MAX;
                for (i, w) in words.iter().enumerate() {
                    let diff = (w.start - draft.start).abs();
                    if diff < min_diff {
                        min_diff = diff;
                        start_idx = i;
                    }
                }

                // If start_idx is in the middle of a sentence, check if the sentence began very recently (<= 1.5s)
                let mut lookback = start_idx;
                let mut sentence_start = start_idx;
                while lookback > 0 && (words[start_idx].start - words[lookback].start) <= 1.5 {
                    let prev_word = &words[lookback - 1];
                    let curr_word = &words[lookback];
                    let pause = curr_word.start - prev_word.end;
                    let prev_ends_punct = prev_word.text.ends_with('.')
                        || prev_word.text.ends_with('?')
                        || prev_word.text.ends_with('!');
                    if prev_ends_punct || pause >= 0.40 {
                        sentence_start = lookback;
                        break;
                    }
                    lookback -= 1;
                }
                sentence_start
            };

            // Strip any conversational greetings, intros, and filler words
            best_start_idx = clean_leading_filler_and_greetings(words, best_start_idx, 6);

            let final_start = words[best_start_idx].start.max(0.0);
            let max_allowed_end = (final_start + 60.0).min(transcript.duration);

            // 2. Select narrative conclusion boundary, prioritizing 30-45s and never exceeding 60s
            let mut end_idx = words.len() - 1;
            let mut min_diff = f64::MAX;
            for (i, w) in words.iter().enumerate() {
                let diff = (w.end - draft.end).abs();
                if diff < min_diff {
                    min_diff = diff;
                    end_idx = i;
                }
            }

            let mut best_end_idx = end_idx;

            // If draft.end exceeds 60s limit, search backwards for clean sentence ending under 60s
            if words[end_idx].end > max_allowed_end {
                let mut candidate_end = None;
                for i in (best_start_idx..=end_idx).rev() {
                    if words[i].end <= max_allowed_end {
                        let text = &words[i].text;
                        if text.ends_with('.') || text.ends_with('?') || text.ends_with('!') {
                            candidate_end = Some(i);
                            let dur = words[i].end - final_start;
                            if dur >= 30.0 && dur <= 45.0 {
                                break;
                            }
                        }
                    }
                }
                best_end_idx = candidate_end.unwrap_or_else(|| {
                    words
                        .iter()
                        .enumerate()
                        .rposition(|(_, w)| w.end <= max_allowed_end)
                        .unwrap_or(best_start_idx)
                });
            } else {
                // If duration is too short (< 25s) and more transcript is available, look forward
                let current_dur = words[end_idx].end - final_start;
                if current_dur < 25.0 && end_idx + 1 < words.len() {
                    for i in end_idx..words.len() {
                        if words[i].end > max_allowed_end {
                            break;
                        }
                        let text = &words[i].text;
                        if text.ends_with('.') || text.ends_with('?') || text.ends_with('!') {
                            best_end_idx = i;
                            let new_dur = words[i].end - final_start;
                            if new_dur >= 30.0 {
                                break;
                            }
                        }
                    }
                } else {
                    // Search locally (within +/- 3.5s) for natural sentence conclusion
                    let mut forward_idx = end_idx;
                    while forward_idx < words.len()
                        && (words[forward_idx].end - words[end_idx].end) <= 3.5
                        && words[forward_idx].end <= max_allowed_end
                    {
                        let curr_word = &words[forward_idx];
                        if curr_word.text.ends_with('.')
                            || curr_word.text.ends_with('?')
                            || curr_word.text.ends_with('!')
                        {
                            best_end_idx = forward_idx;
                            break;
                        }
                        if forward_idx + 1 < words.len() {
                            let next_word = &words[forward_idx + 1];
                            if next_word.start - curr_word.end >= 0.40 {
                                best_end_idx = forward_idx;
                                break;
                            }
                        }
                        forward_idx += 1;
                    }
                }
            }

            // Add 200ms vocal trail buffer to the end word
            let mut final_end = (words[best_end_idx].end + 0.20).min(max_allowed_end);

            // Absolute hard limit: NEVER exceed 60.0s
            if final_end - final_start > 60.0 {
                final_end = final_start + 60.0;
            }

            // Enforce minimum clip duration if possible
            if final_end - final_start < 10.0 && final_start + 10.0 <= transcript.duration {
                final_end = (final_start + 10.0).min(max_allowed_end);
            }

            snapped.start = (final_start * 100.0).round() / 100.0;
            snapped.end = (final_end * 100.0).round() / 100.0;
            snapped
        })
        .collect()
}

// =========================================================================
// 2. MULTI-MODAL AUDIO ENERGY & VIRAL HOOK SCORER
// =========================================================================

/// Read 16kHz mono 16-bit PCM WAV and calculate RMS amplitude blocks (0.25s each).
/// Returns (global_average_rms, Vec<(timestamp_sec, block_rms)>).
fn read_audio_rms_profile(wav_path: &Path) -> Option<(f64, Vec<(f64, f64)>)> {
    let mut file = File::open(wav_path).ok()?;
    let mut header = [0u8; 12];
    file.read_exact(&mut header).ok()?;

    if &header[0..4] != b"RIFF" || &header[8..12] != b"WAVE" {
        return None;
    }

    // Seek to "data" chunk
    let mut data_len = 0usize;
    let mut chunk_header = [0u8; 8];
    while file.read_exact(&mut chunk_header).is_ok() {
        let chunk_id = &chunk_header[0..4];
        let chunk_size = u32::from_le_bytes([
            chunk_header[4],
            chunk_header[5],
            chunk_header[6],
            chunk_header[7],
        ]) as usize;

        if chunk_id == b"data" {
            data_len = chunk_size;
            break;
        } else {
            if file.seek(SeekFrom::Current(chunk_size as i64)).is_err() {
                break;
            }
        }
    }

    if data_len == 0 {
        return None;
    }

    // Read PCM 16-bit samples (16000 samples/sec mono = 32000 bytes/sec)
    // 0.25s block = 4000 samples = 8000 bytes
    let block_samples = 4000usize;
    let block_bytes = block_samples * 2;
    let mut buffer = vec![0u8; block_bytes];

    let mut blocks = Vec::new();
    let mut total_rms_sum = 0.0;
    let mut block_idx = 0usize;

    loop {
        let bytes_read = match file.read(&mut buffer) {
            Ok(n) if n > 0 => n,
            _ => break,
        };

        let sample_count = bytes_read / 2;
        if sample_count == 0 {
            break;
        }

        let mut sum_sq = 0.0f64;
        for i in 0..sample_count {
            let sample = i16::from_le_bytes([buffer[i * 2], buffer[i * 2 + 1]]) as f64;
            let norm = sample / 32768.0;
            sum_sq += norm * norm;
        }

        let rms = (sum_sq / (sample_count as f64)).sqrt();
        let timestamp = (block_idx as f64) * 0.25;
        blocks.push((timestamp, rms));
        total_rms_sum += rms;
        block_idx += 1;
    }

    if blocks.is_empty() {
        return None;
    }

    let global_avg = total_rms_sum / (blocks.len() as f64);
    Some((global_avg, blocks))
}

/// Evaluates the linguistic viral potential and scroll-stopping power of an opening hook.
/// Returns a score between 45.0 and 99.0, along with descriptive badges.
pub fn evaluate_hook_linguistics(hook_text: &str) -> (f64, Vec<&'static str>) {
    let lower = hook_text.trim().to_lowercase();
    let words: Vec<&str> = lower.split_whitespace().collect();
    let mut score = 65.0f64;
    let mut badges = Vec::new();

    // 1. Intriguing Question Hook (curiosity gap)
    let is_question = hook_text.contains('?')
        || lower.starts_with("why ")
        || lower.starts_with("how ")
        || lower.starts_with("what if ")
        || lower.starts_with("have you ever ")
        || lower.starts_with("did you know ")
        || lower.starts_with("is it true ")
        || lower.starts_with("can you believe ")
        || lower.starts_with("who is ");
    if is_question {
        score += 20.0;
        badges.push("❓ Curiosity Question");
    }

    // 2. Bold / Controversial / Myth-Busting / Warning
    let contrarian_keywords = [
        "never", "stop", "mistake", "wrong", "lie", "truth", "secret", "nobody", "ruined",
        "failed", "myth", "worst", "don't", "avoid", "warning", "danger", "fake", "trap",
    ];
    if contrarian_keywords.iter().any(|k| lower.contains(k)) {
        score += 18.0;
        badges.push("⚡ Bold Contrarian Hook");
    }

    // 3. Quantitative / Specific / High Stakes
    let quantitative_patterns = [
        "99%", "90%", "80%", "50%", "million", "billion", "dollars", "0 to", "10x", "top 3",
        "3 things", "5 ways", "number one", "first time", "every single", "rules", "formula",
    ];
    if quantitative_patterns.iter().any(|p| lower.contains(p))
        || words.iter().any(|w| w.chars().any(|c| c.is_ascii_digit()))
    {
        score += 14.0;
        badges.push("📊 High-Stakes Metric");
    }

    // 4. Emotional / Storytelling Setup
    let narrative_keywords = [
        "crazy", "insane", "shocking", "changed my life", "i lost", "couldn't believe",
        "hardest thing", "story", "terrifying", "huge", "unexpected", "secret", "revealed",
    ];
    if narrative_keywords.iter().any(|k| lower.contains(k)) {
        score += 14.0;
        badges.push("🔥 Emotional Hook");
    }

    // 5. Promises of High Value / Breakthrough Formula
    let value_keywords = [
        "how to", "secret to", "the real reason", "this one thing", "the formula",
        "blueprint", "unlock", "double your", "10x your", "fastest way", "hack",
    ];
    if value_keywords.iter().any(|k| lower.contains(k)) {
        score += 12.0;
        badges.push("💡 High-Value Promise");
    }

    // 6. Penalty for Conversational Weak Intros & Greetings
    let weak_intro_patterns = [
        "hey guys", "welcome back", "hello everyone", "so basically", "i wanted to share",
        "in this video", "today i'm going to", "what's up guys",
    ];
    if weak_intro_patterns.iter().any(|p| lower.contains(p)) {
        score -= 15.0;
        badges.push("⚠️ Weak Conversational Intro");
    }

    // 7. Hook Brevity & Punchiness (first 3 seconds rule)
    if words.len() >= 4 && words.len() <= 16 {
        score += 8.0;
    } else if words.len() > 26 {
        score -= 10.0; // Overly long or rambling
    }

    (score.clamp(45.0, 99.0), badges)
}

/// Calculate viewer retention potential based on duration sweet spot (30-45s) and speech pacing.
pub fn calculate_retention_potential(duration_sec: f64, wpm: f64) -> (f64, Option<&'static str>) {
    // 1. Duration Retention Score (30-45s is the optimal short-form window. Absolute max 60s).
    let (duration_score, duration_badge) = if (30.0..=45.0).contains(&duration_sec) {
        (98.0, Some("🎯 30–45s Sweet Spot"))
    } else if (25.0..30.0).contains(&duration_sec) {
        (88.0, None)
    } else if (45.0..=55.0).contains(&duration_sec) {
        (84.0, None)
    } else if (55.0..=60.0).contains(&duration_sec) {
        (70.0, None)
    } else {
        (55.0, None)
    };

    // 2. Speech Velocity / Pacing Score (155-205 WPM is ideal)
    let pacing_score = (96.0 - ((wpm - 175.0).abs() * 0.35)).clamp(50.0, 98.0);

    let composite_retention = 0.55 * duration_score + 0.45 * pacing_score;
    (composite_retention, duration_badge)
}

/// Comprehensive multi-modal candidate scoring combining:
/// 1. Hook Quality (30%): Linguistic scroll-stopping power + vocal energy surge
/// 2. Viewer Retention & Pacing (30%): 30-45s sweet spot + speech tempo (WPM)
/// 3. Narrative Completeness & LLM Relevance (20%): Narrative arc & thought resolution
/// 4. Visual Quality & Active-Speaker Stability (20%): Active-speaker confidence + camera switch penalty
pub fn calculate_composite_reel_scores(
    wav_path: Option<&Path>,
    drafts: &[CandidateDraft],
    transcript: &NormalizedTranscript,
    asd_timeline: Option<&crate::media::active_speaker::ActiveSpeakerTimeline>,
) -> Vec<CandidateDraft> {
    let audio_profile = wav_path.and_then(read_audio_rms_profile);

    drafts
        .iter()
        .map(|draft| {
            let mut enriched = draft.clone();

            // 1. Calculate Speech Velocity (Words Per Minute - WPM)
            let clip_duration = (draft.end - draft.start).max(1.0);
            let word_count = transcript
                .words
                .iter()
                .filter(|w| w.start >= draft.start && w.end <= draft.end)
                .count();
            let wpm = (word_count as f64 / clip_duration) * 60.0;

            // 2. Calculate Audio Energy (Hook 3.5s surge and clip peak)
            let mut hook_energy_ratio = 1.0f64;
            let mut peak_energy_ratio = 1.0f64;

            if let Some((global_avg, ref blocks)) = audio_profile {
                if global_avg > 1e-4 {
                    // Hook window: first 3.5 seconds
                    let hook_end = draft.start + 3.5;
                    let hook_blocks: Vec<f64> = blocks
                        .iter()
                        .filter(|(t, _)| *t >= draft.start && *t <= hook_end)
                        .map(|(_, rms)| *rms)
                        .collect();

                    if !hook_blocks.is_empty() {
                        let hook_avg = hook_blocks.iter().sum::<f64>() / (hook_blocks.len() as f64);
                        hook_energy_ratio = hook_avg / global_avg;
                    }

                    // Peak block in clip
                    let clip_peak = blocks
                        .iter()
                        .filter(|(t, _)| *t >= draft.start && *t <= draft.end)
                        .map(|(_, rms)| *rms)
                        .fold(0.0f64, |a, b| a.max(b));

                    peak_energy_ratio = clip_peak / global_avg;
                }
            }

            // Audio Surge Score: if audio profile is available, measure surge; otherwise neutral 75.0
            let audio_score = if audio_profile.is_some() {
                (50.0 + (hook_energy_ratio - 1.0) * 35.0 + (peak_energy_ratio - 1.0) * 15.0)
                    .clamp(45.0, 99.0)
            } else {
                75.0
            };

            // 3. First-Class Hook Quality and Retention Potential Evaluation
            let (linguistic_score, hook_badges) = evaluate_hook_linguistics(&draft.hook);
            let (retention_potential, duration_badge) = calculate_retention_potential(clip_duration, wpm);

            // Hook composite: 60% linguistic scroll-stopper power + 40% audio vocal delivery/energy
            let hook_composite = 0.60 * linguistic_score + 0.40 * audio_score;

            // 4. Visual Quality & Active Speaker Validation
            let (visual_score, visual_badges) = if let Some(timeline) = asd_timeline {
                let relevant_segs: Vec<_> = timeline
                    .segments
                    .iter()
                    .filter(|s| s.end > draft.start && s.start < draft.end)
                    .collect();

                let mut v_score = if relevant_segs.is_empty() {
                    65.0
                } else {
                    let avg_conf: f64 = relevant_segs.iter().map(|s| s.confidence).sum::<f64>()
                        / relevant_segs.len() as f64;
                    75.0 + avg_conf * 20.0
                };

                let mut v_badges = Vec::new();

                // Camera-Switch Penalty: penalize rapid speaker flip-flops (> 4 switches per 30s)
                let switches = if relevant_segs.len() > 1 {
                    relevant_segs.windows(2).filter(|w| w[0].person_id != w[1].person_id).count()
                } else {
                    0
                };

                let max_acceptable_switches = ((clip_duration / 10.0).round() as usize).max(2);
                if switches > max_acceptable_switches {
                    let penalty = ((switches - max_acceptable_switches) as f64 * 3.5).min(15.0);
                    v_score -= penalty;
                } else if switches >= 1 {
                    v_badges.push("👥 Dynamic Dual-Speaker");
                } else {
                    v_badges.push("🎯 Focused Single-Speaker");
                }

                (v_score.clamp(45.0, 98.0), v_badges)
            } else {
                (85.0, Vec::new())
            };

            // Final Composite Ranking Blend:
            // 30% Hook Quality (linguistics + vocal energy)
            // 30% Viewer Retention & Pacing (duration 30-45s sweet spot + speech tempo)
            // 20% Narrative Completeness & LLM Content Relevance
            // 20% Visual Quality & Active-Speaker Framing Stability
            let llm_score_100 = (draft.score * 100.0).clamp(40.0, 100.0);
            let composite = (0.30 * hook_composite
                + 0.30 * retention_potential
                + 0.20 * llm_score_100
                + 0.20 * visual_score)
                .round();
            enriched.score = composite.clamp(45.0, 99.0);

            // Enrich rationale with hook badges and auditory/visual insights
            let mut prefix = String::new();
            if let Some(d_badge) = duration_badge {
                prefix.push_str(d_badge);
                prefix.push_str(". ");
            }
            if !hook_badges.is_empty() {
                prefix.push_str(hook_badges[0]);
                prefix.push_str(". ");
            }
            if !visual_badges.is_empty() {
                prefix.push_str(visual_badges[0]);
                prefix.push_str(". ");
            }
            if hook_energy_ratio >= 1.15 {
                prefix.push_str(&format!(
                    "⚡ High Audio Energy ({:.1}x hook surge, {:.0} WPM). ",
                    hook_energy_ratio, wpm
                ));
            }

            if !prefix.is_empty() {
                enriched.rationale = format!("{}{}", prefix, enriched.rationale);
            }

            enriched
        })
        .collect()
}

pub fn calculate_audio_energy_scores(
    wav_path: Option<&Path>,
    drafts: &[CandidateDraft],
    transcript: &NormalizedTranscript,
) -> Vec<CandidateDraft> {
    calculate_composite_reel_scores(wav_path, drafts, transcript, None)
}

// =========================================================================
// 3. HORMOZI KINETIC KARAOKE WORD-BY-WORD SUBTITLE GENERATOR (ASS)
// =========================================================================

/// Format timestamp into ASS format H:MM:SS.cs
fn format_ass_time(seconds: f64) -> String {
    let total_cs = (seconds.max(0.0) * 100.0).round() as u64;
    let cs = total_cs % 100;
    let total_s = total_cs / 100;
    let s = total_s % 60;
    let total_m = total_s / 60;
    let m = total_m % 60;
    let h = total_m / 60;
    format!("{}:{:02}:{:02}.{:02}", h, m, s, cs)
}

/// Generates an Advanced SubStation Alpha (.ass) subtitle file with word-by-word
/// kinetic pops, active word highlighting, and viral keyword emphasis.
pub fn generate_kinetic_ass(
    words: &[TranscriptWord],
    start_sec: f64,
    end_sec: f64,
    style: &str,
) -> String {
    let candidate_words: Vec<&TranscriptWord> = words
        .iter()
        .filter(|w| w.end > start_sec && w.start < end_sec)
        .collect();

    let is_hormozi = style == "hormozi-kinetic" || style == "hormozi-punch";
    let is_neon = style == "neon-glow";
    let is_submagic = style == "submagic-viral";

    let (
        font_name,
        font_size,
        primary_color,
        highlight_color,
        border_color,
        border_w,
        shadow_w,
    ) = if is_hormozi {
        ("Arial", 84, "&H00FFFFFF", "&H0000E6FF", "&H00000000", 6, 3)
    } else if is_neon {
        ("Arial", 80, "&H00FFFFFF", "&H00FFFF00", "&H00330000", 5, 4)
    } else if is_submagic {
        ("Arial", 82, "&H00FFFFFF", "&H0000E6FF", "&H00000000", 7, 2)
    } else {
        ("Arial", 78, "&H00FFFFFF", "&H0000E6FF", "&H00000000", 5, 2)
    };

    let (alignment, margin_v) = (2, 380); // Standard lower third

    let mut ass = String::new();
    ass.push_str("[Script Info]\n");
    ass.push_str("Title: ClipOn Kinetic Subtitles\n");
    ass.push_str("ScriptType: v4.00+\n");
    ass.push_str("PlayResX: 1080\n");
    ass.push_str("PlayResY: 1920\n");
    ass.push_str("ScaledBorderAndShadow: yes\n\n");

    ass.push_str("[V4+ Styles]\n");
    ass.push_str("Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\n");
    ass.push_str(&format!(
        "Style: Default,{},{},{},{},{},&H80000000,-1,0,0,0,100,100,1,0,1,{},{},{},60,60,{},1\n\n",
        font_name,
        font_size,
        primary_color,
        highlight_color,
        border_color,
        border_w,
        shadow_w,
        alignment,
        margin_v
    ));

    ass.push_str("[Events]\n");
    ass.push_str(
        "Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n",
    );

    let impact_keywords = [
        "money",
        "million",
        "millionaire",
        "secret",
        "viral",
        "crazy",
        "insane",
        "stop",
        "never",
        "always",
        "warning",
        "danger",
        "hack",
        "free",
        "rich",
        "power",
        "truth",
        "mistake",
        "fast",
    ];

    let pos_prefix = "";

    // Group into 2 or 3 words per line for high-velocity retention
    for chunk in candidate_words.chunks(3) {
        if chunk.is_empty() {
            continue;
        }

        for (active_idx, active_word) in chunk.iter().enumerate() {
            let word_start = (active_word.start - start_sec).max(0.0);
            let word_end = (active_word.end - start_sec).max(word_start + 0.08);

            if word_end <= word_start {
                continue;
            }

            let start_formatted = format_ass_time(word_start);
            let end_formatted = format_ass_time(word_end);

            let mut line_text = String::new();
            for (idx, w) in chunk.iter().enumerate() {
                let clean = w
                    .text
                    .chars()
                    .filter(|c| c.is_alphanumeric() || *c == '\'' || *c == '!' || *c == '?')
                    .collect::<String>()
                    .to_uppercase();

                if idx > 0 {
                    line_text.push(' ');
                }

                if idx == active_idx {
                    let is_impact = impact_keywords
                        .iter()
                        .any(|k| clean.to_lowercase().contains(k));
                    let color = if is_impact {
                        "&H14FF39&" // Electric neon green for impact keywords!
                    } else {
                        "&H0000E6FF&" // Vibrant TikTok/Hormozi yellow
                    };
                    line_text.push_str(&format!(r"{{\c{}\fscx114\fscy114}}{}{{\r}}", color, clean));
                } else {
                    line_text.push_str(&clean);
                }
            }

            ass.push_str(&format!(
                "Dialogue: 0,{},{},Default,,0,0,0,,{}{}\n",
                start_formatted, end_formatted, pos_prefix, line_text
            ));
        }
    }

    ass
}

// =========================================================================
// 4. DEAD-AIR SILENCE REMOVAL & JUMP-CUT DETECTOR
// =========================================================================

/// Parse silence start and end intervals from FFmpeg silencedetect output.
pub fn parse_silences_from_log(log: &str) -> Vec<(f64, f64)> {
    let mut silences = Vec::new();
    let mut current_start: Option<f64> = None;

    for line in log.lines() {
        if line.contains("silence_start:") {
            if let Some(pos) = line.find("silence_start:") {
                let rest = &line[pos + 14..].trim();
                if let Some(token) = rest.split_whitespace().next() {
                    if let Ok(val) = token.parse::<f64>() {
                        current_start = Some(val);
                    }
                }
            }
        } else if line.contains("silence_end:") {
            if let Some(start) = current_start.take() {
                if let Some(pos) = line.find("silence_end:") {
                    let rest = &line[pos + 12..].trim();
                    if let Some(token) = rest.split_whitespace().next() {
                        if let Ok(end) = token.parse::<f64>() {
                            if end > start && (end - start) >= 0.40 {
                                silences.push((start, end));
                            }
                        }
                    }
                }
            }
        }
    }

    silences
}

/// Build FFmpeg select / aselect expressions to cleanly omit silences.
/// Preserves a 60ms room-tone padding around speech so syllables are never clipped.
pub fn build_silence_jumpcut_filter(
    silences: &[(f64, f64)],
    total_duration: f64,
) -> Option<(String, String)> {
    if silences.is_empty() {
        return None;
    }

    let mut speech_intervals = Vec::new();
    let mut cursor = 0.0f64;

    for &(silence_start, silence_end) in silences {
        let segment_end = (silence_start + 0.06).min(total_duration);
        if segment_end > cursor + 0.15 {
            speech_intervals.push((cursor, segment_end));
        }
        cursor = (silence_end - 0.06).max(segment_end);
    }

    if cursor + 0.15 < total_duration {
        speech_intervals.push((cursor, total_duration));
    }

    if speech_intervals.len() <= 1 {
        return None;
    }

    let select_parts: Vec<String> = speech_intervals
        .iter()
        .map(|(s, e)| format!("between(t,{:.3},{:.3})", s, e))
        .collect();

    let combined = select_parts.join("+");
    let video_filter = format!("select='{}',setpts=N/FRAME_RATE/TB", combined);
    let audio_filter = format!("aselect='{}',asetpts=N/SR/TB", combined);

    Some((video_filter, audio_filter))
}

// =========================================================================
// 5. ATTENTION RETENTION PUNCH-ZOOM ENGINE
// =========================================================================

/// Builds an FFmpeg attention retention punch-zoom filter for arbitrary canvas width and height.
/// Punches 1.14x zoom for 1.6s every 5.5s interval to reset human visual attention.
pub fn build_punch_zoom_filter(width: i64, height: i64) -> String {
    format!(
        "crop=w='2*trunc(({}/if(lt(mod(t,5.5),1.6),1.14,1.0))/2)':h='2*trunc(({}/if(lt(mod(t,5.5),1.6),1.14,1.0))/2)':x='({}-ow)/2':y='({}-oh)/2',scale={}:{}",
        width, height, width, height, width, height
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_leading_discourse_marker() {
        let words = vec![
            TranscriptWord {
                text: "And".to_string(),
                start: 1.0,
                end: 1.2,
                speaker: None,
            },
            TranscriptWord {
                text: "this".to_string(),
                start: 1.3,
                end: 1.6,
                speaker: None,
            },
            TranscriptWord {
                text: "happened".to_string(),
                start: 1.7,
                end: 2.2,
                speaker: None,
            },
        ];
        let idx = clean_leading_discourse_marker(&words, 0);
        assert_eq!(idx, 1);
    }

    #[test]
    fn test_snap_candidates_to_boundaries() {
        let words = vec![
            TranscriptWord {
                text: "Hello".to_string(),
                start: 0.0,
                end: 0.4,
                speaker: None,
            },
            TranscriptWord {
                text: "world.".to_string(),
                start: 0.5,
                end: 0.9,
                speaker: None,
            },
            TranscriptWord {
                text: "This".to_string(),
                start: 1.4,
                end: 1.8,
                speaker: None,
            },
            TranscriptWord {
                text: "is".to_string(),
                start: 1.9,
                end: 2.1,
                speaker: None,
            },
            TranscriptWord {
                text: "viral.".to_string(),
                start: 2.2,
                end: 2.7,
                speaker: None,
            },
            TranscriptWord {
                text: "Next".to_string(),
                start: 3.5,
                end: 4.0,
                speaker: None,
            },
        ];
        let transcript = NormalizedTranscript {
            language: "en".to_string(),
            duration: 15.0,
            speakers: vec![],
            words,
            segments: vec![],
        };
        let drafts = vec![CandidateDraft {
            start: 1.5,
            end: 2.5,
            score: 85.0,
            hook: "Test Hook".to_string(),
            rationale: "Test".to_string(),
        }];
        let snapped = snap_candidates_to_boundaries(&drafts, &transcript);
        assert_eq!(snapped.len(), 1);
        assert_eq!(snapped[0].start, 1.4);
    }

    #[test]
    fn test_parse_silences_from_log() {
        let log = r#"
[silencedetect @ 0x123] silence_start: 3.456
[silencedetect @ 0x123] silence_end: 4.890 | silence_duration: 1.434
"#;
        let silences = parse_silences_from_log(log);
        assert_eq!(silences.len(), 1);
        assert_eq!(silences[0].0, 3.456);
        assert_eq!(silences[0].1, 4.890);
    }

    #[test]
    fn test_build_silence_jumpcut_filter() {
        let silences = vec![(2.0, 3.5)];
        let res = build_silence_jumpcut_filter(&silences, 6.0);
        assert!(res.is_some());
        let (v, a) = res.unwrap();
        assert!(v.contains("select='between"));
        assert!(a.contains("aselect='between"));
    }

    #[test]
    fn test_generate_kinetic_ass() {
        let words = vec![
            TranscriptWord {
                text: "Insane".to_string(),
                start: 0.0,
                end: 0.5,
                speaker: None,
            },
            TranscriptWord {
                text: "Money".to_string(),
                start: 0.6,
                end: 1.1,
                speaker: None,
            },
        ];
        let ass = generate_kinetic_ass(&words, 0.0, 2.0, "hormozi-kinetic");
        assert!(ass.contains("[Script Info]"));
        assert!(ass.contains("Dialogue:"));
        assert!(ass.contains("MONEY"));
    }

    #[test]
    fn test_build_punch_zoom_filter() {
        let f_9_16 = build_punch_zoom_filter(1080, 1920);
        assert!(f_9_16.contains("1080/if(lt(mod(t,5.5),1.6),1.14,1.0)"));
        assert!(f_9_16.contains("1920/if(lt(mod(t,5.5),1.6),1.14,1.0)"));
        assert!(f_9_16.contains("scale=1080:1920"));

        let f_16_9 = build_punch_zoom_filter(1920, 1080);
        assert!(f_16_9.contains("scale=1920:1080"));
    }

    #[test]
    fn test_find_hook_start_in_transcript_skips_greetings_and_filler() {
        let words = vec![
            TranscriptWord {
                text: "Hey".to_string(),
                start: 10.0,
                end: 10.3,
                speaker: None,
            },
            TranscriptWord {
                text: "guys,".to_string(),
                start: 10.4,
                end: 10.8,
                speaker: None,
            },
            TranscriptWord {
                text: "welcome".to_string(),
                start: 11.0,
                end: 11.4,
                speaker: None,
            },
            TranscriptWord {
                text: "back.".to_string(),
                start: 11.5,
                end: 11.9,
                speaker: None,
            },
            TranscriptWord {
                text: "Did".to_string(),
                start: 13.0,
                end: 13.3,
                speaker: None,
            },
            TranscriptWord {
                text: "you".to_string(),
                start: 13.4,
                end: 13.6,
                speaker: None,
            },
            TranscriptWord {
                text: "know".to_string(),
                start: 13.7,
                end: 14.1,
                speaker: None,
            },
            TranscriptWord {
                text: "that".to_string(),
                start: 14.2,
                end: 14.5,
                speaker: None,
            },
            TranscriptWord {
                text: "AI".to_string(),
                start: 14.6,
                end: 15.0,
                speaker: None,
            },
            TranscriptWord {
                text: "changed?".to_string(),
                start: 15.1,
                end: 15.7,
                speaker: None,
            },
        ];

        let idx = find_hook_start_in_transcript(&words, 10.0, "Did you know that AI changed?");
        assert_eq!(idx, Some(4));
        assert_eq!(words[idx.unwrap()].start, 13.0);
    }

    #[test]
    fn test_snap_candidates_enforces_60s_maximum_and_targets_sweet_spot() {
        let mut words = Vec::new();
        // Generate words spanning 90 seconds
        for i in 0..180 {
            let start = i as f64 * 0.5;
            let end = start + 0.4;
            let is_sentence_end = (i + 1) % 70 == 0 || (i + 1) == 180;
            let text = if is_sentence_end {
                format!("word{}.", i)
            } else {
                format!("word{}", i)
            };
            words.push(TranscriptWord {
                text,
                start,
                end,
                speaker: None,
            });
        }

        let transcript = NormalizedTranscript {
            language: "en".to_string(),
            duration: 90.0,
            speakers: vec![],
            words,
            segments: vec![],
        };

        // Draft requests 75 seconds (from 5.0 to 80.0)
        let drafts = vec![CandidateDraft {
            start: 5.0,
            end: 80.0,
            score: 0.90,
            hook: "Crazy hook".to_string(),
            rationale: "Long story".to_string(),
        }];

        let snapped = snap_candidates_to_boundaries(&drafts, &transcript);
        assert_eq!(snapped.len(), 1);
        let duration = snapped[0].end - snapped[0].start;
        // MUST NEVER exceed 60.0 seconds!
        assert!(duration <= 60.0, "Duration {} exceeded 60.0s", duration);
        assert!(duration >= 30.0, "Duration {} was below 30.0s", duration);
    }

    #[test]
    fn test_evaluate_hook_linguistics_scores_questions_and_contrarian() {
        let (q_score, q_badges) =
            evaluate_hook_linguistics("Why does 99% of people fail with AI?");
        assert!(q_score >= 85.0);
        assert!(q_badges.iter().any(|b| b.contains("Curiosity Question")));

        let (c_score, c_badges) =
            evaluate_hook_linguistics("Never make this shocking mistake in 2026.");
        assert!(c_score >= 85.0);
        assert!(c_badges.iter().any(|b| b.contains("Contrarian")));

        let (b_score, _) = evaluate_hook_linguistics("Today we discuss regular topics.");
        assert!(b_score < q_score);
    }

    #[test]
    fn test_calculate_retention_potential_favors_30_to_45s() {
        let (sweet_score, badge) = calculate_retention_potential(36.0, 175.0);
        assert!(sweet_score >= 90.0);
        assert!(badge.is_some());

        let (long_score, long_badge) = calculate_retention_potential(58.0, 175.0);
        assert!(long_score < sweet_score);
        assert!(long_badge.is_none());
    }

    #[test]
    fn test_reel_pipeline_hook_start_and_duration_sweet_spot() {
        // Construct realistic interview transcript:
        // [0.0 - 12.0s] Casual chatter / greetings
        // [12.4s] "Why do 99% of creators fail before making a single dollar?" (Hook)
        // [12.4s - 48.0s] Explanation and insight (35.6s of substance)
        // [48.0s] "Because consistency without strategy is loud noise." (Punchline / Payoff)
        // [48.5s - 90.0s] Next topic
        let mut words = Vec::new();
        let intro_words = ["Hey", "guys,", "welcome", "back", "to", "the", "show.", "Yeah,", "so", "glad", "to", "be", "here."];
        for (i, w) in intro_words.iter().enumerate() {
            words.push(TranscriptWord {
                text: w.to_string(),
                start: i as f64 * 0.9,
                end: (i as f64 * 0.9) + 0.7,
                speaker: Some("Host".to_string()),
            });
        }

        let hook_words = ["Why", "do", "99%", "of", "creators", "fail", "before", "making", "a", "single", "dollar?"];
        let hook_start_base = 12.4;
        for (i, w) in hook_words.iter().enumerate() {
            words.push(TranscriptWord {
                text: w.to_string(),
                start: hook_start_base + (i as f64 * 0.4),
                end: hook_start_base + (i as f64 * 0.4) + 0.35,
                speaker: Some("Guest".to_string()),
            });
        }

        // Story progression leading to punchline at ~48.0s
        for i in 0..70 {
            let start = 17.0 + (i as f64 * 0.44);
            let end = start + 0.38;
            let text = if i == 69 {
                "noise.".to_string()
            } else {
                format!("word{}", i)
            };
            words.push(TranscriptWord {
                text,
                start,
                end,
                speaker: Some("Guest".to_string()),
            });
        }

        let transcript = NormalizedTranscript {
            language: "en".to_string(),
            duration: 90.0,
            speakers: vec!["Host".to_string(), "Guest".to_string()],
            words,
            segments: vec![],
        };

        // Draft comes from LLM with initial rough bounds (e.g. 5.0 to 75.0)
        let drafts = vec![CandidateDraft {
            start: 5.0,
            end: 75.0,
            score: 0.88,
            hook: "Why do 99% of creators fail before making a single dollar?".to_string(),
            rationale: "Compelling creator economics insight".to_string(),
        }];

        // Step 1: Algorithmic Sentence & Hook Boundary Snapping
        let snapped = snap_candidates_to_boundaries(&drafts, &transcript);
        assert_eq!(snapped.len(), 1);

        let candidate = &snapped[0];
        // 1. Reel MUST start directly on the hook, skipping intro/greetings!
        assert!((candidate.start - 12.4).abs() < 0.1, "Start was {} instead of 12.4s", candidate.start);

        // 2. Reel MUST be within the 30-45s sweet spot (never > 60s)
        let dur = candidate.end - candidate.start;
        assert!(dur >= 30.0 && dur <= 45.0, "Duration was {}s, expected 30-45s", dur);
        assert!(dur <= 60.0, "Duration exceeded 60s max: {}s", dur);

        // Step 2: Multi-Modal Hook Quality & Retention Potential Scoring
        let scored = calculate_audio_energy_scores(None, &snapped, &transcript);
        assert_eq!(scored.len(), 1);
        let final_candidate = &scored[0];

        // 3. Score must be high due to curiosity question + quantitative metric + 30-45s sweet spot
        assert!(final_candidate.score >= 88.0, "Score was only {}", final_candidate.score);
        assert!(final_candidate.rationale.contains("30–45s Sweet Spot"));
        assert!(final_candidate.rationale.contains("Curiosity Question"));
    }
}
