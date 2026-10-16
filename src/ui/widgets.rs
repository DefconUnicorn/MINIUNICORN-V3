//! Widget primitives for the Model D panel: knobs, toggles, A/B selectors,
//! rotary switches, and waveform glyphs. Pure painting + input; no dependency
//! on ModelDApp or layout state.

use crate::params::OscWave;
use egui::{Align2, Color32, FontId, Pos2, Sense, Stroke, Vec2};

// --- ink on paper palette ---
pub const PAPER: Color32 = Color32::from_rgb(252, 252, 249);
pub const INK: Color32 = Color32::BLACK;
pub const SOFT: Color32 = Color32::from_rgb(90, 90, 90);
pub const RED_ON: Color32 = Color32::from_rgb(200, 40, 45);
pub const BLUE_ON: Color32 = Color32::from_rgb(45, 100, 220);
pub const BLACK_ON: Color32 = Color32::from_rgb(25, 25, 25);
pub const OFF_FILL: Color32 = Color32::from_rgb(245, 245, 242);
pub const HIGHLIGHT: Color32 = Color32::from_rgb(255, 200, 60);

/// Item ids whose widget exists in both orientations (Rotate 90°).
pub const ROCKER_IDS: &[&str] = &[
    "c_aba", "c_abb", "c_lfoshape", "c_oscmod", "o_ctrl3", "m_sw1", "m_sw2", "m_sw3",
    "m_swext", "m_swnoise", "m_wpink", "s_fmod", "s_mode", "s_key1", "s_key2", "s_fdec",
    "s_ldec", "u_main", "u_a440",
];

// ---------------------------------------------------------------------------
// Knob — label ABOVE, white face, black outline, pointer, 0/10 stops, value below.
// ---------------------------------------------------------------------------

pub fn knob(ui: &mut egui::Ui, label: &str, sub: impl AsRef<str>, v: &mut f32, size: f32) -> egui::Response {
    let h = 16.0 + size + 12.0 + 13.0;
    let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(size + 46.0, h), Sense::click_and_drag());
    if resp.dragged() {
        let d = resp.drag_delta();
        *v = (*v - d.y * 0.045 + d.x * 0.02).clamp(0.0, 10.0);
        resp.mark_changed();
    }
    if resp.hovered() {
        let scroll = ui.input(|i| i.raw_scroll_delta.y);
        if scroll != 0.0 {
            *v = (*v + scroll * 0.01).clamp(0.0, 10.0);
            resp.mark_changed();
        }
    }
    let painter = ui.painter_at(rect);
    painter.text(
        Pos2::new(rect.center().x, rect.min.y + 8.0),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(11.0),
        INK,
    );
    let center = Pos2::new(rect.center().x, rect.min.y + 16.0 + 4.0 + size / 2.0);
    let radius = size / 2.0;
    painter.circle_filled(center, radius + 1.5, INK);
    painter.circle_filled(center, radius, PAPER);
    painter.circle_stroke(center, radius, Stroke::new(1.5_f32, INK));
    for i in 0..=10 {
        let ang = (135.0 + (i as f32) * 27.0).to_radians();
        let (s, c) = ang.sin_cos();
        let p1 = Pos2::new(center.x + c * (radius - 1.0), center.y + s * (radius - 1.0));
        let p2 = Pos2::new(center.x + c * (radius - 5.0), center.y + s * (radius - 5.0));
        painter.line_segment([p1, p2], Stroke::new(1.0_f32, SOFT));
    }
    let ang = (135.0 + (*v / 10.0) * 270.0).to_radians();
    let (s, c) = ang.sin_cos();
    let tip = Pos2::new(center.x + c * (radius - 3.0), center.y + s * (radius - 3.0));
    painter.line_segment([center, tip], Stroke::new(2.5_f32, INK));
    painter.circle_filled(center, 2.5, SOFT);
    painter.text(
        Pos2::new(center.x - radius - 2.0, center.y + radius - 6.0),
        Align2::CENTER_CENTER,
        "0",
        FontId::proportional(9.0),
        SOFT,
    );
    painter.text(
        Pos2::new(center.x + radius + 2.0, center.y + radius - 6.0),
        Align2::CENTER_CENTER,
        "10",
        FontId::proportional(9.0),
        SOFT,
    );
    painter.text(
        Pos2::new(rect.center().x, rect.min.y + 16.0 + 4.0 + size + 8.0),
        Align2::CENTER_TOP,
        format!("{:.1} {}", *v, sub.as_ref()),
        FontId::monospace(10.0),
        SOFT,
    );
    if resp.double_clicked() {
        *v = 5.0;
        resp.mark_changed();
    }
    resp
}

