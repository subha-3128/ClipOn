use crate::media::face_tracker::VisionPersonTrack;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct TrackingKeyframe {
    pub t: f64,
    pub x: f64,
    pub y: f64,
}

pub type PersonKeyframe = TrackingKeyframe;

pub fn build_dynamic_crop_expr(
    keyframes: Option<&[TrackingKeyframe]>,
    iw: f64,
    ih: f64,
    cw: f64,
    ch: f64,
    fallback_cx: f64,
    fallback_cy: f64,
) -> (String, String) {
    let to_px = |cx: f64, cy: f64| -> (i64, i64) {
        let max_x = (iw - cw).max(0.0) as i64;
        let max_y = (ih - ch).max(0.0) as i64;
        let rx = (cx * iw - (cw / 2.0)).round() as i64;
        let ry = (cy * ih - (ch * 0.38)).round() as i64;
        let mut px = rx.clamp(0, max_x);
        let mut py = ry.clamp(0, max_y);
        px -= px % 2;
        py -= py % 2;
        (px, py)
    };

    let (fb_px, fb_py) = to_px(fallback_cx, fallback_cy);

    let kfs = match keyframes {
        Some(k) if !k.is_empty() => k,
        _ => return (format!("'{}'", fb_px), format!("'{}'", fb_py)),
    };

    if kfs.len() == 1 {
        let (px, py) = to_px(kfs[0].x, kfs[0].y);
        return (format!("'{}'", px), format!("'{}'", py));
    }

    let points: Vec<(f64, i64, i64)> = kfs
        .iter()
        .map(|kf| {
            let (px, py) = to_px(kf.x, kf.y);
            (kf.t, px, py)
        })
        .collect();

    let dead_zone_x = (iw * 0.012).max(12.0) as i64;
    let dead_zone_y = (ih * 0.012).max(12.0) as i64;

    let smooth_and_simplify = |extract_val: fn(&(f64, i64, i64)) -> i64,
                               dead_zone: i64,
                               max_vel_px: f64|
     -> Vec<(f64, i64)> {
        let raw: Vec<(f64, i64)> = points.iter().map(|p| (p.0, extract_val(p))).collect();
        if raw.len() <= 2 {
            return raw;
        }

        // Pass 1: Dead-zone hysteresis against micro-head movements and sensor wobble
        let mut steady = vec![raw[0]];
        for &curr in raw.iter().skip(1) {
            let prev_v = steady.last().unwrap().1;
            if (curr.1 - prev_v).abs() <= dead_zone {
                steady.push((curr.0, prev_v));
            } else {
                steady.push(curr);
            }
        }

        // Pass 2: Velocity damping and point simplification
        let mut simplified = vec![steady[0]];
        for i in 1..steady.len() - 1 {
            let prev = *simplified.last().unwrap();
            let curr = steady[i];
            let next = steady[i + 1];

            // Drop redundant colinear / stationary intermediate points
            if (curr.1 - prev.1).abs() <= 2 && (next.1 - curr.1).abs() <= 2 {
                continue;
            }

            // Damped velocity limit to eliminate camera whipping (unless scene cut)
            let dt = (curr.0 - prev.0).max(0.04);
            let dv = (curr.1 - prev.1) as f64;
            let vel = dv.abs() / dt;
            let is_scene_cut = dv.abs() > (iw * 0.22);

            let clamped_val = if !is_scene_cut && vel > max_vel_px {
                let max_delta = (max_vel_px * dt).round() as i64;
                if dv > 0.0 {
                    prev.1 + max_delta
                } else {
                    prev.1 - max_delta
                }
            } else {
                curr.1
            };

            simplified.push((curr.0, clamped_val));
        }
        simplified.push(*steady.last().unwrap());
        simplified
    };

    let pts_x = smooth_and_simplify(|p| p.1, dead_zone_x, 550.0);
    let pts_y = smooth_and_simplify(|p| p.2, dead_zone_y, 450.0);

    let build_expr = |pts: &[(f64, i64)], max_limit: i64| -> String {
        if pts.is_empty() {
            return "0".to_string();
        }
        if pts.len() == 1 {
            return format!("{}", pts[0].1);
        }

        let mut expr = format!("{}", pts.last().unwrap().1);
        for i in (0..pts.len() - 1).rev() {
            let (t0, v0) = pts[i];
            let (t1, v1) = pts[i + 1];
            let dt = (t1 - t0).max(0.05);
            let dv = v1 - v0;
            let segment = if dv == 0 {
                format!("{}", v0)
            } else if dv > 0 {
                format!("({}+{}*(t-{:.2})/{:.2})", v0, dv, t0, dt)
            } else {
                format!("({}-{}*(t-{:.2})/{:.2})", v0, dv.abs(), t0, dt)
            };
            expr = format!("if(lt(t,{:.2}),{},{})", t1, segment, expr);
        }
        format!("'2*trunc(min(max(0,{}),{})/2)'", expr, max_limit)
    };

    let max_x = (iw - cw).max(0.0) as i64;
    let max_y = (ih - ch).max(0.0) as i64;

    (build_expr(&pts_x, max_x), build_expr(&pts_y, max_y))
}

