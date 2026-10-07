use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct TrackingKeyframe {
    pub t: f64,
    pub x: f64,
    pub y: f64,
}

pub type PodcastKeyframe = TrackingKeyframe;

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

    let simplify = |extract_val: fn(&(f64, i64, i64)) -> i64| -> Vec<(f64, i64)> {
        let raw: Vec<(f64, i64)> = points.iter().map(|p| (p.0, extract_val(p))).collect();
        if raw.len() <= 2 {
            return raw;
        }
        let mut simplified = vec![raw[0]];
        for i in 1..raw.len() - 1 {
            let prev_v = simplified.last().unwrap().1;
            let curr_v = raw[i].1;
            let next_v = raw[i + 1].1;
            if (curr_v - prev_v).abs() <= 3 && (next_v - curr_v).abs() <= 3 {
                continue;
            }
            simplified.push(raw[i]);
        }
        simplified.push(*raw.last().unwrap());
        simplified
    };

    let pts_x = simplify(|p| p.1);
    let pts_y = simplify(|p| p.2);

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