// ---------------------------------------------------------------------------
// Slide-switch painter: coloured track + movable thumb.
// ---------------------------------------------------------------------------

pub fn paint_slide(
    painter: &egui::Painter,
    track: egui::Rect,
    on: bool,
    on_col: Color32,
    vertical: bool,
    thumb_col: Color32,
) {
    painter.rect_filled(track, 2.0, on_col);
    painter.rect_stroke(track, 2.0, Stroke::new(1.5_f32, INK));
    let c = if vertical {
        Pos2::new(
            track.center().x,
            if on { track.bottom() - 4.5 - 8.0 } else { track.top() + 4.5 + 8.0 },
        )
    } else {
        Pos2::new(
            if on { track.right() - 4.5 - 8.0 } else { track.left() + 4.5 + 8.0 },
            track.center().y,
        )
    };
    let thumb = egui::Rect::from_center_size(c, Vec2::new(16.0, 16.0));
    painter.rect_filled(thumb, 1.5, thumb_col);
    painter.rect_stroke(thumb, 1.5, Stroke::new(1.5_f32, INK));
}

// ---------------------------------------------------------------------------
// Toggle family
// ---------------------------------------------------------------------------

pub fn toggle(ui: &mut egui::Ui, label: &str, on: &mut bool, on_col: Color32) -> egui::Response {
    toggle_hc(ui, label, on, on_col, PAPER)
}

pub fn toggle_hc(
    ui: &mut egui::Ui,
    label: &str,
    on: &mut bool,
    track_col: Color32,
    thumb_col: Color32,
) -> egui::Response {
    let w = 112.0;
    let h = 13.0 + 4.0 + 22.0 + 4.0 + 13.0;
    let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let painter = ui.painter_at(rect);
    painter.text(
        Pos2::new(rect.center().x, rect.min.y + 6.5),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(9.0),
        INK,
    );
    paint_slide(
        &painter,
        egui::Rect::from_center_size(
            Pos2::new(rect.center().x, rect.min.y + 13.0 + 4.0 + 11.0),
            Vec2::new(64.0, 22.0),
        ),
        *on,
        track_col,
        false,
        thumb_col,
    );
    painter.text(
        Pos2::new(rect.center().x, rect.min.y + 13.0 + 4.0 + 22.0 + 4.0 + 6.5),
        Align2::CENTER_CENTER,
        if *on { "ON" } else { "off" },
        FontId::proportional(9.0),
        if *on { INK } else { SOFT },
    );
    resp
}

pub fn toggle_vc(
    ui: &mut egui::Ui,
    label: &str,
    on: &mut bool,
    track_col: Color32,
    thumb_col: Color32,
) -> egui::Response {
    let lines: Vec<String> = if label.contains('\n') {
        label.split('\n').map(|s| s.to_owned()).collect()
    } else {
        let w: Vec<&str> = label.split_whitespace().collect();
        match w.len() {
            0 => vec![String::new()],
            1 => vec![w[0].to_owned()],
            2 => vec![w[0].to_owned(), w[1].to_owned()],
            _ => vec![format!("{} {}", w[0], w[1]), w[2..].join(" ")],
        }
    };
    let label_h = (lines.len().max(1) as f32) * 12.0;
    let h = label_h + 4.0 + 64.0 + 4.0 + 13.0;
    let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(56.0, h), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let painter = ui.painter_at(rect);
    for (i, line) in lines.iter().enumerate() {
        painter.text(
            Pos2::new(rect.center().x, rect.min.y + 6.0 + (i as f32) * 12.0),
            Align2::CENTER_CENTER,
            line,
            FontId::proportional(9.0),
            INK,
        );
    }
    paint_slide(
        &painter,
        egui::Rect::from_center_size(
            Pos2::new(rect.center().x, rect.min.y + label_h + 4.0 + 32.0),
            Vec2::new(22.0, 64.0),
        ),
        *on,
        track_col,
        true,
        thumb_col,
    );
    painter.text(
        Pos2::new(rect.center().x, rect.min.y + label_h + 4.0 + 64.0 + 4.0 + 6.5),
        Align2::CENTER_CENTER,
        if *on { "ON" } else { "off" },
        FontId::proportional(9.0),
        if *on { INK } else { SOFT },
    );
    resp
}

