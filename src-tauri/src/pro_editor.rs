use crate::models::{CandidateDraft, NormalizedTranscript, TranscriptWord};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

// =========================================================================
// 1. SENTENCE & CONTEXT BOUNDARY SNAPPING ENGINE
// =========================================================================

/// Clean dangling discourse markers that weaken hooks.
/// Words like "And so", "So", "Because", "Like", "Well", "Then" at the very start
/// are advanced to the next word if sufficient clip duration remains.
fn clean_leading_discourse_marker(words: &[TranscriptWord], mut start_idx: usize) -> usize {
    if words.is_empty() || start_idx >= words.len() {
        return start_idx;
    }

    let filler_words = [
        "and", "so", "but", "because", "like", "well", "then", "or", "now", "plus", "also", "anyway",
    ];

    let first_text = words[start_idx].text.trim().to_lowercase();
    let cleaned = first_text.trim_matches(|c: char| !c.is_alphabetic());

    if filler_words.contains(&cleaned) && start_idx + 1 < words.len() {
        let gap = words[start_idx + 1].start - words[start_idx].end;
        if gap < 0.65 {
            start_idx += 1;
        }
    }

    start_idx
}

/// Snap candidate start and end to real spoken sentence / clause boundaries.
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

            // 1. Find the nearest word index to draft.start
            let mut start_idx = 0;
            let mut min_diff = f64::MAX;
            for (i, w) in words.iter().enumerate() {
                let diff = (w.start - draft.start).abs();
                if diff < min_diff {
                    min_diff = diff;
                    start_idx = i;
                }
            }

            // Look backwards up to 3.5 seconds to find a sentence boundary
            let mut best_start_idx = start_idx;
            let mut lookback = start_idx;
            while lookback > 0 && (words[start_idx].start - words[lookback].start) <= 3.5 {
                let prev_word = &words[lookback - 1];
                let curr_word = &words[lookback];
                let pause = curr_word.start - prev_word.end;

                let ends_punct = prev_word.text.ends_with('.')
                    || prev_word.text.ends_with('?')
                    || prev_word.text.ends_with('!');

                if ends_punct || pause >= 0.35 {
                    best_start_idx = lookback;
                    break;
                }
                lookback -= 1;
            }

            // Remove leading filler conjunction ("And", "So", etc.)
            best_start_idx = clean_leading_discourse_marker(words, best_start_idx);

            // 2. Find the nearest word index to draft.end
            let mut end_idx = words.len() - 1;
            min_diff = f64::MAX;
            for (i, w) in words.iter().enumerate() {
                let diff = (w.end - draft.end).abs();
                if diff < min_diff {
                    min_diff = diff;
                    end_idx = i;
                }
            }

            // Look forwards up to 4.0s for sentence conclusion (. ? !)
            let mut best_end_idx = end_idx;
            let mut lookforward = end_idx;
            while lookforward < words.len() && (words[lookforward].end - words[end_idx].end) <= 4.0 {
                let curr_word = &words[lookforward];
                let ends_punct = curr_word.text.ends_with('.')
                    || curr_word.text.ends_with('?')
                    || curr_word.text.ends_with('!');

                if ends_punct {
                    best_end_idx = lookforward;
                    break;
                }
                if lookforward + 1 < words.len() {
                    let next_word = &words[lookforward + 1];
                    if next_word.start - curr_word.end >= 0.40 {
                        best_end_idx = lookforward;
                        break;
                    }
                }
                lookforward += 1;
            }

            let mut final_start = words[best_start_idx].start.max(0.0);
            // Add 250ms vocal trail buffer to the end to prevent syllable cutoff
            let mut final_end = (words[best_end_idx].end + 0.25).min(transcript.duration);

            // Guarantee a minimum clip duration of 10.0s
            if final_end - final_start < 10.0 {
                if final_start + 10.0 <= transcript.duration {
                    final_end = final_start + 10.0;
                } else if final_end >= 10.0 {
                    final_start = final_end - 10.0;
                }
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

/// Calculate multi-modal audio energy, hook strength, and pacing scores.
pub fn calculate_audio_energy_scores(
    wav_path: Option<&Path>,
    drafts: &[CandidateDraft],
    transcript: &NormalizedTranscript,
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

            // Ideal viral short pacing is 150-205 WPM
            let pacing_score = (95.0 - ((wpm - 175.0).abs() * 0.35)).clamp(50.0, 98.0);

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

            // Audio Surge Score: 50 base, boosted by loud hook / vocal delivery
            let audio_score = (50.0
                + (hook_energy_ratio - 1.0) * 35.0
                + (peak_energy_ratio - 1.0) * 15.0)
                .clamp(45.0, 99.0);

            // Multi-modal composite blend: 40% LLM hook + 35% Audio Energy + 25% Speech Pacing
            let composite = (0.40 * draft.score + 0.35 * audio_score + 0.25 * pacing_score).round();
            enriched.score = composite.clamp(55.0, 99.0);

            // Enrich rationale with auditory insights
            if hook_energy_ratio >= 1.15 {
                let badge = format!(
                    "⚡ High Audio Energy ({:.1}x hook surge, {:.0} WPM). ",
                    hook_energy_ratio, wpm
                );
                enriched.rationale = format!("{}{}", badge, enriched.rationale);
            }

            enriched
        })
        .collect()
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

    let (font_name, font_size, primary_color, highlight_color, border_color, border_w, shadow_w) =
        if is_hormozi {
            ("Arial", 84, "&H00FFFFFF", "&H0000E6FF", "&H00000000", 6, 3)
        } else if is_neon {
            ("Arial", 80, "&H00FFFFFF", "&H00FFFF00", "&H00330000", 5, 4)
        } else if is_submagic {
            ("Arial", 82, "&H00FFFFFF", "&H0000E6FF", "&H00000000", 7, 2)
        } else {
            ("Arial", 78, "&H00FFFFFF", "&H0000E6FF", "&H00000000", 5, 2)
        };

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
        "Style: Default,{},{},{},{},{},&H80000000,-1,0,0,0,100,100,1,0,1,{},{},2,60,60,380,1\n\n",
        font_name, font_size, primary_color, highlight_color, border_color, border_w, shadow_w
    ));

    ass.push_str("[Events]\n");
    ass.push_str("Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n");

    let impact_keywords = [
        "money", "million", "millionaire", "secret", "viral", "crazy", "insane", "stop", "never",
        "always", "warning", "danger", "hack", "free", "rich", "power", "truth", "mistake", "fast",
    ];

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
                "Dialogue: 0,{},{},Default,,0,0,0,,{}\n",
                start_formatted, end_formatted, line_text
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_leading_discourse_marker() {
        let words = vec![
            TranscriptWord { text: "And".to_string(), start: 1.0, end: 1.2, speaker: None },
            TranscriptWord { text: "this".to_string(), start: 1.3, end: 1.6, speaker: None },
            TranscriptWord { text: "happened".to_string(), start: 1.7, end: 2.2, speaker: None },
        ];
        let idx = clean_leading_discourse_marker(&words, 0);
        assert_eq!(idx, 1);
    }

    #[test]
    fn test_snap_candidates_to_boundaries() {
        let words = vec![
            TranscriptWord { text: "Hello".to_string(), start: 0.0, end: 0.4, speaker: None },
            TranscriptWord { text: "world.".to_string(), start: 0.5, end: 0.9, speaker: None },
            TranscriptWord { text: "This".to_string(), start: 1.4, end: 1.8, speaker: None },
            TranscriptWord { text: "is".to_string(), start: 1.9, end: 2.1, speaker: None },
            TranscriptWord { text: "viral.".to_string(), start: 2.2, end: 2.7, speaker: None },
            TranscriptWord { text: "Next".to_string(), start: 3.5, end: 4.0, speaker: None },
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
            TranscriptWord { text: "Insane".to_string(), start: 0.0, end: 0.5, speaker: None },
            TranscriptWord { text: "Money".to_string(), start: 0.6, end: 1.1, speaker: None },
        ];
        let ass = generate_kinetic_ass(&words, 0.0, 2.0, "hormozi-kinetic");
        assert!(ass.contains("[Script Info]"));
        assert!(ass.contains("Dialogue:"));
        assert!(ass.contains("MONEY"));
    }
}
