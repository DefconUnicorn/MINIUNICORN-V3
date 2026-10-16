//! Patch-bay jack rendering and routing logic.
//!
//! Jacks are circular connectors: outputs get a unique colour, inputs start
//! grey and fade toward the colour of whatever output they're patched to.
//! "MIDI IN" is drawn as a 5-pin DIN instead.

use crate::layout;
use egui::{Align2, Color32, FontId, Pos2, Sense, Stroke, Vec2};
use std::collections::HashMap;

use super::widgets::{INK, PAPER};

/// Grey fill for unconnected inputs.
pub const JACK_OFF_COL: Color32 = Color32::from_rgb(160, 160, 160);

/// Unique colour per patch output (indexed by sorted output id).
pub const JACK_OUT_COLORS: [Color32; 9] = [
    Color32::from_rgb(200, 60, 120),
    Color32::from_rgb(80, 140, 220),
    Color32::from_rgb(180, 60, 60),
    Color32::from_rgb(140, 80, 180),
    Color32::from_rgb(40, 180, 180),
    Color32::from_rgb(100, 80, 180),
    Color32::from_rgb(180, 70, 150),
    Color32::from_rgb(70, 120, 190),
    Color32::from_rgb(120, 90, 160),
];

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

pub fn render_jack(
    ui: &mut egui::Ui,
    jack_id: &str,
    spec: &layout::JackSpec,
    output_colors: &HashMap<String, Color32>,
    time: f64,
) {
    let is_in = spec.is_input;
    let (rect, _resp) = ui.allocate_exact_size(Vec2::new(110.0, 96.0), Sense::hover());
    let center = rect.center();
    let painter = ui.painter_at(rect);
    if spec.label == "MIDI IN" {
        painter.circle_filled(center, 21.0, PAPER);
        painter.circle_stroke(center, 21.0, Stroke::new(2.0_f32, INK));
        painter.circle_stroke(center, 16.0, Stroke::new(1.0_f32, INK));
        for (x, y) in [
            (-8.0_f32, -7.0_f32),
            (-3.0, -11.0),
            (3.0, -11.0),
            (8.0, -7.0),
            (0.0, 8.0),
        ] {
            painter.circle_filled(center + Vec2::new(x, y), 2.5, INK);
        }
        painter.text(
            Pos2::new(center.x, rect.bottom() - 14.0),
            Align2::CENTER_CENTER,
            &spec.label,
            FontId::proportional(9.0),
            INK,
        );
        return;
    }
    painter.circle_stroke(center, 16.0, Stroke::new(2.0_f32, INK));
    let fill = if is_in && !spec.output_id.is_empty() {
        let out_col = output_colors.get(&spec.output_id).copied().unwrap_or(JACK_OFF_COL);
        let fade = ((time * 2.0).sin() as f32 + 1.0) / 2.0;
        Color32::from_rgb(
            (JACK_OFF_COL.r() as f32 + (out_col.r() as f32 - JACK_OFF_COL.r() as f32) * fade) as u8,
            (JACK_OFF_COL.g() as f32 + (out_col.g() as f32 - JACK_OFF_COL.g() as f32) * fade) as u8,
            (JACK_OFF_COL.b() as f32 + (out_col.b() as f32 - JACK_OFF_COL.b() as f32) * fade) as u8,
        )
    } else if is_in {
        JACK_OFF_COL
    } else {
        output_colors.get(jack_id).copied().unwrap_or(INK)
    };
    painter.circle_filled(center, 12.0, fill);
    painter.circle_stroke(center, 12.0, Stroke::new(1.5_f32, INK));
    let tri_y = if is_in { center.y + 22.0 } else { center.y - 22.0 };
    let tri = vec![
        Pos2::new(center.x, tri_y - 5.0),
        Pos2::new(center.x - 5.0, tri_y + 4.0),
        Pos2::new(center.x + 5.0, tri_y + 4.0),
    ];
    painter.add(egui::Shape::convex_polygon(tri, INK, Stroke::new(1.0_f32, INK)));
    painter.text(
        Pos2::new(center.x, rect.bottom() - 14.0),
        Align2::CENTER_CENTER,
        &spec.label,
        FontId::proportional(9.0),
        INK,
    );
}

// ---------------------------------------------------------------------------
// Routing logic
// ---------------------------------------------------------------------------

/// Stable colour for an output jack based on its sorted position among
/// all outputs in the canvas.
pub fn output_color(canvas: &layout::CanvasLayout, jack_id: &str) -> Color32 {
    let mut ids: Vec<&String> = canvas
        .jacks
        .iter()
        .filter(|(_, spec)| !spec.is_input)
        .map(|(id, _)| id)
        .collect();
    ids.sort();
    let index = ids.iter().position(|id| id.as_str() == jack_id).unwrap_or(0);
    JACK_OUT_COLORS[index % JACK_OUT_COLORS.len()]
}

/// Map a jack id to the route code byte written into `Params.jack_routes`.
pub fn output_route_code(jack_id: &str) -> u8 {
    match jack_id {
        "jack_lfosaw" => 1,
        "jack_lfosqr" => 2,
        "jack_mix" => 3,
        "jack_filtcont" => 4,
        "jack_loudcont" => 5,
        "jack_phones" => 6,
        "jack_main" => 7,
        _ => 0,
    }
}

/// Routing validity matrix: which outputs may connect to which inputs.
pub fn compatible_output(input_id: &str, output_id: &str) -> bool {
    match input_id {
        "jack_mods" => matches!(output_id, "jack_lfosaw" | "jack_lfosqr"),
        "jack_osc1v" | "jack_lfocv" | "jack_cutcv" | "jack_loudcv" => {
            matches!(output_id, "jack_filtcont" | "jack_loudcont")
        }
        "jack_ext" => matches!(output_id, "jack_mix" | "jack_phones" | "jack_main"),
        "jack_fcgate" | "jack_lcgate" => output_id == "jack_lfosqr",
        _ => false,
    }
}