pub fn toggle_r(
    ui: &mut egui::Ui,
    label: &str,
    on: &mut bool,
    track_col: Color32,
    thumb_col: Color32,
    vertical: bool,
) -> egui::Response {
    if vertical {
        toggle_vc(ui, label, on, track_col, thumb_col)
    } else {
        toggle_hc(ui, label, on, track_col, thumb_col)
    }
}

// ---------------------------------------------------------------------------
// A/B selector family (2-position, no off state)
// ---------------------------------------------------------------------------

pub fn ab_r(
    ui: &mut egui::Ui,
    a: &str,
    b: &str,
    is_b: &mut bool,
    col: Color32,
    vertical: bool,
) -> egui::Response {
    if vertical {
        ab_selector_v(ui, a, b, is_b, col)
    } else {
        ab_selector(ui, a, b, is_b, col)
    }
}

pub fn ab_selector(ui: &mut egui::Ui, a: &str, b: &str, is_b: &mut bool, col: Color32) -> egui::Response {
    let w = 112.0;
    let h = 1.0 + 22.0 + 4.0 + 13.0;
    let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
    if resp.clicked() {
        *is_b = !*is_b;
        resp.mark_changed();
    }
    let painter = ui.painter_at(rect);
    let track = egui::Rect::from_center_size(
        Pos2::new(rect.center().x, rect.min.y + 1.0 + 11.0),
        Vec2::new(64.0, 22.0),
    );
    paint_slide(&painter, track, *is_b, col, false, PAPER);
    let ly = rect.min.y + 1.0 + 22.0 + 4.0 + 6.5;
    painter.text(
        Pos2::new(rect.min.x + 1.0, ly),
        Align2::LEFT_CENTER,
        a,
        FontId::proportional(9.0),
        if *is_b { SOFT } else { INK },
    );
    painter.text(
        Pos2::new(rect.max.x - 1.0, ly),
        Align2::RIGHT_CENTER,
        b,
        FontId::proportional(9.0),
        if *is_b { INK } else { SOFT },
    );
    resp
}

pub fn ab_selector_v(ui: &mut egui::Ui, a: &str, b: &str, is_b: &mut bool, col: Color32) -> egui::Response {
    let w = 56.0;
    let h = 13.0 + 4.0 + 64.0 + 4.0 + 13.0;
    let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
    if resp.clicked() {
        *is_b = !*is_b;
        resp.mark_changed();
    }
    let painter = ui.painter_at(rect);
    painter.text(
        Pos2::new(rect.center().x, rect.min.y + 6.5),
        Align2::CENTER_CENTER,
        a,
        FontId::proportional(9.0),
        if *is_b { SOFT } else { INK },
    );
    let track = egui::Rect::from_center_size(
        Pos2::new(rect.center().x, rect.min.y + 13.0 + 4.0 + 32.0),
        Vec2::new(22.0, 64.0),
    );
    paint_slide(&painter, track, *is_b, col, true, PAPER);
    painter.text(
        Pos2::new(rect.center().x, rect.min.y + 13.0 + 4.0 + 64.0 + 4.0 + 6.5),
        Align2::CENTER_CENTER,
        b,
        FontId::proportional(9.0),
        if *is_b { INK } else { SOFT },
    );
    resp
}

// ---------------------------------------------------------------------------
// Rotary switch
// ---------------------------------------------------------------------------

