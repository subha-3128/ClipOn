use crate::jobs;
use crate::media;
use crate::models::{NormalizedTranscript, TranscriptWord};
use crate::pro_editor;
use crate::services::project_service::documents_project_dir;
use crate::AppState;

pub async fn render_flat_clip_for_candidate(
    app: tauri::AppHandle,
    state: AppState,
    candidate_id: String,
    reframe_mode: Option<String>,
    output_dir: Option<String>,
    remove_silence: Option<bool>,
    punch_zoom: Option<bool>,
    studio_audio: Option<bool>,
    export_preset: Option<String>,
    layout_override: Option<String>,
) -> Result<String, String> {
    let db = state.db.clone();
    let data_dir = state.data_dir.clone();
    let mode = reframe_mode.clone();
    let out_dir = output_dir.clone();
    let silence_removal = remove_silence;
    let punch = punch_zoom.unwrap_or(false);
    let studio = studio_audio.unwrap_or(true);
    let job_mgr = state.jobs.clone();
    let app_clone = app.clone();
    let preset_name = export_preset.clone();
    let explicit_layout_override = layout_override.clone();

    let job_id = job_mgr.create_job_with_details(
        &candidate_id,
        Some(&candidate_id),
        None,
        "Queued in render queue",
    );
    let permit = job_mgr
        .semaphore()
        .acquire_owned()
        .await
        .map_err(|e| e.to_string())?;

    if job_mgr.is_cancelled(&job_id) {
        return Err("Cancelled by user".to_string());
    }

    job_mgr.update_progress(
        &job_id,
        jobs::JobState::Analyzing,
        20,
        "Analyzing media & faces",
        Some(&app),
    );

    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let (candidate, project) = db
            .get_candidate_with_project(&candidate_id)
            .map_err(|e| e.to_string())?;

        if job_mgr.is_cancelled(&job_id) {
            return Err("Cancelled by user".to_string());
        }

        db.update_clip_for_candidate(&candidate_id, "cutting", None, None, None)
            .map_err(|e| e.to_string())?;

        let output_path = documents_project_dir(&project, out_dir.as_deref())?
            .join("clips")
            .join(format!("clip-{:02}_flat.mp4", candidate.rank));

        let mut srt_path = None;
        let mut ass_path = None;
        let mut drawtext_filters = None;

        job_mgr.update_progress(
            &job_id,
            jobs::JobState::Processing,
            45,
            "Generating subtitles & layout",
            Some(&app_clone),
        );

        let probe = media::probe_media(&project.source_path).ok();
        let cropped_width = if let Some(p) = &probe {
            let iw = p.width.unwrap_or(1920) as f64;
            let ih = p.height.unwrap_or(1080) as f64;
            let w = (iw.min(ih * 9.0 / 16.0) / 2.0).floor() * 2.0;
            w as i64
        } else {
            1080
        };

        if let Ok(Some(transcript_record)) = db.latest_transcript(&project.id) {
            if let Ok(normalized) =
                serde_json::from_str::<NormalizedTranscript>(&transcript_record.raw_json)
            {
                let srt_content =
                    generate_srt(&normalized.words, candidate.start_sec, candidate.end_sec);
                let clip_srt_path = data_dir
                    .join("projects")
                    .join(&project.id)
                    .join(format!("clip-{}.srt", candidate.id));
                if std::fs::write(&clip_srt_path, srt_content).is_ok() {
                    srt_path = Some(clip_srt_path);
                }
                let style = project
                    .caption_style
                    .as_deref()
                    .unwrap_or("hormozi-kinetic");

                let is_split = mode.as_deref() == Some("podcast_split");

                // Generate kinetic ASS subtitles
                let ass_content = pro_editor::generate_kinetic_ass(
                    &normalized.words,
                    candidate.start_sec,
                    candidate.end_sec,
                    style,
                    is_split,
                );
                let clip_ass_path = data_dir
                    .join("projects")
                    .join(&project.id)
                    .join(format!("clip-{}.ass", candidate.id));
                if std::fs::write(&clip_ass_path, ass_content).is_ok() {
                    ass_path = Some(clip_ass_path);
                }

                let drawtext = build_drawtext_filters(
                    &normalized.words,
                    candidate.start_sec,
                    candidate.end_sec,
                    cropped_width,
                    style,
                    is_split,
                );
                if !drawtext.is_empty() {
                    drawtext_filters = Some(drawtext);
                }
            }
        }

        if job_mgr.is_cancelled(&job_id) {
            return Err("Cancelled by user".to_string());
        }

        let should_remove_silence = silence_removal.unwrap_or(false);
        job_mgr.update_progress(
            &job_id,
            jobs::JobState::Encoding,
            70,
            "Rendering & hardware encoding",
            Some(&app_clone),
        );

        let effective_override = explicit_layout_override
            .as_deref()
            .or(candidate.layout_override.as_deref());

        match media::render_flat_clip_with_job(
            &project.source_path,
            candidate.start_sec,
            candidate.end_sec,
            &output_path,
            drawtext_filters.as_deref(),
            ass_path.as_deref(),
            mode.as_deref(),
            should_remove_silence,
            punch,
            studio,
            Some(job_id.clone()),
            preset_name.as_deref(),
            effective_override,
        ) {
            Ok(path) => {
                let path_string = path.to_string_lossy().to_string();
                let srt_string = srt_path.map(|p| p.to_string_lossy().to_string());
                db.update_clip_for_candidate(
                    &candidate_id,
                    "done",
                    Some(&path_string),
                    srt_string.as_deref(),
                    None,
                )
                .map_err(|e| e.to_string())?;
                job_mgr.complete_job(&job_id, Some(&app_clone));
                Ok(path_string)
            }
            Err(error) => {
                let err_msg = error.to_string();
                if job_mgr.is_cancelled(&job_id) || err_msg.contains("cancelled") {
                    let _ = std::fs::remove_file(&output_path);
                    let _ = db.update_clip_for_candidate(
                        &candidate_id,
                        "failed",
                        None,
                        None,
                        Some("Cancelled by user"),
                    );
                    job_mgr.fail_job(&job_id, "Cancelled by user", Some(&app_clone));
                    return Err("Cancelled by user".to_string());
                }

                // Fallback retry rendering without captions overlay on any non-cancellation error
                match media::render_flat_clip_with_job(
                    &project.source_path,
                    candidate.start_sec,
                    candidate.end_sec,
                    &output_path,
                    None,
                    None,
                    mode.as_deref(),
                    false,
                    punch,
                    studio,
                    Some(job_id.clone()),
                    preset_name.as_deref(),
                    effective_override,
                ) {
                    Ok(path) => {
                        let path_string = path.to_string_lossy().to_string();
                        let srt_string = srt_path.map(|p| p.to_string_lossy().to_string());
                        let warning_msg = format!(
                            "Clip rendered successfully, but captions were skipped. Error: {}",
                            err_msg
                        );
                        db.update_clip_for_candidate(
                            &candidate_id,
                            "done",
                            Some(&path_string),
                            srt_string.as_deref(),
                            Some(&warning_msg),
                        )
                        .map_err(|e| e.to_string())?;
                        job_mgr.complete_job(&job_id, Some(&app_clone));
                        Ok(path_string)
                    }
                    Err(retry_err) => {
                        let message = retry_err.to_string();
                        if job_mgr.is_cancelled(&job_id) || message.contains("cancelled") {
                            let _ = std::fs::remove_file(&output_path);
                            let _ = db.update_clip_for_candidate(
                                &candidate_id,
                                "failed",
                                None,
                                None,
                                Some("Cancelled by user"),
                            );
                            job_mgr.fail_job(&job_id, "Cancelled by user", Some(&app_clone));
                            return Err("Cancelled by user".to_string());
                        }
                        db.update_clip_for_candidate(
                            &candidate_id,
                            "error",
                            None,
                            None,
                            Some(&message),
                        )
                        .map_err(|e| e.to_string())?;
                        job_mgr.fail_job(&job_id, &message, Some(&app_clone));
                        Err(message)
                    }
                }
            }
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn get_podcast_preview(
    source_path: String,
    start_sec: f64,
    duration_sec: f64,
) -> Result<media::DynamicPodcastReframingResult, String> {
    tokio::task::spawn_blocking(move || {
        Ok(media::detect_faces_full(
            &source_path,
            start_sec,
            duration_sec,
        ))
    })
    .await
    .map_err(|e| e.to_string())?
}

pub fn cancel_job(
    app: &tauri::AppHandle,
    state: &AppState,
    job_id: &str,
) -> Result<(), String> {
    state.jobs.cancel_job(job_id, Some(app))
}

pub fn get_active_jobs(state: &AppState) -> Result<Vec<jobs::JobInfo>, String> {
    Ok(state.jobs.list_active_jobs())
}

pub fn generate_srt(words: &[TranscriptWord], start_sec: f64, end_sec: f64) -> String {
    let mut srt = String::new();
    let mut index = 1;

    let candidate_words: Vec<&TranscriptWord> = words
        .iter()
        .filter(|w| w.end > start_sec && w.start < end_sec)
        .collect();

    for chunk in candidate_words.chunks(3) {
        if chunk.is_empty() {
            continue;
        }
        let first = chunk[0];
        let last = chunk[chunk.len() - 1];

        let start_rel = (first.start - start_sec).max(0.0);
        let end_rel = (last.end - start_sec).min(end_sec - start_sec).max(0.0);

        let text = chunk
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        srt.push_str(&format!("{index}\n"));
        srt.push_str(&format!("{}\n", format_srt_time(start_rel, end_rel)));
        srt.push_str(&format!("{text}\n\n"));
        index += 1;
    }

    srt
}

pub fn format_srt_time(start: f64, end: f64) -> String {
    let format_time = |secs: f64| {
        let hours = (secs / 3600.0) as u32;
        let mins = ((secs % 3600.0) / 60.0) as u32;
        let secs_only = (secs % 60.0) as u32;
        let ms = ((secs.fract()) * 1000.0) as u32;
        format!("{hours:02}:{mins:02}:{secs_only:02},{ms:03}")
    };
    format!("{} --> {}", format_time(start), format_time(end))
}

pub fn get_contextual_emoji(text: &str) -> Option<&'static str> {
    let lower = text.to_lowercase();
    if lower.contains("money")
        || lower.contains("dollar")
        || lower.contains("cash")
        || lower.contains("rich")
        || lower.contains("cost")
        || lower.contains("paid")
        || lower.contains("price")
    {
        Some("💰")
    } else if lower.contains("fire")
        || lower.contains("lit")
        || lower.contains("hot")
        || lower.contains("burn")
    {
        Some("🔥")
    } else if lower.contains("crazy")
        || lower.contains("mind")
        || lower.contains("insane")
        || lower.contains("shock")
        || lower.contains("unbelievable")
    {
        Some("🤯")
    } else if lower.contains("rocket")
        || lower.contains("fast")
        || lower.contains("speed")
        || lower.contains("growth")
        || lower.contains("explode")
        || lower.contains("scale")
    {
        Some("🚀")
    } else if lower.contains("laugh")
        || lower.contains("funny")
        || lower.contains("joke")
        || lower.contains("hilarious")
        || lower.contains("lol")
    {
        Some("😂")
    } else if lower.contains("party")
        || lower.contains("boat")
        || lower.contains("trip")
        || lower.contains("drink")
        || lower.contains("fun")
        || lower.contains("celebrat")
    {
        Some("🥳")
    } else if lower.contains("time")
        || lower.contains("late")
        || lower.contains("clock")
        || lower.contains("wait")
        || lower.contains("minute")
        || lower.contains("hour")
        || lower.contains("second")
    {
        Some("⏱️")
    } else if lower.contains("danger")
        || lower.contains("warn")
        || lower.contains("stop")
        || lower.contains("threat")
        || lower.contains("trouble")
    {
        Some("⚠️")
    } else if lower.contains("dead")
        || lower.contains("die")
        || lower.contains("kill")
        || lower.contains("skull")
    {
        Some("💀")
    } else if lower.contains("love")
        || lower.contains("heart")
        || lower.contains("best")
        || lower.contains("friend")
    {
        Some("❤️")
    } else if lower.contains("win")
        || lower.contains("won")
        || lower.contains("champ")
        || lower.contains("first")
        || lower.contains("trophy")
    {
        Some("🏆")
    } else if lower.contains("food")
        || lower.contains("eat")
        || lower.contains("dinner")
        || lower.contains("lunch")
        || lower.contains("cook")
    {
        Some("🍔")
    } else if lower.contains("look")
        || lower.contains("see")
        || lower.contains("watch")
        || lower.contains("eyes")
    {
        Some("👀")
    } else if lower.contains("secret") || lower.contains("quiet") || lower.contains("shh") {
        Some("🤫")
    } else {
        None
    }
}

pub fn build_drawtext_filters(
    words: &[TranscriptWord],
    start_sec: f64,
    end_sec: f64,
    cropped_width: i64,
    caption_style: &str,
    is_podcast_split: bool,
) -> String {
    let candidate_words: Vec<&TranscriptWord> = words
        .iter()
        .filter(|w| w.end > start_sec && w.start < end_sec)
        .collect();

    if candidate_words.is_empty() {
        return String::new();
    }

    // Build font option once for drawtext filter
    let mut font_paths = vec![
        // macOS
        "/System/Library/Fonts/Supplemental/Futura.ttc".to_string(),
        "/System/Library/Fonts/Avenir Next.ttc".to_string(),
        "/System/Library/Fonts/Supplemental/Arial Bold.ttf".to_string(),
        "/System/Library/Fonts/Helvetica.ttc".to_string(),
        // Windows standard
        "C:/Windows/Fonts/SegoeUIb.ttf".to_string(),
        "C:/Windows/Fonts/segoeuib.ttf".to_string(),
        "C:/Windows/Fonts/SegoeUI.ttf".to_string(),
        "C:/Windows/Fonts/segoeui.ttf".to_string(),
        "C:/Windows/Fonts/arialbd.ttf".to_string(),
        "C:/Windows/Fonts/arial.ttf".to_string(),
        // Linux
        "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf".to_string(),
        "/usr/share/fonts/truetype/freefont/FreeSansBold.ttf".to_string(),
    ];
    if let Ok(windir) = std::env::var("WINDIR").or_else(|_| std::env::var("SystemRoot")) {
        let windir_fonts = format!("{}/Fonts", windir.replace('\\', "/"));
        font_paths.push(format!("{}/SegoeUIb.ttf", windir_fonts));
        font_paths.push(format!("{}/segoeuib.ttf", windir_fonts));
        font_paths.push(format!("{}/SegoeUI.ttf", windir_fonts));
        font_paths.push(format!("{}/arialbd.ttf", windir_fonts));
        font_paths.push(format!("{}/arial.ttf", windir_fonts));
    }

    let mut font_option = String::new();
    for path in &font_paths {
        if std::path::Path::new(path).exists() {
            let normalized_path = path.replace('\\', "/");
            let escaped_path = normalized_path.replace('\'', "'\\''");
            font_option = format!("fontfile='{}':", escaped_path);
            break;
        }
    }

    let mut drawtext_filters = Vec::new();

    // Group into chunks of 2 words for fast-paced style captions
    for chunk in candidate_words.chunks(2) {
        if chunk.is_empty() {
            continue;
        }
        let first = chunk[0];
        let last = chunk[chunk.len() - 1];

        // Timestamps relative to clip start (due to fast input seeking resetting stream PTS)
        let start_rel = (first.start - start_sec).max(0.0);
        let end_rel = (last.end - start_sec).min(end_sec - start_sec).max(0.0);
        if end_rel <= start_rel {
            continue;
        }

        let text = chunk
            .iter()
            .map(|w| w.text.to_uppercase())
            .collect::<Vec<_>>()
            .join(" ");

        // Clean text to avoid breaking filter parameters
        let clean_text: String = text
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '!' || *c == '?')
            .collect();

        // Responsive font size and padding box
        let fontsize = ((cropped_width as f64) * 0.075).clamp(16.0, 80.0).round() as i64;
        let padding = ((fontsize as f64) * 0.3).clamp(4.0, 24.0).round() as i64;

        let emoji_suffix = get_contextual_emoji(&clean_text).unwrap_or("");
        let display_text = if !emoji_suffix.is_empty() {
            format!("{clean_text} {emoji_suffix}")
        } else {
            clean_text.clone()
        };

        let y_default = if is_podcast_split {
            "(h-text_h)/2"
        } else {
            "h*0.72"
        };
        let y_high = if is_podcast_split {
            "(h-text_h)/2"
        } else {
            "h*0.7"
        };
        let y_classic = if is_podcast_split {
            "(h-text_h)/2"
        } else {
            "h*0.65"
        };

        let drawtext = match caption_style {
            "submagic-viral" => {
                format!(
                    "drawtext={}text='{}':x=(w-text_w)/2:y={}:fontsize={}:fontcolor=0xFFE600:box=1:boxcolor=0x000000d0:boxborderw={}:shadowcolor=black@0.7:shadowx=2:shadowy=2:enable='between(t,{:.3},{:.3})'",
                    font_option, display_text, y_default, fontsize, padding, start_rel, end_rel
                )
            }
            "hormozi-punch" => {
                format!(
                    "drawtext={}text='{}':x=(w-text_w)/2:y={}:fontsize={}:fontcolor=black:box=1:boxcolor=0xFFE600f0:boxborderw={}:enable='between(t,{:.3},{:.3})'",
                    font_option, display_text, y_default, fontsize, padding, start_rel, end_rel
                )
            }
            "neon-glow" => {
                format!(
                    "drawtext={}text='{}':x=(w-text_w)/2:y={}:fontsize={}:fontcolor=0x00FFFF:borderw=3:bordercolor=0x003366:shadowcolor=black@0.8:shadowx=3:shadowy=3:enable='between(t,{:.3},{:.3})'",
                    font_option, display_text, y_high, fontsize, start_rel, end_rel
                )
            }
            "classic-outline" => {
                let borderw = ((fontsize as f64) * 0.1).clamp(2.0, 8.0).round() as i64;
                format!(
                    "drawtext={}text='{}':x=(w-text_w)/2:y={}:fontsize={}:fontcolor=yellow:borderw={}:bordercolor=black:enable='between(t,{:.3},{:.3})'",
                    font_option, clean_text, y_classic, fontsize, borderw, start_rel, end_rel
                )
            }
            "minimal-shadow" => {
                format!(
                    "drawtext={}text='{}':x=(w-text_w)/2:y={}:fontsize={}:fontcolor=white:shadowcolor=black@0.5:shadowx=2:shadowy=2:enable='between(t,{:.3},{:.3})'",
                    font_option, clean_text, y_high, fontsize, start_rel, end_rel
                )
            }
            "vibrant-cyan" => {
                format!(
                    "drawtext={}text='{}':x=(w-text_w)/2:y={}:fontsize={}:fontcolor=0x00FFFF:shadowcolor=black@0.6:shadowx=2:shadowy=2:enable='between(t,{:.3},{:.3})'",
                    font_option, clean_text, y_high, fontsize, start_rel, end_rel
                )
            }
            "vibrant-yellow-box" => {
                format!(
                    "drawtext={}text='{}':x=(w-text_w)/2:y={}:fontsize={}:fontcolor=black:box=1:boxcolor=0xffff00e0:boxborderw={}:enable='between(t,{:.3},{:.3})'",
                    font_option, clean_text, y_default, fontsize, padding, start_rel, end_rel
                )
            }
            "vibrant-green" => {
                let borderw = ((fontsize as f64) * 0.08).clamp(1.5, 6.0).round() as i64;
                format!(
                    "drawtext={}text='{}':x=(w-text_w)/2:y={}:fontsize={}:fontcolor=0x39FF14:borderw={}:bordercolor=black:shadowcolor=black@0.6:shadowx=2:shadowy=2:enable='between(t,{:.3},{:.3})'",
                    font_option, clean_text, y_high, fontsize, borderw, start_rel, end_rel
                )
            }
            "vibrant-red" => {
                let borderw = ((fontsize as f64) * 0.08).clamp(1.5, 6.0).round() as i64;
                format!(
                    "drawtext={}text='{}':x=(w-text_w)/2:y={}:fontsize={}:fontcolor=0xFF3B30:borderw={}:bordercolor=black:shadowcolor=black@0.6:shadowx=2:shadowy=2:enable='between(t,{:.3},{:.3})'",
                    font_option, clean_text, y_high, fontsize, borderw, start_rel, end_rel
                )
            }
            _ => {
                format!(
                    "drawtext={}text='{}':x=(w-text_w)/2:y={}:fontsize={}:fontcolor=white:box=1:boxcolor=0x000000b0:boxborderw={}:enable='between(t,{:.3},{:.3})'",
                    font_option, clean_text, y_default, fontsize, padding, start_rel, end_rel
                )
            }
        };
        drawtext_filters.push(drawtext);
    }

    drawtext_filters.join(",")
}
