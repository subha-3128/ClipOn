use super::filters::{build_dynamic_crop_expr, PodcastKeyframe};
pub use crate::dynamic_podcast_reframing::{
    DynamicPodcastReframing, DynamicPodcastReframingResult, LayoutSegment, PersonKeyframe,
    PersonTrack, PodcastFaceTracking,
};

pub type FaceTrackerResult = DynamicPodcastReframingResult;

pub fn detect_faces_full(
    source_path: &str,
    start_sec: f64,
    duration_sec: f64,
) -> FaceTrackerResult {
    DynamicPodcastReframing::analyze(source_path, start_sec, duration_sec)
}

/// Detect face horizontal center X in normalized coords [0.0, 1.0] using Apple Vision.
pub fn detect_face_center_x(source_path: &str, start_sec: f64, duration_sec: f64) -> f64 {
    detect_faces_full(source_path, start_sec, duration_sec)
        .avg_center_x
        .clamp(0.20, 0.80)
}

pub fn build_segment_filter_graph(
    layout_type: &str,
    assigned_person_ids: &[usize],
    people: &[PersonTrack],
    iw_f: f64,
    ih_f: f64,
    seg_start: f64,
    seg_end: f64,
    fallback_p1_x: f64,
    fallback_p1_y: f64,
    fallback_p2_x: f64,
    fallback_p2_y: f64,
) -> String {
    let get_person_kfs = |person_id: usize| -> Vec<PodcastKeyframe> {
        if let Some(p) = people.iter().find(|pt| pt.id == person_id) {
            let mut out = Vec::new();
            for kf in &p.keyframes {
                if kf.t >= seg_start - 1.0 && kf.t <= seg_end + 1.0 {
                    out.push(PodcastKeyframe {
                        t: (kf.t - seg_start).max(0.0),
                        x: kf.x,
                        y: kf.y,
                    });
                }
            }
            if !out.is_empty() {
                return out;
            }
            if let Some(first) = p.keyframes.first() {
                return vec![PodcastKeyframe {
                    t: 0.0,
                    x: first.x,
                    y: first.y,
                }];
            }
        }
        Vec::new()
    };

    match layout_type {
        "single" => {
            // Full 9:16 vertical crop centered/face-tracked on Person 1
            let ch = ih_f;
            let cw = ((ch * 9.0 / 16.0) / 2.0).round() * 2.0;
            let p_id = assigned_person_ids.first().copied().unwrap_or(1);
            let kfs = get_person_kfs(p_id);
            let (fb_x, fb_y) = if p_id == 2 {
                (fallback_p2_x, fallback_p2_y)
            } else if p_id == 3 {
                (0.50, fallback_p2_y)
            } else {
                (fallback_p1_x, fallback_p1_y)
            };
            let (x_expr, y_expr) = build_dynamic_crop_expr(
                if kfs.is_empty() { None } else { Some(&kfs) },
                iw_f,
                ih_f,
                cw,
                ch,
                fb_x,
                fb_y,
            );
            format!(
                "[0:v]crop={}:{}:{}:{},scale=1080:1920",
                cw as i64, ch as i64, x_expr, y_expr
            )
        }
        "split_three" => {
            // Two people top (540x960 each: Person 1 = top-left, Person 2 = top-right),
            // one person bottom (1080x960 full width: Person 3 = bottom)
            let ch_top = ((ih_f * 0.60) / 2.0).round() * 2.0;
            let cw_top = ((ch_top * 9.0 / 16.0) / 2.0).round() * 2.0;
            let ch_bot = ((ih_f * 0.50) / 2.0).round() * 2.0;
            let cw_bot = ((ch_bot * 9.0 / 8.0) / 2.0).round() * 2.0;

            let p1_id = assigned_person_ids.get(0).copied().unwrap_or(1);
            let p2_id = assigned_person_ids.get(1).copied().unwrap_or(2);
            let p3_id = assigned_person_ids.get(2).copied().unwrap_or(3);

            let p1_kfs = get_person_kfs(p1_id);
            let p2_kfs = get_person_kfs(p2_id);
            let p3_kfs = get_person_kfs(p3_id);

            let (p1_x, p1_y) = build_dynamic_crop_expr(
                if p1_kfs.is_empty() {
                    None
                } else {
                    Some(&p1_kfs)
                },
                iw_f,
                ih_f,
                cw_top,
                ch_top,
                fallback_p1_x,
                fallback_p1_y,
            );
            let (p2_x, p2_y) = build_dynamic_crop_expr(
                if p2_kfs.is_empty() {
                    None
                } else {
                    Some(&p2_kfs)
                },
                iw_f,
                ih_f,
                cw_top,
                ch_top,
                fallback_p2_x,
                fallback_p2_y,
            );
            let (p3_x, p3_y) = build_dynamic_crop_expr(
                if p3_kfs.is_empty() {
                    None
                } else {
                    Some(&p3_kfs)
                },
                iw_f,
                ih_f,
                cw_bot,
                ch_bot,
                0.50,
                fallback_p2_y,
            );

            format!(
                "[0:v]crop={}:{}:{}:{},scale=540:960[p1];                 [0:v]crop={}:{}:{}:{},scale=540:960[p2];                 [p1][p2]hstack[top_row];                 [0:v]crop={}:{}:{}:{},scale=1080:960[bot];                 [top_row][bot]vstack[stacked];                 [stacked]drawbox=x=0:y=956:w=1080:h=8:color=0x0a0d14@0.95:t=fill,                          drawbox=x=0:y=958:w=1080:h=3:color=0x38bdf8@0.9:t=fill,                          drawbox=x=537:y=0:w=6:h=960:color=0x0a0d14@0.95:t=fill,                          drawbox=x=539:y=0:w=2:h=960:color=0x38bdf8@0.9:t=fill",
                cw_top as i64, ch_top as i64, p1_x, p1_y,
                cw_top as i64, ch_top as i64, p2_x, p2_y,
                cw_bot as i64, ch_bot as i64, p3_x, p3_y,
            )
        }
        _ => {
            // Default "split_two": Two horizontal sections (Top = Person 1, Bottom = Person 2)
            let ch = ((ih_f * 0.50) / 2.0).round() * 2.0;
            let cw = ((ch * 9.0 / 8.0) / 2.0).round() * 2.0;

            let p1_id = assigned_person_ids.get(0).copied().unwrap_or(1);
            let p2_id = assigned_person_ids.get(1).copied().unwrap_or(2);

            let p1_kfs = get_person_kfs(p1_id);
            let p2_kfs = get_person_kfs(p2_id);

            let (top_x, top_y) = build_dynamic_crop_expr(
                if p1_kfs.is_empty() {
                    None
                } else {
                    Some(&p1_kfs)
                },
                iw_f,
                ih_f,
                cw,
                ch,
                fallback_p1_x,
                fallback_p1_y,
            );
            let (bot_x, bot_y) = build_dynamic_crop_expr(
                if p2_kfs.is_empty() {
                    None
                } else {
                    Some(&p2_kfs)
                },
                iw_f,
                ih_f,
                cw,
                ch,
                fallback_p2_x,
                fallback_p2_y,
            );

            format!(
                "[0:v]crop={}:{}:{}:{},scale=1080:960[top];                 [0:v]crop={}:{}:{}:{},scale=1080:960[bot];                 [top][bot]vstack[stacked];                 [stacked]drawbox=x=0:y=956:w=1080:h=8:color=0x0a0d14@0.95:t=fill,                          drawbox=x=0:y=958:w=1080:h=3:color=0x38bdf8@0.9:t=fill",
                cw as i64, ch as i64, top_x, top_y,
                cw as i64, ch as i64, bot_x, bot_y,
            )
        }
    }
}