pub fn rotary(ui: &mut egui::Ui, label: &str, idx: &mut usize, options: &[&str], size: f32) -> egui::Response {
    let n = options.len().max(1);
    *idx = (*idx).min(n - 1);
    let pad = 22.0;
    let w = size + pad * 2.0 + 8.0;
    let h = 15.0 + size + pad * 2.0 + 31.0;
    let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
    // Left-click steps clockwise, right-click steps counter-clockwise. No drag.
    if resp.clicked() {
        *idx = (*idx + 1) % n;
        resp.mark_changed();
    }
    if resp.secondary_clicked() {
        *idx = (*idx + n - 1) % n;
        resp.mark_changed();
    }
    let painter = ui.painter_at(rect);
    painter.text(
        Pos2::new(rect.center().x, rect.min.y + 7.0),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(11.0),
        INK,
    );
    let center = Pos2::new(rect.center().x, rect.min.y + 15.0 + pad + size / 2.0 + 12.0);
    let radius = size / 2.0;
    for (i, capaz) in options.iter().enumerate() {
        let ang = if n == 1 {
            -90.0f32.to_radians()
        } else {
            (135.0 + (i as f32) * (270.0 / (n as f32 - 1.0))).to_radians()
        };
        let (s, c) = ang.sin_cos();
        let pos = Pos2::new(center.x + c * (radius + 12.0), center.y + s * (radius + 12.0));
        let active = i == *idx;
        painter.text(
            pos,
            Align2::CENTER_CENTER,
            *capaz,
            FontId::proportional(9.0),
            if active { INK } else { SOFT },
        );
        if active {
            let r = egui::Rect::from_center_size(pos, Vec2::new(24.0, 12.0));
            painter.rect_stroke(r, 3.0, Stroke::new(1.2_f32, INK));
        }
        let p1 = Pos2::new(center.x + c * (radius + 2.0), center.y + s * (radius + 2.0));
        let p2 = Pos2::new(center.x + c * (radius + 6.0), center.y + s * (radius + 6.0));
        painter.line_segment([p1, p2], Stroke::new(1.0_f32, SOFT));
    }
    painter.circle_filled(center, radius + 1.5, INK);
    painter.circle_filled(center, radius, PAPER);
    painter.circle_stroke(center, radius, Stroke::new(1.5_f32, INK));
    let ang = if n == 1 {
        -90.0f32.to_radians()
    } else {
        (135.0 + (*idx as f32) * (270.0 / (n as f32 - 1.0))).to_radians()
    };
    let (s, c) = ang.sin_cos();
    let tip = Pos2::new(center.x + c * (radius - 3.0), center.y + s * (radius - 3.0));
    painter.line_segment([center, tip], Stroke::new(2.5_f32, INK));
    painter.text(
        Pos2::new(rect.center().x, rect.max.y - 7.0),
        Align2::CENTER_CENTER,
        options[*idx],
        FontId::monospace(10.0),
        SOFT,
    );
    resp
}

pub fn range_rotary(ui: &mut egui::Ui, range_oct: &mut i32) {
    const LABELS: [&str; 6] = ["LO", "32'", "16'", "8'", "4'", "2'"];
    let mut idx = (*range_oct + 3).clamp(0, 5) as usize;
    rotary(ui, "RANGE", &mut idx, &LABELS, 30.0)
        .on_hover_text("Left-click: up one range. Right-click: down one range.");
    *range_oct = idx as i32 - 3;
}

// ---------------------------------------------------------------------------
// Waveform glyph + WAVE rotary
// ---------------------------------------------------------------------------

pub fn wave_rotary(ui: &mut egui::Ui, osc_idx: usize, wave: &mut OscWave) {
    let opts = OscWave::for_osc(osc_idx);
    let mut idx = opts.iter().position(|w| *w == *wave).unwrap_or(2).min(5);
    wave_switch(ui, &opts, &mut idx, 28.0);
    *wave = opts[idx];
}