pub fn build_center_crop_filter() -> String {
    "crop=w='2*trunc(min(iw,ih*9/16)/2)':h='2*trunc(min(ih,iw*16/9)/2)',scale=1080:1920".to_string()
}

pub fn build_smart_face_crop_filter(center_x: f64) -> String {
    let crop_x = format!("min(max(0,({:.3}*iw-ow/2)),iw-ow)", center_x);
    format!(
        "crop=w='2*trunc(min(iw,ih*9/16)/2)':h='2*trunc(min(ih,iw*16/9)/2)':x='{}':y='(ih-oh)/2',scale=1080:1920",
        crop_x
    )
}

#[allow(dead_code)]
pub fn build_original_scale_filter() -> String {
    "scale='2*trunc(iw/2)':'2*trunc(ih/2)'".to_string()
}

/// Builds an FFmpeg video filter graph for multi-speaker layouts (single speaker vertical crop,
/// 2-person split screen, and 3-person split screen) with dynamic face tracking and dividers.
pub fn build_multi_speaker_layout_filter_graph(
    layout_type: &str,
    assigned_person_ids: &[usize],
    people: &[VisionPersonTrack],
    iw_f: f64,
    ih_f: f64,
    seg_start: f64,
    seg_end: f64,
    fallback_p1_x: f64,
    fallback_p1_y: f64,
    fallback_p2_x: f64,
    fallback_p2_y: f64,
) -> String {
    let get_person_kfs = |person_id: usize| -> Vec<TrackingKeyframe> {
        if let Some(p) = people.iter().find(|pt| pt.id == person_id) {
            let mut out = Vec::new();
            for kf in &p.keyframes {
                if kf.t >= seg_start - 1.0 && kf.t <= seg_end + 1.0 {
                    out.push(TrackingKeyframe {
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
                return vec![TrackingKeyframe {
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

            let p1_id = assigned_person_ids.first().copied().unwrap_or(1);
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
                "[0:v]crop={}:{}:{}:{},scale=540:960[p1];\
                 [0:v]crop={}:{}:{}:{},scale=540:960[p2];\
                 [p1][p2]hstack[top_row];\
                 [0:v]crop={}:{}:{}:{},scale=1080:960[bot];\
                 [top_row][bot]vstack[stacked];\
                 [stacked]drawbox=x=0:y=956:w=1080:h=8:color=0x0a0d14@0.95:t=fill,\
                          drawbox=x=0:y=958:w=1080:h=3:color=0x38bdf8@0.9:t=fill,\
                          drawbox=x=537:y=0:w=6:h=960:color=0x0a0d14@0.95:t=fill,\
                          drawbox=x=539:y=0:w=2:h=960:color=0x38bdf8@0.9:t=fill",
                cw_top as i64,
                ch_top as i64,
                p1_x,
                p1_y,
                cw_top as i64,
                ch_top as i64,
                p2_x,
                p2_y,
                cw_bot as i64,
                ch_bot as i64,
                p3_x,
                p3_y,
            )
        }
        _ => {
            // Default "split_two": Two horizontal sections (Top = Person 1, Bottom = Person 2)
            let ch = ((ih_f * 0.50) / 2.0).round() * 2.0;
            let cw = ((ch * 9.0 / 8.0) / 2.0).round() * 2.0;

            let p1_id = assigned_person_ids.first().copied().unwrap_or(1);
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
                "[0:v]crop={}:{}:{}:{},scale=1080:960[top];\
                 [0:v]crop={}:{}:{}:{},scale=1080:960[bot];\
                 [top][bot]vstack[stacked];\
                 [stacked]drawbox=x=0:y=956:w=1080:h=8:color=0x0a0d14@0.95:t=fill,\
                          drawbox=x=0:y=958:w=1080:h=3:color=0x38bdf8@0.9:t=fill",
                cw as i64, ch as i64, top_x, top_y, cw as i64, ch as i64, bot_x, bot_y,
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::face_tracker::VisionKeyframe;

    #[test]
    fn test_multi_speaker_layout_filter_graphs() {
        let p1 = VisionPersonTrack {
            id: 1,
            name: "Host".to_string(),
            keyframes: vec![
                VisionKeyframe {
                    t: 0.0,
                    x: 0.25,
                    y: 0.38,
                    width: 0.2,
                    height: 0.2,
                    confidence: 1.0,
                    visible: true,
                    state: None,
                },
                VisionKeyframe {
                    t: 5.0,
                    x: 0.30,
                    y: 0.38,
                    width: 0.2,
                    height: 0.2,
                    confidence: 1.0,
                    visible: true,
                    state: None,
                },
            ],
        };
        let p2 = VisionPersonTrack {
            id: 2,
            name: "Guest 1".to_string(),
            keyframes: vec![VisionKeyframe {
                t: 0.0,
                x: 0.75,
                y: 0.38,
                width: 0.2,
                height: 0.2,
                confidence: 1.0,
                visible: true,
                state: None,
            }],
        };
        let p3 = VisionPersonTrack {
            id: 3,
            name: "Guest 2".to_string(),
            keyframes: vec![VisionKeyframe {
                t: 0.0,
                x: 0.50,
                y: 0.38,
                width: 0.2,
                height: 0.2,
                confidence: 1.0,
                visible: true,
                state: None,
            }],
        };

        let people = vec![p1, p2, p3];

        // 1. Single layout
        let single_graph = build_multi_speaker_layout_filter_graph(
            "single",
            &[1],
            &people,
            1920.0,
            1080.0,
            0.0,
            10.0,
            0.26,
            0.38,
            0.78,
            0.38,
        );
        assert!(single_graph.contains("crop="));
        assert!(single_graph.contains("scale=1080:1920"));

        // 2. Split two layout
        let split_two_graph = build_multi_speaker_layout_filter_graph(
            "split_two",
            &[1, 2],
            &people,
            1920.0,
            1080.0,
            0.0,
            10.0,
            0.26,
            0.38,
            0.78,
            0.38,
        );
        assert!(split_two_graph.contains("scale=1080:960[top]"));
        assert!(split_two_graph.contains("scale=1080:960[bot]"));
        assert!(split_two_graph.contains("[top][bot]vstack[stacked]"));
        assert!(split_two_graph.contains("drawbox="));

        // 3. Split three layout
        let split_three_graph = build_multi_speaker_layout_filter_graph(
            "split_three",
            &[1, 2, 3],
            &people,
            1920.0,
            1080.0,
            0.0,
            10.0,
            0.26,
            0.38,
            0.78,
            0.38,
        );
        assert!(split_three_graph.contains("[p1][p2]hstack[top_row]"));
        assert!(split_three_graph.contains("[top_row][bot]vstack[stacked]"));
    }
}
