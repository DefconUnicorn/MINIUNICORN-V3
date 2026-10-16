//! Text helpers: word-wrap for canvas labels, and sublabel formatters
//! that convert knob positions into human-readable units (Hz, ms, oct, etc.).

use crate::params;

/// Greedy word-wrap for canvas text items. Hard-splits overlong words.
pub fn reflow_text(text: &str, size: f32, max_w: f32) -> String {
    let char_w = (size * 0.55).max(1.0);
    let w = max_w.max(char_w);
    let measure = |s: &str| s.chars().count() as f32 * char_w;
    let space_w = char_w;
    let max_chars = ((w / char_w).floor() as usize).max(1);
    let mut out = String::new();
    for (pi, para) in text.split('\n').enumerate() {
        if pi > 0 {
            out.push('\n');
        }
        let mut line_w = 0.0f32;
        let mut first = true;
        for word in para.split_whitespace() {
            let ww = measure(word);
            if ww <= w {
                if !first && line_w + space_w + ww > w {
                    out.push('\n');
                    line_w = 0.0;
                    first = true;
                }
                if !first {
                    out.push(' ');
                    line_w += space_w;
                }
                out.push_str(word);
                line_w += ww;
                first = false;
            } else {
                let chars: Vec<char> = word.chars().collect();
                let mut i = 0;
                while i < chars.len() {
                    let end = (i + max_chars).min(chars.len());
                    if !first {
                        out.push('\n');
                    }
                    out.extend(chars[i..end].iter());
                    line_w = (end - i) as f32 * char_w;
                    first = false;
                    i = end;
                    if i < chars.len() {
                        out.push('\n');
                        first = true;
                    }
                }
            }
        }
    }
    out
}

// --- sublabel formatters ---

pub fn cutoff_sub(p: &params::Params) -> String {
    let hz = params::knob_0_10_to_cutoff(p.cutoff_0_10);
    if hz >= 1000.0 {
        format!("{:.1}kHz", hz / 1000.0)
    } else {
        format!("{:.0}Hz", hz)
    }
}

pub fn contour_sub(p: &params::Params) -> String {
    format!("{:.1} oct", (p.contour_amt_0_10 / 10.0) * 6.0)
}

pub fn glide_sub(p: &params::Params) -> String {
    let g = params::knob_0_10_to_glide(p.glide_0_10);
    if g < 0.0006 {
        "off".into()
    } else {
        format!("{}ms", (g * 1000.0) as i32)
    }
}

pub fn tune_sub(p: &params::Params) -> String {
    format!("{:+.2} st", params::knob_0_10_to_tune_semi(p.tune_0_10))
}

pub fn a_sub(k: f32) -> String {
    let s = params::knob_0_10_to_attack(k);
    if s < 1.0 {
        format!("{}ms", (s * 1000.0) as i32)
    } else {
        format!("{:.1}s", s)
    }
}

pub fn d_sub(k: f32) -> String {
    let s = params::knob_0_10_to_decay(k);
    if s < 1.0 {
        format!("{}ms", (s * 1000.0) as i32)
    } else {
        format!("{:.1}s", s)
    }
}

pub fn lfo_sub(p: &params::Params) -> String {
    let hz = params::knob_0_10_to_lfo_hz(p.lfo_rate_0_10);
    if hz < 1.0 {
        format!("{:.2}Hz", hz)
    } else {
        format!("{:.1}Hz", hz)
    }
}