pub fn wave_glyph(
    painter: &egui::Painter,
    c: Pos2,
    w: f32,
    h: f32,
    wave: OscWave,
    col: Color32,
) {
    let x0 = c.x - w / 2.0;
    let x1 = c.x + w / 2.0;
    let y0 = c.y - h / 2.0;
    let y1 = c.y + h / 2.0;
    let st = Stroke::new(1.4_f32, col);
    let line = |a: Pos2, b: Pos2| painter.line_segment([a, b], st);
    match wave {
        OscWave::Triangle => {
            line(Pos2::new(x0, y1), Pos2::new(c.x, y0));
            line(Pos2::new(c.x, y0), Pos2::new(x1, y1));
        }
        OscWave::TriSaw => {
            line(Pos2::new(x0, y1), Pos2::new(c.x, y0));
            line(Pos2::new(c.x, y0), Pos2::new(x1, y0));
            line(Pos2::new(x1, y0), Pos2::new(x1, y1));
        }
        OscWave::Saw => {
            line(Pos2::new(x0, y1), Pos2::new(x1, y0));
            line(Pos2::new(x1, y0), Pos2::new(x1, y1));
        }
        OscWave::ReverseSaw => {
            line(Pos2::new(x0, y0), Pos2::new(x0, y1));
            line(Pos2::new(x0, y0), Pos2::new(x1, y1));
        }
        OscWave::Square => {
            line(Pos2::new(x0, y1), Pos2::new(x0, y0));
            line(Pos2::new(x0, y0), Pos2::new(x1, y0));
            line(Pos2::new(x1, y0), Pos2::new(x1, y1));
        }
        OscWave::WidePulse => {
            let a = x0 + w * 0.2;
            let b = x1 - w * 0.2;
            line(Pos2::new(x0, y1), Pos2::new(a, y1));
            line(Pos2::new(a, y1), Pos2::new(a, y0));
            line(Pos2::new(a, y0), Pos2::new(b, y0));
            line(Pos2::new(b, y0), Pos2::new(b, y1));
            line(Pos2::new(b, y1), Pos2::new(x1, y1));
        }
        OscWave::NarrowPulse => {
            let a = c.x - 1.5;
            let b = c.x + 1.5;
            line(Pos2::new(x0, y1), Pos2::new(a, y1));
            line(Pos2::new(a, y1), Pos2::new(a, y0));
            line(Pos2::new(a, y0), Pos2::new(b, y0));
            line(Pos2::new(b, y0), Pos2::new(b, y1));
            line(Pos2::new(b, y1), Pos2::new(x1, y1));
        }
    }
}

pub fn wave_switch(ui: &mut egui::Ui, opts: &[OscWave; 6], idx: &mut usize, size: f32) {
    let n = 6usize;
    *idx = (*idx).min(n - 1);
    let pad = 30.0;
    let w = size + pad * 2.0 + 8.0;
    let h = 15.0 + size + pad * 2.0 + 15.0;
    let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
    // Left-click steps clockwise, right-click steps counter-clockwise. No drag.
    if resp.clicked() {
        *idx = (*idx + 1) % n;
        resp.mark_changed();
    }
    if resp.secondary_clicked() {
        *idx = (*idx + n - 1) % n;
        resp.mark_changed();
    }
    let painter = ui.painter_at(rect);
    painter.text(
        Pos2::new(rect.center().x, rect.min.y + 7.0),
        Align2::CENTER_CENTER,
        "WAVE",
        FontId::proportional(11.0),
        INK,
    );
    let center = Pos2::new(rect.center().x, rect.min.y + 15.0 + pad + size / 2.0 - 8.0);
    let radius = size / 2.0;
    for (i, wv) in opts.iter().enumerate() {
        let ang = (135.0 + (i as f32) * (270.0 / 5.0)).to_radians();
        let (s, c) = ang.sin_cos();
        let pos = Pos2::new(center.x + c * (radius + 15.0), center.y + s * (radius + 15.0));
        wave_glyph(&painter, pos, 16.0, 11.0, *wv, if i == *idx { INK } else { SOFT });
        if i == *idx {
            painter.rect_stroke(
                egui::Rect::from_center_size(pos, Vec2::new(21.0, 15.0)),
                3.0,
                Stroke::new(1.2_f32, INK),
            );
        }
        let p1 = Pos2::new(center.x + c * (radius + 2.0), center.y + s * (radius + 2.0));
        let p2 = Pos2::new(center.x + c * (radius + 6.0), center.y + s * (radius + 6.0));
        painter.line_segment([p1, p2], Stroke::new(1.0_f32, SOFT));
    }
    painter.circle_filled(center, radius + 1.5, INK);
    painter.circle_filled(center, radius, PAPER);
    painter.circle_stroke(center, radius, Stroke::new(1.5_f32, INK));
    let ang = (135.0 + (*idx as f32) * (270.0 / 5.0)).to_radians();
    let (s, c) = ang.sin_cos();
    let tip = Pos2::new(center.x + c * (radius - 3.0), center.y + s * (radius - 3.0));
    painter.line_segment([center, tip], Stroke::new(2.5_f32, INK));
    resp.on_hover_text("Left-click: next waveform. Right-click: previous waveform.");
}
