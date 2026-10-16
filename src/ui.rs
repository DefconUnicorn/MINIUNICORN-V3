//! MODEL D panel — white line-drawing theme matching the user's diagram.
//!
//! One tight row, left-to-right:
//! CONTROLLERS | OSCILLATOR BANK | MIXER | MODIFIERS | OUTPUT
//! plus KEYS (one octave) + 16-step SEQUENCER below.
//! OSC BANK is a 3x3 grid (RANGE left, FREQUENCY middle, WAVE right)
//! with OSC-1 FREQUENCY empty — the fixed reference.
//! Rockers are 2-state coloured toggles (all hardware rockers are 2-state;
//! keyboard tracking gets its 4 levels from two toggles combined).

pub mod devices;
pub mod jacks;
pub mod text;
pub mod widgets;

use crate::layout;
use crate::learn;
use crate::params::{self, Shared};
use crate::presets;
use egui::{Color32, Order, Pos2, Sense, Stroke, Vec2};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

pub use devices::*;
pub use jacks::*;
pub use text::*;
pub use widgets::*;

// ---------------------------------------------------------------------------
// v2 layout tool: every knob/toggle (+ attached text — each widget draws
// as one unit) floats in its own `Area` on one big white sheet. In edit
// mode a ⠿ handle drags the item; the position snaps live to the grid
// and is reported back via the return value so it can be persisted.
// NOTE: call sites must pass an explicit `|ui|` closure (macro hygiene:
// a bare `{...}` block would resolve `ui` to the outer panel instead of
// the area).
// ---------------------------------------------------------------------------

/// Show one free canvas item; returns `(new snapped position, dragging,
/// new wrap width)`.
///
/// In edit mode a transparent drag sheet covers the whole item (allocated
/// last = hit-tested first), so grabbing anywhere moves it and the
/// control underneath can't function. Plain click selects exclusively,
/// Ctrl+click toggles selection; drag moves the whole selection together.
/// Text items: Shift+drag resizes the wrap width instead of moving —
/// anchored at the natural width when wrapping is off, so the first grab
/// never explodes the text. Drops snap to the grid only. Item rects are
/// recorded for box-select and selection outlines.
fn canvas_item(
    ctx: &egui::Context,
    canvas: &layout::CanvasLayout,
    id: &str,
    handle: &str,
    edit: bool,
    selected: &mut HashSet<String>,
    rects: &mut HashMap<String, egui::Rect>,
    add: impl FnOnce(&mut egui::Ui),
) -> (Option<[f32; 2]>, bool, Option<f32>) {
    let grid = canvas.grid;
    let start = canvas.pos.get(id).copied().unwrap_or([10.0, 10.0]);
    let mut p = Pos2::new(start[0], start[1]);
    let mut moved = false;
    let mut dragging = false;
    let mut wrap_set: Option<f32> = None;
    // Hidden items vanish in play mode; in edit mode they stay as ghosts
    // so they can be shown again.
    if !edit && canvas.hidden.contains(id) {
        rects.remove(id);
        return (None, false, None);
    }
    let ghost = edit && canvas.hidden.contains(id);
    // Text items support Shift+drag width resize.
    let is_text = canvas.texts.contains_key(id);
    // Selected items float above the rest so neighbours can never cover
    // the item being arranged.
    let order = if selected.contains(id) {
        Order::Foreground
    } else {
        Order::Middle
    };
    egui::Area::new(egui::Id::new(format!("v2c_{id}")))
        .fixed_pos(p)
        .order(order)
        .show(ctx, |ui| {
            add(ui);
            rects.insert(id.to_string(), ui.min_rect());
            if edit {
                // Full-item drag sheet (no visible handle: the whole item
                // grabs; its name + hidden state show as a hover tooltip).
                let r = ui.min_rect();
                let resp = ui.allocate_rect(r, Sense::click_and_drag());
                let shift = ctx.input(|i| i.modifiers.shift);
                if resp.hovered() {
                    ctx.set_cursor_icon(if shift && is_text {
                        egui::CursorIcon::ResizeHorizontal
                    } else {
                        egui::CursorIcon::Grab
                    });
                }
                if resp.clicked() {
                    let ctrl = ctx.input(|i| i.modifiers.ctrl);
                    if ctrl {
                        if !selected.remove(id) {
                            selected.insert(id.to_string());
                        }
                    } else {
                        selected.clear();
                        selected.insert(id.to_string());
                    }
                }
                if resp.dragged() {
                    ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
                    dragging = true;
                    let ctrl = ctx.input(|i| i.modifiers.ctrl);
                    if shift && is_text {
                        // Shift+drag on text: set an absolute wrap width so
                        // resize tracks the pointer 1:1 with no accumulation
                        // or snap artefacts.
                        // Shrink (d.x<0) with wrap far wider than the visible
                        // text (e.g. 500 vs 53 natural): start from the
                        // visible width so the drag has immediate effect
                        // instead of needing a ~450px pull before anything
                        // reflows. Expand (d.x>=0) keeps `cur` so there is
                        // no initial dip backwards.
                        // Floor is 12px, never 0: 0 means "wrapping off" and
                        // slamming fully left used to catapult back to full
                        // natural width, which felt like expand was broken.
                        let d = resp.drag_delta();
                        let cur = canvas.texts.get(id).map(|s| s.wrap).unwrap_or(0.0);
                        let rect_w = rects.get(id).map(|r| r.width()).unwrap_or(120.0);
                        let base = if cur <= 0.0 {
                            rect_w
                        } else if resp.drag_started() && d.x < 0.0 && cur > rect_w + 1.0 {
                            rect_w
                        } else {
                            cur
                        };
                        wrap_set = Some((base + d.x).clamp(12.0, 600.0));
                    } else {
                        if !ctrl && !selected.contains(id) {
                            selected.clear();
                            selected.insert(id.to_string());
                        }
                        let d = resp.drag_delta();
                        // Grid-only snap (no neighbour snap).
                        p.x = layout::snap(p.x + d.x, grid);
                        p.y = layout::snap(p.y + d.y, grid).max(0.0);
                        moved = true;
                    }
                }
                let tip = if ghost {
                    format!("⠿ {handle} — hidden, drag to move")
                } else if is_text {
                    let w = canvas.texts.get(id).map(|s| s.wrap).unwrap_or(0.0);
                    format!("⠿ {handle} — drag to move • Shift+drag to stretch (wrap {w:.0}px)")
                } else {
                    format!("⠿ {handle} — drag to move")
                };
                let _ = resp.on_hover_text(tip);
            }
        });
    (
        if moved { Some([p.x, p.y]) } else { None },
        dragging,
        wrap_set,
    )
}

/// Place one canvas item, persisting moves and noting the dragged item
/// (for alignment guides). Pass an explicit closure so `ui` binds to
/// the area: `item!(self, ctx, edit, "id", "H", |ui| {...})`.
macro_rules! item {
    ($self:ident, $ctx:ident, $edit:ident, $id:expr, $handle:expr, $closure:expr) => {
        {
            let (mv, dg, wset) = canvas_item(
                $ctx,
                &$self.canvas,
                $id,
                $handle,
                $edit,
                &mut $self.selected,
                &mut $self.item_rects,
                $closure,
            );
            if dg {
                $self.dragging_id = Some($id.to_string());
                // A fresh canvas drag dismisses any open context menu so
                // it can never swallow the gesture.
                $self.menu_for = None;
            }
            // Shift+drag on text sets its wrap width (live reflow).
            if let Some(w) = wset {
                if let Some(t) = $self.canvas.texts.get_mut($id) {
                    t.wrap = w;
                    $self.layout_dirty = true;
                }
            }
            if let Some(pos) = mv {
                let old = $self.canvas.pos.get($id).copied().unwrap_or(pos);
                let (dx, dy) = (pos[0] - old[0], pos[1] - old[1]);
                $self.canvas.pos.insert($id.to_string(), pos);
                // Carry the rest of the selection along by the same delta.
                if (dx != 0.0 || dy != 0.0) && $self.selected.contains($id) {
                    let g = $self.canvas.grid;
                    let followers: Vec<String> =
                        $self.selected.iter().filter(|o| *o != $id).cloned().collect();
                    for oid in followers {
                        if let Some(op) = $self.canvas.pos.get(&oid).copied() {
                            $self.canvas.pos.insert(
                                oid,
                                [layout::snap(op[0] + dx, g), layout::snap(op[1] + dy, g).max(0.0)],
                            );
                        }
                    }
                }
                $self.layout_dirty = true;
            }
        }
    };
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

// Fixed manual base: C4 (MIDI 60), keys C4–C5. Octave shifting lives
// on the OSC RANGE selectors like the hardware — no separate octave
// switch. A key = A4, so A + RANGE 8' matches the A-440 reference.
const KEYS_BASE: i32 = 60;

pub struct ModelDApp {
    shared: Arc<Mutex<Shared>>,
    midi_status: String,
    midi_selection: Arc<Mutex<Option<String>>>,
    preset_name: String,
    preset_current: Option<String>,
    preset_list: Vec<String>,
    preset_msg: String,
    last_autosave: std::time::Instant,
    last_saved_json: String,
    // v2 free-canvas layout tool
    edit_layout: bool,
    canvas: layout::CanvasLayout,
    layout_name: String,
    layout_msg: String,
    audio_output_names: Vec<String>,
    audio_input_names: Vec<String>,
    midi_input_names: Vec<String>,
    last_layout_save: std::time::Instant,
    layout_dirty: bool,
    dragging_id: Option<String>,
    // v2 multi-select (session only, not persisted)
    selected: HashSet<String>,
    item_rects: HashMap<String, egui::Rect>,
    marquee_start: Option<Pos2>,
    // v2 right-click menu (session only): item id + click pos, live rect.
    menu_for: Option<(String, Pos2)>,
    menu_rect: Option<egui::Rect>,
    icon: Option<egui::TextureHandle>,
    // MIDI learn (session UI state; bindings live in `params.midi_cc`).
    learn_open: bool,
    learning: Option<u8>,
    learn_armed_seq: u32,
    learn_msg: String,
    // Cheat-sheet reference window (session only).
    cheat_open: bool,
    // Layout undo/redo (session only). Snapshots are whole CanvasLayouts.
    undo_stack: Vec<layout::CanvasLayout>,
    redo_stack: Vec<layout::CanvasLayout>,
    was_dragging: bool,
    skip_undo_commit: bool,
}

impl ModelDApp {
    pub fn new(
        shared: Arc<Mutex<Shared>>,
        midi_status: String,
        midi_selection: Arc<Mutex<Option<String>>>,
        icon: Option<egui::TextureHandle>,
    ) -> Self {
        // Snapshot the restored state so autosave only writes on change.
        let last_saved_json = shared
            .lock()
            .ok()
            .and_then(|s| serde_json::to_string(&s.params).ok())
            .unwrap_or_default();
        Self {
            shared,
            midi_status,
            midi_selection,
            preset_name: String::new(),
            preset_current: None,
            preset_list: presets::list_presets(),
            preset_msg: String::new(),
            last_autosave: std::time::Instant::now(),
            last_saved_json,
            edit_layout: false,
            canvas: layout::load(),
            layout_name: String::new(),
            layout_msg: String::new(),
            audio_output_names: computer_output_names(),
            audio_input_names: computer_input_names(),
            midi_input_names: midi_input_names(),
            last_layout_save: std::time::Instant::now(),
            layout_dirty: false,
            dragging_id: None,
            selected: HashSet::new(),
            item_rects: HashMap::new(),
            marquee_start: None,
            menu_for: None,
            menu_rect: None,
            icon,
            learn_open: false,
            learning: None,
            learn_armed_seq: 0,
            learn_msg: String::new(),
            cheat_open: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            was_dragging: false,
            skip_undo_commit: false,
        }
    }

    fn play_note(shared: &Arc<Mutex<Shared>>, note: u8, vel: f32) {
        if let Ok(mut s) = shared.lock() {
            s.runtime.note_on(note, vel);
        }
    }

    fn stop_note(shared: &Arc<Mutex<Shared>>, note: u8) {
        if let Ok(mut s) = shared.lock() {
            s.runtime.note_off(note);
        }
    }

    /// Record a layout snapshot for undo (bounded history), clearing redo.
    fn push_undo(&mut self, snapshot: layout::CanvasLayout) {
        const MAX: usize = 100;
        if self.undo_stack.len() >= MAX {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(snapshot);
        self.redo_stack.clear();
    }

    fn undo_layout(&mut self) {
        if let Some(prev) = self.undo_stack.pop() {
            let cur = std::mem::replace(&mut self.canvas, prev);
            self.redo_stack.push(cur);
            self.layout_dirty = true;
            self.skip_undo_commit = true;
        }
    }

    fn redo_layout(&mut self) {
        if let Some(next) = self.redo_stack.pop() {
            let cur = std::mem::replace(&mut self.canvas, next);
            self.undo_stack.push(cur);
            self.layout_dirty = true;
            self.skip_undo_commit = true;
        }
    }
}

impl eframe::App for ModelDApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut visuals = egui::Visuals::light();
        visuals.panel_fill = PAPER;
        visuals.override_text_color = Some(INK);
        ctx.set_visuals(visuals);

        // Layout undo/redo: Ctrl+Z / Ctrl+Y (or Ctrl+Shift+Z), only while
        // arranging so the shortcuts never interfere with play mode.
        if self.edit_layout && !ctx.wants_keyboard_input() {
            let (undo, redo) = ctx.input_mut(|i| {
                let undo = i.consume_key(egui::Modifiers::COMMAND, egui::Key::Z);
                let redo = i.consume_key(egui::Modifiers::COMMAND, egui::Key::Y)
                    || i.consume_key(
                        egui::Modifiers::COMMAND.plus(egui::Modifiers::SHIFT),
                        egui::Key::Z,
                    );
                (undo, redo)
            });
            if undo {
                self.undo_layout();
            }
            if redo {
                self.redo_layout();
            }
        }
        // Snapshot taken after undo/redo so this frame's commit logic never
        // re-records the state we just restored.
        let frame_start_canvas = self.canvas.clone();

        let key_map: Vec<(egui::Key, i32)> = vec![
            (egui::Key::A, 0),
            (egui::Key::W, 1),
            (egui::Key::S, 2),
            (egui::Key::E, 3),
            (egui::Key::D, 4),
            (egui::Key::F, 5),
            (egui::Key::T, 6),
            (egui::Key::G, 7),
            (egui::Key::Y, 8),
            (egui::Key::H, 9),
            (egui::Key::U, 10),
            (egui::Key::J, 11),
            (egui::Key::K, 12),
        ];
        ctx.input(|i| {
            // Fixed one-octave manual (C3=48 base); octave shifting lives
            // on the OSC RANGE selectors like the hardware.
            for (key, offset) in &key_map {
                if i.key_pressed(*key) {
                    let note = (KEYS_BASE + offset).clamp(0, 127) as u8;
                    Self::play_note(&self.shared, note, 0.9);
                }
                if i.key_released(*key) {
                    let note = (KEYS_BASE + offset).clamp(0, 127) as u8;
                    Self::stop_note(&self.shared, note);
                }
            }
        });

        let mut p = self.shared.lock().map(|s| s.params).unwrap_or_default();
        let (seq_step, held, held_len) = self
            .shared
            .lock()
            .map(|s| (s.runtime.seq_step, s.runtime.held, s.runtime.held_len))
            .unwrap_or((0, [0u8; 16], 0));

        // --- MIDI learn: drain CC-driven parameter updates, arm/engage ---
        let cur_cc_seq;
        {
            let mut cc_seq = 0u32;
            let mut last_cc = 0u8;
            if let Ok(mut s) = self.shared.lock() {
                let len = s.runtime.cc_len as usize;
                for i in 0..len.min(s.runtime.cc_events.len()) {
                    let ev = s.runtime.cc_events[i];
                    learn::set(&mut p, ev.param, ev.value);
                }
                s.runtime.cc_len = 0;
                cc_seq = s.runtime.cc_seq;
                last_cc = s.runtime.last_cc;
            }
            cur_cc_seq = cc_seq;
            // A fresh control move while armed completes the binding.
            if let Some(param) = self.learning {
                if cc_seq != self.learn_armed_seq {
                    if let Some(id) = learn::P::from_id(param) {
                        if learn::bind(&mut p, last_cc, param) {
                            self.learn_msg = format!("Bound {} to CC {}.", id.name(), last_cc);
                        } else {
                            self.learn_msg = "No free slots — clear one first.".to_string();
                        }
                    }
                    self.learning = None;
                }
            }
        }

        egui::TopBottomPanel::top("topbar")
            .frame(egui::Frame::side_top_panel(&ctx.style()).inner_margin(egui::Margin::symmetric(10.0, 0.0)))
            .show(ctx, |ui| {
            // The banner is painted as a background pinned to the right edge at
            // its natural size, so nothing laid out over it can shrink it. The
            // bar keeps a fixed height so it never jumps between modes.
            const BANNER_H: f32 = 118.0;
            ui.set_min_height(BANNER_H);
            ui.set_max_height(BANNER_H);
            if let Some(icon) = &self.icon {
                let [iw, ih] = icon.size();
                let aspect = iw as f32 / ih.max(1) as f32;
                let w = (BANNER_H * aspect).min(ui.available_width());
                let full = ui.max_rect();
                let rect = egui::Rect::from_min_max(
                    egui::pos2(full.right() - w, full.top()),
                    egui::pos2(full.right(), full.top() + BANNER_H),
                );
                ui.painter().image(
                    icon.id(),
                    rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            }
            ui.horizontal(|ui| {
                toggle(ui, "EDIT LAYOUT", &mut self.edit_layout, BLUE_ON).on_hover_text(
                    "Grab any control to move it • Shift+drag text stretches it • Ctrl+click toggles • box-drag empty space • right-click: hide / rotate / lines+text • arrows nudge (Shift = ×5) • knobs stay put while editing.",
                );
                toggle(ui, "MIDI LEARN", &mut self.learn_open, RED_ON).on_hover_text(
                    "Open the MIDI-learn panel: bind any control to a MIDI CC, then move that knob from your controller.",
                );
                toggle(ui, "CHEAT SHEET", &mut self.cheat_open, BLACK_ON).on_hover_text(
                    "Open a reference describing what every knob, toggle, selector and jack does.",
                );
                ui.separator();
                ui.label(egui::RichText::new("VOICES").color(SOFT).size(11.0));
                let mut voices_pick = p.voice_count.max(1);
                egui::ComboBox::from_id_source("voices_pick")
                    .selected_text(format!("{voices_pick}"))
                    .width(44.0)
                    .show_ui(ui, |ui| {
                        for v in [1u8, 2, 3, 4, 6, 8] {
                            let label = if v == 1 { "1 (mono)".to_string() } else { format!("{v}") };
                            ui.selectable_value(&mut voices_pick, v, label);
                        }
                    })
                    .response
                    .on_hover_text(
                        "Number of sounding voices. 1 = monophonic Model D. Higher values let held notes overlap (paraphonic/polyphonic).",
                    );
                if voices_pick != p.voice_count {
                    p.voice_count = voices_pick;
                }
                ui.add_enabled_ui(p.voice_count > 1, |ui| {
                    ui.label(egui::RichText::new("LEVEL").color(SOFT).size(11.0));
                    let mode_names = ["NATURAL", "GENTLE", "CONSTANT"];
                    let mut mode_pick = p.poly_level_mode.min(2);
                    egui::ComboBox::from_id_source("poly_level_pick")
                        .selected_text(mode_names[mode_pick as usize])
                        .width(84.0)
                        .show_ui(ui, |ui| {
                            for (i, name) in mode_names.iter().enumerate() {
                                ui.selectable_value(&mut mode_pick, i as u8, *name);
                            }
                        })
                        .response
                        .on_hover_text(
                            "How loudness responds to playing more notes: NATURAL = chords get louder (no dip); GENTLE = small dip, mild growth; CONSTANT = fixed overall level (each added note lowers the others).",
                        );
                    if mode_pick != p.poly_level_mode {
                        p.poly_level_mode = mode_pick;
                    }
                });
            });
            if self.edit_layout {
                ui.horizontal(|ui| {
                ui.separator();
                ui.add_enabled_ui(!self.undo_stack.is_empty(), |ui| {
                    if ui.button("↶ Undo").on_hover_text("Ctrl+Z").clicked() {
                        self.undo_layout();
                    }
                });
                ui.add_enabled_ui(!self.redo_stack.is_empty(), |ui| {
                    if ui.button("↷ Redo").on_hover_text("Ctrl+Y / Ctrl+Shift+Z").clicked() {
                        self.redo_layout();
                    }
                });
                ui.label(egui::RichText::new("GRID").color(SOFT).size(11.0));
                let mut grid_pick = format!("{}", self.canvas.grid as i32);
                egui::ComboBox::from_id_source("grid_pick")
                    .selected_text(grid_pick.clone())
                    .width(52.0)
                    .show_ui(ui, |ui| {
                        for g in ["4", "8", "16"] {
                            ui.selectable_value(&mut grid_pick, g.to_string(), g);
                        }
                    });
                if let Ok(g) = grid_pick.parse::<f32>() {
                    if (g - self.canvas.grid).abs() > 0.01 {
                        self.canvas.grid = g.clamp(1.0, 64.0);
                        self.layout_dirty = true;
                    }
                }
                if ui.button("RESET").clicked() {
                    // Factory flow again, but keep the user's own lines
                    // and texts.
                    let keep_lines = self.canvas.lines.clone();
                    let keep_seq = self.canvas.line_seq;
                    let keep_texts = self.canvas.texts.clone();
                    let keep_tseq = self.canvas.text_seq;
                    self.canvas = layout::CanvasLayout::default();
                    self.canvas.lines = keep_lines;
                    self.canvas.line_seq = keep_seq;
                    self.canvas.texts = keep_texts;
                    self.canvas.text_seq = keep_tseq;
                    for lid in self.canvas.lines.keys() {
                        if !self.canvas.pos.contains_key(lid) {
                            self.canvas.pos.insert(lid.clone(), [800.0, 420.0]);
                        }
                    }
                    for tid in self.canvas.texts.keys() {
                        if !self.canvas.pos.contains_key(tid) {
                            self.canvas.pos.insert(tid.clone(), [800.0, 480.0]);
                        }
                    }
                    self.layout_dirty = true;
                }
                ui.label(egui::RichText::new("NAME").color(SOFT).size(11.0));
                ui.add(
                    egui::TextEdit::singleline(&mut self.layout_name)
                        .desired_width(110.0)
                        .hint_text("layout name"),
                );
                if ui.button("SAVE LAYOUT").clicked() {
                    if layout::save_named(&self.layout_name, &self.canvas) {
                        layout::save(&self.canvas);
                        self.layout_dirty = false;
                        self.last_layout_save = std::time::Instant::now();
                        self.layout_msg = format!("Saved '{}'.", self.layout_name.trim());
                    } else {
                        self.layout_msg = "Enter a layout name first.".to_string();
                    }
                }
                let saved_names = layout::named_names();
                egui::ComboBox::from_id_source("saved_layouts")
                    .selected_text(if self.layout_name.trim().is_empty() {
                        "saved layouts"
                    } else {
                        self.layout_name.as_str()
                    })
                    .width(110.0)
                    .show_ui(ui, |ui| {
                        for name in &saved_names {
                            ui.selectable_value(&mut self.layout_name, name.clone(), name);
                        }
                    });
                if ui.button("LOAD").clicked() {
                    if let Some(saved) = layout::load_named(&self.layout_name) {
                        self.canvas = saved;
                        layout::save(&self.canvas);
                        self.layout_dirty = false;
                        self.selected.clear();
                        self.menu_for = None;
                        self.layout_msg = format!("Loaded '{}'.", self.layout_name.trim());
                    } else {
                        self.layout_msg = "Choose a saved layout first.".to_string();
                    }
                }
                if !self.layout_msg.is_empty() {
                    ui.label(egui::RichText::new(&self.layout_msg).color(SOFT).size(10.0));
                }
                if ui.button("ADD LINE").clicked() {
                    let n = self.canvas.line_seq + 1;
                    self.canvas.line_seq = n;
                    let lid = format!("line{n}");
                    self.canvas.pos.insert(
                        lid.clone(),
                        [800.0 + ((n as f32 * 37.0) % 400.0), 420.0],
                    );
                    self.canvas.lines.insert(
                        lid.clone(),
                        layout::LineSpec {
                            name: format!("Line {n}"),
                            vertical: true,
                            len: 120.0,
                            thick: 3.0,
                        },
                    );
                    self.selected.clear();
                    self.selected.insert(lid);
                    self.layout_dirty = true;
                }
                if ui.button("ADD TEXT").clicked() {
                    let n = self.canvas.text_seq + 1;
                    self.canvas.text_seq = n;
                    let tid = format!("text{n}");
                    self.canvas.pos.insert(
                        tid.clone(),
                        [800.0 + ((n as f32 * 53.0) % 400.0), 480.0],
                    );
                    self.canvas.texts.insert(
                        tid.clone(),
                        layout::TextSpec {
                            content: format!("Text {n}"),
                            size: 13.0,
                            wrap: 0.0,
                        },
                    );
                    self.selected.clear();
                    self.selected.insert(tid);
                    self.layout_dirty = true;
                }
                if !self.canvas.hidden.is_empty() {
                    let n = self.canvas.hidden.len();
                    if ui.button(format!("SHOW ALL ({n})")).clicked() {
                        self.canvas.hidden.clear();
                        self.layout_dirty = true;
                    }
                }
                });
                }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // v2: one large white sheet; every knob/toggle (+ attached
            // text) floats on it as its own grid-snapped draggable item.
            // EDIT LAYOUT shows the ⠿ handles; drops persist.
            let sheet = ui.max_rect().shrink(6.0);
            ui.painter().rect(sheet, 8.0, Color32::WHITE, Stroke::new(1.5_f32, INK));
            let edit = self.edit_layout;
            self.dragging_id = None;
            // v2 per-item rotation (right-click menu): rockers consult this.
            let rot90 = self.canvas.rot90.clone();
            // v2 arrow-key nudge: selection steps one grid cell per press
            // (Shift = 5 cells). Skipped while typing (preset name, menu
            // number fields) so arrows keep working there.
            if edit && !self.selected.is_empty() && !ctx.wants_keyboard_input() {
                let g = self.canvas.grid;
                let step =
                    g * if ctx.input(|i| i.modifiers.shift) { 5.0 } else { 1.0 };
                let mut dx = 0.0;
                let mut dy = 0.0;
                ctx.input(|i| {
                    if i.key_pressed(egui::Key::ArrowLeft) {
                        dx -= step;
                    }
                    if i.key_pressed(egui::Key::ArrowRight) {
                        dx += step;
                    }
                    if i.key_pressed(egui::Key::ArrowUp) {
                        dy -= step;
                    }
                    if i.key_pressed(egui::Key::ArrowDown) {
                        dy += step;
                    }
                });
                if dx != 0.0 || dy != 0.0 {
                    for oid in self.selected.clone().iter() {
                        if let Some(op) = self.canvas.pos.get(oid).copied() {
                            self.canvas.pos.insert(
                                oid.clone(),
                                [(op[0] + dx).max(0.0), (op[1] + dy).max(0.0)],
                            );
                        }
                    }
                    self.layout_dirty = true;
                }
            }
            // v2 marquee: box-select on empty sheet. Item Areas are above,
            // so grabs starting on controls win; only empty-space gestures
            // reach this response.
            let marquee = ui.allocate_rect(sheet, Sense::click_and_drag());
            if edit {
                if marquee.drag_started() {
                    self.marquee_start = marquee.interact_pointer_pos();
                }
                if marquee.drag_stopped() {
                    if let (Some(a), Some(b)) = (self.marquee_start, marquee.interact_pointer_pos()) {
                        let r = egui::Rect::from_two_pos(a, b);
                        let ctrl = ctx.input(|i| i.modifiers.ctrl);
                        let hits: Vec<String> = self
                            .item_rects
                            .iter()
                            .filter(|(_, ir)| r.intersects(**ir))
                            .map(|(k, _)| k.clone())
                            .collect();
                        if ctrl {
                            for h in hits {
                                if !self.selected.remove(&h) {
                                    self.selected.insert(h);
                                }
                            }
                        } else {
                            self.selected = hits.into_iter().collect();
                        }
                    }
                    self.marquee_start = None;
                }
                if marquee.clicked() && !ctx.input(|i| i.modifiers.ctrl) {
                    self.selected.clear();
                }
                // v2 right-click menu: smallest containing item wins
                // (topmost heuristic on overlaps); right-click selects it.
                if ctx.input(|i| i.pointer.secondary_clicked()) {
                    if let Some(pos) = ctx.pointer_interact_pos() {
                        let mut best: Option<(String, f32)> = None;
                        for (k, r) in self.item_rects.iter() {
                            if r.contains(pos) {
                                let area = r.width() * r.height();
                                if best.as_ref().map_or(true, |(_, a)| area < *a) {
                                    best = Some((k.clone(), area));
                                }
                            }
                        }
                        if let Some((k, _)) = best {
                            if !ctx.input(|i| i.modifiers.ctrl) {
                                self.selected.clear();
                            }
                            self.selected.insert(k.clone());
                            self.menu_for = Some((k, pos));
                        } else {
                            self.menu_for = None;
                        }
                    }
                }
                // Primary press outside the open menu dismisses it.
                if self.menu_for.is_some() && ctx.input(|i| i.pointer.primary_pressed()) {
                    if let Some(mp) = ctx.pointer_interact_pos() {
                        if self.menu_rect.map_or(true, |r| !r.contains(mp)) {
                            self.menu_for = None;
                        }
                    }
                }
            } else {
                self.marquee_start = None;
                self.selected.clear();
                let jack_menu_open = self.menu_for.as_ref().map_or(false, |(id, _)| {
                    self.canvas
                        .jacks
                        .get(id)
                        .map_or(false, |spec| spec.is_input || spec.label == "MAIN")
                });
                if !jack_menu_open {
                    self.menu_for = None;
                }
            }
            if !edit && self.menu_for.is_some() && ctx.input(|i| i.pointer.primary_pressed()) {
                if let Some(pos) = ctx.pointer_interact_pos() {
                    if self.menu_rect.map_or(true, |rect| !rect.contains(pos)) {
                        self.menu_for = None;
                    }
                }
            }
            // ===== CONTROLLERS (v2: OSC MODULATION is a regular rocker
            // here instead of straddling a divider) =====
            item!(self, ctx, edit, "c_tune", "TUNE", |ui| {
                knob(ui, "TUNE", tune_sub(&p), &mut p.tune_0_10, 44.0);
            });
            item!(self, ctx, edit, "c_glide", "GLIDE", |ui| {
                knob(ui, "GLIDE", glide_sub(&p), &mut p.glide_0_10, 36.0);
            });
            item!(self, ctx, edit, "c_modmix", "MOD MIX", |ui| {
                knob(ui, "MOD MIX", "A<>B", &mut p.mod_mix_0_10, 36.0);
            });
            item!(self, ctx, edit, "c_aba", "OSC 3 / FILTER EG", |ui| {
                ab_r(ui, "OSC 3", "FILTER EG", &mut p.mod_src_a_is_filter_eg, BLACK_ON, rot90.contains("c_aba"));
            });
            item!(self, ctx, edit, "c_abb", "NOISE / LFO", |ui| {
                ab_r(ui, "NOISE", "LFO", &mut p.mod_src_b_is_lfo, BLACK_ON, rot90.contains("c_abb"));
            });
            item!(self, ctx, edit, "c_moddepth", "MOD DEPTH", |ui| {
                knob(ui, "MOD DEPTH", "vibrato", &mut p.mod_depth_0_10, 36.0);
            });
            item!(self, ctx, edit, "c_lfoshape", "TRI / SQUARE", |ui| {
                ab_r(ui, "TRIANGLE", "SQUARE", &mut p.lfo_square, BLACK_ON, !rot90.contains("c_lfoshape"));
            });
            item!(self, ctx, edit, "c_lforate", "LFO RATE", |ui| {
                knob(ui, "LFO RATE", lfo_sub(&p), &mut p.lfo_rate_0_10, 36.0);
            });
            item!(self, ctx, edit, "c_oscmod", "OSC MODULATION", |ui| {
                toggle_r(ui, "OSCILLATOR MODULATION", &mut p.osc_mod_on, BLUE_ON, PAPER, rot90.contains("c_oscmod"));
            });
            // (v2 items above replace the CONTROLLERS frame)

            // ===== OSCILLATOR BANK (RANGE / FREQUENCY / WAVE per osc;
            // OSC-1 is the fixed reference; CONTROL affects OSC-3) =====
            item!(self, ctx, edit, "o_name1", "OSC-1 tag", |ui| {
                ui.label(egui::RichText::new("OSC-1").color(INK).strong().size(11.0));
            });
            item!(self, ctx, edit, "o_range1", "OSC-1 RANGE", |ui| {
                range_rotary(ui, &mut p.osc1_range_oct);
            });
            item!(self, ctx, edit, "o_wave1", "OSC-1 WAVE", |ui| {
                wave_rotary(ui, 0, &mut p.osc1_wave);
            });
            item!(self, ctx, edit, "o_name2", "OSC-2 tag", |ui| {
                ui.label(egui::RichText::new("OSC-2").color(INK).strong().size(11.0));
            });
            item!(self, ctx, edit, "o_range2", "OSC-2 RANGE", |ui| {
                range_rotary(ui, &mut p.osc2_range_oct);
            });
            item!(self, ctx, edit, "o_freq2", "OSC-2 FREQUENCY", |ui| {
                let sub = format!("{:+.1} st", params::knob_0_10_to_fine_semi(p.osc2_freq_0_10));
                knob(ui, "FREQUENCY", &sub, &mut p.osc2_freq_0_10, 44.0);
            });
            item!(self, ctx, edit, "o_wave2", "OSC-2 WAVE", |ui| {
                wave_rotary(ui, 1, &mut p.osc2_wave);
            });
            item!(self, ctx, edit, "o_name3", "OSC-3 tag", |ui| {
                ui.label(egui::RichText::new("OSC-3").color(INK).strong().size(11.0));
            });
            item!(self, ctx, edit, "o_range3", "OSC-3 RANGE", |ui| {
                range_rotary(ui, &mut p.osc3_range_oct);
            });
            item!(self, ctx, edit, "o_freq3", "OSC-3 FREQUENCY", |ui| {
                let sub = format!("{:+.1} st", params::knob_0_10_to_fine_semi(p.osc3_freq_0_10));
                knob(ui, "FREQUENCY", &sub, &mut p.osc3_freq_0_10, 44.0);
            });
            item!(self, ctx, edit, "o_wave3", "OSC-3 WAVE", |ui| {
                wave_rotary(ui, 2, &mut p.osc3_wave);
            });
            item!(self, ctx, edit, "o_ctrl3", "OSC 3 CONTROL", |ui| {
                toggle_r(ui, "OSC 3 CONTROL", &mut p.osc3_kb_ctrl, BLUE_ON, PAPER, rot90.contains("o_ctrl3"));
            });

            // ===== MIXER =====
            item!(self, ctx, edit, "m_vol1", "OSC 1 VOL", |ui| {
                knob(ui, "OSC 1 VOL", "", &mut p.osc1_level_0_10, 36.0);
            });
            item!(self, ctx, edit, "m_sw1", "OSC 1 switch", |ui| {
                toggle_r(ui, "OSC 1 VOL", &mut p.osc1_on, RED_ON, PAPER, rot90.contains("m_sw1"));
            });
            item!(self, ctx, edit, "m_vol2", "OSC 2 VOL", |ui| {
                knob(ui, "OSC 2 VOL", "", &mut p.osc2_level_0_10, 36.0);
            });
            item!(self, ctx, edit, "m_sw2", "OSC 2 switch", |ui| {
                toggle_r(ui, "OSC 2 VOL", &mut p.osc2_on, RED_ON, PAPER, rot90.contains("m_sw2"));
            });
            item!(self, ctx, edit, "m_vol3", "OSC 3 VOL", |ui| {
                knob(ui, "OSC 3 VOL", "", &mut p.osc3_level_0_10, 36.0);
            });
            item!(self, ctx, edit, "m_sw3", "OSC 3 switch", |ui| {
                toggle_r(ui, "OSC 3 VOL", &mut p.osc3_on, RED_ON, PAPER, rot90.contains("m_sw3"));
            });
            item!(self, ctx, edit, "m_swext", "EXT IN switch", |ui| {
                toggle_r(ui, "EXT IN", &mut p.ext_on, RED_ON, PAPER, rot90.contains("m_swext"));
            });
            item!(self, ctx, edit, "m_ext", "EXT IN VOLUME", |ui| {
                knob(ui, "EXT IN VOLUME", "", &mut p.ext_level_0_10, 36.0);
            });
            item!(self, ctx, edit, "m_swnoise", "NOISE switch", |ui| {
                toggle_r(ui, "NOISE", &mut p.noise_on, RED_ON, PAPER, rot90.contains("m_swnoise"));
            });
            item!(self, ctx, edit, "m_noise", "NOISE VOLUME", |ui| {
                knob(ui, "NOISE VOLUME", "", &mut p.noise_level_0_10, 36.0);
            });
            item!(self, ctx, edit, "m_wpink", "WHITE / PINK", |ui| {
                ab_r(ui, "WHITE", "PINK", &mut p.noise_pink, RED_ON, rot90.contains("m_wpink"));
            });

            // ===== MODIFIERS =====
            item!(self, ctx, edit, "f_cutoff", "CUTOFF", |ui| {
                knob(ui, "CUTOFF", cutoff_sub(&p), &mut p.cutoff_0_10, 36.0);
            });
            item!(self, ctx, edit, "f_emph", "EMPHASIS", |ui| {
                knob(ui, "EMPHASIS", "reso", &mut p.reso_0_10, 36.0);
            });
            item!(self, ctx, edit, "f_contour", "CONTOUR", |ui| {
                knob(ui, "CONTOUR", contour_sub(&p), &mut p.contour_amt_0_10, 36.0);
            });
            item!(self, ctx, edit, "f_fa", "FILTER ATTACK", |ui| {
                knob(ui, "ATTACK", a_sub(p.filt_attack_0_10), &mut p.filt_attack_0_10, 36.0);
            });
            item!(self, ctx, edit, "f_fd", "FILTER DECAY knob", |ui| {
                knob(ui, "DECAY", d_sub(p.filt_decay_0_10), &mut p.filt_decay_0_10, 36.0);
            });
            item!(self, ctx, edit, "f_fs", "FILTER SUSTAIN", |ui| {
                knob(ui, "SUSTAIN", "", &mut p.filt_sustain_0_10, 36.0);
            });
            item!(self, ctx, edit, "f_aa", "LOUD ATTACK", |ui| {
                knob(ui, "ATTACK", a_sub(p.amp_attack_0_10), &mut p.amp_attack_0_10, 36.0);
            });
            item!(self, ctx, edit, "f_ad", "LOUD DECAY knob", |ui| {
                knob(ui, "DECAY", d_sub(p.amp_decay_0_10), &mut p.amp_decay_0_10, 36.0);
            });
            item!(self, ctx, edit, "f_as", "LOUD SUSTAIN", |ui| {
                knob(ui, "SUSTAIN", "", &mut p.amp_sustain_0_10, 36.0);
            });

            // ===== OUTPUT =====
            item!(self, ctx, edit, "u_vol", "VOLUME", |ui| {
                knob(ui, "VOLUME", "", &mut p.volume_0_10, 38.0);
            });
            item!(self, ctx, edit, "u_main", "MAIN OUT", |ui| {
                toggle_r(ui, "MAIN OUT", &mut p.main_on, RED_ON, PAPER, rot90.contains("u_main"));
            });
            item!(self, ctx, edit, "u_a440", "A-440", |ui| {
                toggle_r(ui, "A-440", &mut p.a440_on, RED_ON, PAPER, rot90.contains("u_a440"));
            });
            item!(self, ctx, edit, "u_phones", "PHONES", |ui| {
                knob(ui, "VOLUME", "phones", &mut p.phones_0_10, 32.0);
                ui.label(egui::RichText::new("PHONES").color(SOFT).size(10.0));
            });
            // ===== MOD SWITCHES (dockable strip) =====
            item!(self, ctx, edit, "s_fmod", "FILTER MOD", |ui| {
                toggle_r(ui, "FILTER MOD", &mut p.filter_mod_on, BLUE_ON, PAPER, rot90.contains("s_fmod"));
            });
            item!(self, ctx, edit, "s_mode", "FILTER LO/HI", |ui| {
                ab_r(ui, "FILTER LO", "FILTER HI", &mut p.filter_mode_hi, BLUE_ON, rot90.contains("s_mode"));
            });
            item!(self, ctx, edit, "s_key1", "KEY 1", |ui| {
                toggle_r(ui, "KEY 1 (1/3)", &mut p.kb_control_1, BLUE_ON, PAPER, rot90.contains("s_key1"));
            });
            item!(self, ctx, edit, "s_key2", "KEY 2", |ui| {
                toggle_r(ui, "KEY 2 (2/3)", &mut p.kb_control_2, BLUE_ON, PAPER, rot90.contains("s_key2"));
            });
            item!(self, ctx, edit, "s_fdec", "FILTER DECAY sw", |ui| {
                toggle_r(ui, "FILTER DECAY", &mut p.filter_decay_on, PAPER, BLACK_ON, rot90.contains("s_fdec"));
            });
            item!(self, ctx, edit, "s_ldec", "LOUD DECAY sw", |ui| {
                toggle_r(ui, "LOUD DECAY", &mut p.loud_decay_on, PAPER, BLACK_ON, rot90.contains("s_ldec"));
            });
            // (v2: overlaps replaced by free items above — no row, no
            // dividers to straddle)

            // v2 divider lines (user-added via ADD LINE): draggable,
            // stretchable, right-click to edit / duplicate / delete.
            let line_ids: Vec<String> = {
                let mut v: Vec<String> = self.canvas.lines.keys().cloned().collect();
                v.sort();
                v
            };
            for lid in line_ids {
                let spec = self.canvas.lines.get(&lid).cloned().unwrap_or_default();
                let hname = spec.name.clone();
                item!(self, ctx, edit, lid.as_str(), hname.as_str(), |ui| {
                    let (w, h) = if spec.vertical {
                        (spec.thick, spec.len)
                    } else {
                        (spec.len, spec.thick)
                    };
                    let (rect, _) =
                        ui.allocate_exact_size(Vec2::new(w.max(1.0), h.max(1.0)), Sense::hover());
                    ui.painter_at(rect).rect_filled(rect, 0.0, INK);
                });
            }
            // v2 text labels (user-added via ADD TEXT): draggable, wording
            // + fontsize via right-click.
            let text_ids: Vec<String> = {
                let mut v: Vec<String> = self.canvas.texts.keys().cloned().collect();
                v.sort();
                v
            };
            for tid in text_ids {
                let spec = self.canvas.texts.get(&tid).cloned().unwrap_or_default();
                // Handle + menu preview use the first line only.
                let hname: String = if spec.content.trim().is_empty() {
                    "(text)".to_string()
                } else {
                    spec.content.lines().next().unwrap_or("(text)").chars().take(16).collect()
                };
                item!(self, ctx, edit, tid.as_str(), hname.as_str(), |ui| {
                    // `.extend()`: never soft-wrap/truncate — our reflow
                    // already inserted the line breaks, and egui's own wrap
                    // at the stale Area width used to freeze the display
                    // (expand looked dead even as `wrap` grew).
                    if spec.content.trim().is_empty() {
                        ui.label(
                            egui::RichText::new("(text)").color(SOFT).size(spec.size),
                        );
                    } else if spec.wrap > 0.0 {
                        let body = reflow_text(&spec.content, spec.size, spec.wrap);
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(body)
                                    .color(INK)
                                    .size(spec.size),
                            )
                            .extend(),
                        );
                    } else {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(spec.content.as_str())
                                    .color(INK)
                                    .size(spec.size),
                            )
                            .extend(),
                        );
                    }
                });
            }

            // v2 patch-bay jacks: draggable, right-click input to route.
            {
                let output_colors: HashMap<String, Color32> = self
                    .canvas
                    .jacks
                    .iter()
                    .filter(|(_, spec)| !spec.is_input)
                    .map(|(jid, _)| (jid.clone(), output_color(&self.canvas, jid)))
                    .collect();
                let mut jack_ids: Vec<String> = self.canvas.jacks.keys().cloned().collect();
                jack_ids.sort();
                let time = ctx.input(|i| i.time);
                for jid in jack_ids {
                    let spec = self.canvas.jacks.get(&jid).cloned().unwrap_or_default();
                    let hname = spec.label.clone();
                    let is_in = spec.is_input;
                    item!(self, ctx, edit, jid.as_str(), hname.as_str(), |ui| {
                        render_jack(ui, &jid, &spec, &output_colors, time);
                    });
                    if is_in || spec.label == "MAIN" {
                        if let Some(resp) = ui.ctx().input(|i| i.pointer.hover_pos()) {
                            let rect = self.item_rects.get(&jid).cloned().unwrap_or(egui::Rect::NOTHING);
                            if rect.contains(resp) && ui.ctx().input(|i| i.pointer.secondary_clicked()) {
                                self.menu_for = Some((jid, resp));
                            }
                        }
                    }
                }
            }

            // ---- KEYS group (one octave, OSC RANGE selects the octave) ----
            // Local Arc: the closure must not borrow `self` (multi-select
            // takes `&mut` fields alongside it).
            let shared_keys = self.shared.clone();
            item!(self, ctx, edit, "keys", "KEYS", |ui| {
                let mut clicked: Option<u8> = None;
                let mut unclick: Option<u8> = None;
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("A W S E D F T G Y H U J K plays").color(SOFT));
                    for semi in 0..13 {
                        let n = (KEYS_BASE + semi).clamp(0, 127) as u8;
                        let black = matches!(semi % 12, 1 | 3 | 6 | 8 | 10);
                        // Highlight every key that is currently held, not just
                        // the most recent one (last-note priority).
                        let active = !p.seq_on
                            && (0..held_len as usize).any(|i| held[i] == n);
                        // Show the actual MIDI note name, including octave.
                        let full = params::note_name(n);
                        let b = egui::Button::new(egui::RichText::new(full).color(if black { Color32::WHITE } else { INK }).size(16.0))
                            .min_size(Vec2::new(44.0, 52.0))
                            .fill(if active { HIGHLIGHT } else if black { BLACK_ON } else { PAPER })
                            .stroke(Stroke::new(1.5_f32, INK));
                        if ui.add(b).clicked() {
                            // Click toggles: clicking the sounding key
                            // releases it (note-off); clicking another key
                            // retriggers.
                            if active {
                                unclick = Some(n);
                            } else {
                                clicked = Some(n);
                            }
                        }
                    }
                    if ui.button("panic").clicked() {
                        if let Ok(mut s) = shared_keys.lock() { s.runtime.all_notes_off(); }
                    }
                });
                if let Some(n) = clicked { Self::play_note(&shared_keys, n, 0.9); }
                if let Some(n) = unclick { Self::stop_note(&shared_keys, n); }
            });

            // ---- SEQUENCER group: 16 steps ----
            // Loading a preset may carry a layout; apply it after the item
            // is drawn (the closure cannot mutate `self.canvas` while
            // `canvas_item` holds a shared borrow of it).
            let mut pending_layout: Option<layout::CanvasLayout> = None;
            item!(self, ctx, edit, "seq", "SEQUENCER", |ui| {
                ui.horizontal(|ui| {
                    toggle(ui, "SEQ PLAY", &mut p.seq_on, BLUE_ON);
                    knob(ui, "TEMPO", format!("{} BPM", params::knob_0_10_to_bpm(p.seq_bpm_0_10) as i32), &mut p.seq_bpm_0_10, 34.0);
                    ui.label(egui::RichText::new("Click a step to toggle rest/note. Plays 16ths.").color(SOFT).size(10.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let del_target = self.preset_current.clone().unwrap_or_default();
                        if ui
                            .add_enabled(!del_target.is_empty(), egui::Button::new("Delete"))
                            .clicked()
                        {
                            if presets::delete_preset(&del_target) {
                                self.preset_msg = format!("Deleted “{del_target}”.");
                            } else {
                                self.preset_msg = format!("Could not delete “{del_target}”.");
                            }
                            self.preset_current = None;
                            self.preset_list = presets::list_presets();
                        }
                        if ui.button("Save").clicked() {
                            match presets::save_preset(&self.preset_name, &p, Some(&self.canvas)) {
                                Some(clean) => {
                                    self.preset_current = Some(clean.clone());
                                    self.preset_msg = format!("Saved “{clean}”.");
                                    self.preset_list = presets::list_presets();
                                }
                                None => {
                                    self.preset_msg = "Enter a valid name first.".to_string();
                                }
                            }
                        }
                        ui.add(
                            egui::TextEdit::singleline(&mut self.preset_name)
                                .hint_text("name for save…")
                                .desired_width(160.0),
                        );
                        let mut pick = self.preset_current.clone().unwrap_or_default();
                        egui::ComboBox::from_id_source("preset_pick")
                            .selected_text(if pick.is_empty() { "— select —".to_string() } else { pick.clone() })
                            .width(180.0)
                            .show_ui(ui, |ui| {
                                for name in &self.preset_list {
                                    ui.selectable_value(&mut pick, name.clone(), name);
                                }
                            });
                        if pick != self.preset_current.clone().unwrap_or_default() && !pick.is_empty() {
                            if let Some((loaded, loaded_layout)) = presets::load_preset(&pick) {
                                p = loaded;
                                pending_layout = loaded_layout;
                                self.preset_current = Some(pick.clone());
                                self.preset_name = pick.clone();
                                self.preset_msg = format!("Loaded “{pick}”.");
                            } else {
                                self.preset_msg = format!("Could not load “{pick}”.");
                                self.preset_list = presets::list_presets();
                            }
                        }
                        ui.label(egui::RichText::new("PRESET").color(SOFT).size(11.0));
                        if !self.preset_msg.is_empty() {
                            ui.label(egui::RichText::new(&self.preset_msg).color(SOFT).size(11.0));
                        }
                    });
                });
                ui.horizontal_top(|ui| {
                    for i in 0..16 {
                        let cur = p.seq_on && (seq_step as usize) == i;
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(format!("{}", i + 1)).color(if cur { Color32::from_rgb(0,100,200) } else { SOFT }).strong());
                            let is_rest = p.seq_notes[i] == 0;
                            let lbl = if is_rest {
                                "rest".to_string()
                            } else {
                                params::note_name(p.seq_notes[i])
                            };
                            let b = egui::Button::new(egui::RichText::new(lbl).color(if is_rest { SOFT } else { Color32::WHITE }))
                                .min_size(Vec2::new(52.0, 26.0))
                                .fill(if cur { HIGHLIGHT } else if is_rest { OFF_FILL } else { BLACK_ON })
                                .stroke(Stroke::new(1.5_f32, INK));
                            if ui.add(b).clicked() {
                                p.seq_notes[i] = if is_rest { 60 } else { 0 };
                            }
                        });
                    }
                });
            });
            if let Some(layout) = pending_layout.take() {
                self.canvas = layout;
                self.layout_dirty = true;
            }
            // v2 overlays: marquee box, selection outlines, and alignment
            // guides for the item being dragged (crosshairs at its snapped
            // top-left plus rings on items sharing that x or y).
            if edit {
                let g = ctx.layer_painter(egui::LayerId::new(
                    egui::Order::Foreground,
                    egui::Id::new("v2guides"),
                ));
                if let (Some(a), Some(b)) = (self.marquee_start, ctx.pointer_interact_pos()) {
                    let r = egui::Rect::from_two_pos(a, b);
                    g.rect_filled(r, 0.0, Color32::from_black_alpha(16));
                    g.rect_stroke(r, 0.0, Stroke::new(1.0_f32, INK));
                }
                for oid in self.selected.iter() {
                    if let Some(ir) = self.item_rects.get(oid) {
                        g.rect_stroke(ir.expand(3.0), 3.0, Stroke::new(1.5_f32, BLUE_ON));
                    }
                }
                // Overlap alert: outline any pair covering each other so
                // cover-clipping reads as what it is — spread them apart.
                {
                    let ids: Vec<(&String, &egui::Rect)> =
                        self.item_rects.iter().collect();
                    let mut over: HashSet<&String> = HashSet::new();
                    for (i, (a, ra)) in ids.iter().enumerate() {
                        for (b, rb) in ids.iter().skip(i + 1) {
                            if ra.intersects(**rb) {
                                over.insert(*a);
                                over.insert(*b);
                            }
                        }
                    }
                    for oid in over {
                        if let Some(ir) = self.item_rects.get(oid) {
                            g.rect_stroke(
                                ir.expand(2.0),
                                2.0,
                                Stroke::new(1.0_f32, Color32::from_rgb(255, 120, 0)),
                            );
                        }
                    }
                }
                // Hidden ghosts read via a thin soft outline (no text).
                for hid in self.canvas.hidden.iter() {
                    if let Some(ir) = self.item_rects.get(hid) {
                        g.rect_stroke(ir.expand(2.0), 2.0, Stroke::new(1.0_f32, SOFT));
                    }
                }
                if let Some(aid) = self.dragging_id.clone() {
                    if let Some(ap) = self.canvas.pos.get(&aid).copied() {
                        let guide = Stroke::new(1.0_f32, SOFT);
                        g.line_segment(
                            [Pos2::new(ap[0], sheet.top()), Pos2::new(ap[0], sheet.bottom())],
                            guide,
                        );
                        g.line_segment(
                            [Pos2::new(sheet.left(), ap[1]), Pos2::new(sheet.right(), ap[1])],
                            guide,
                        );
                        for (oid, op) in self.canvas.pos.iter() {
                            if oid == &aid {
                                continue;
                            }
                            if (op[0] - ap[0]).abs() <= 1.5 || (op[1] - ap[1]).abs() <= 1.5 {
                                g.circle_stroke(
                                    Pos2::new(op[0], op[1]),
                                    5.0,
                                    Stroke::new(1.5_f32, INK),
                                );
                            }
                        }
                    }
                }
            }
            // Jack routing remains available during play; the general item
            // context menu remains restricted to layout editing.
            let jack_menu = self.menu_for.as_ref().map_or(false, |(id, _)| {
                self.canvas
                    .jacks
                    .get(id)
                        .map_or(false, |spec| spec.is_input || spec.label == "MAIN")
            });
            if edit || jack_menu {
                if let Some((mid, mpos)) = self.menu_for.clone() {
                    let is_line =
                        mid.starts_with("line") && self.canvas.lines.contains_key(&mid);
                    let is_text =
                        mid.starts_with("text") && self.canvas.texts.contains_key(&mid);
                    let is_jack =
                        mid.starts_with("jack") && self.canvas.jacks.contains_key(&mid);
                    let can_rotate = ROCKER_IDS.contains(&mid.as_str());
                    let is_hidden = self.canvas.hidden.contains(&mid);
                    let mut lspec = self.canvas.lines.get(&mid).cloned();
                    let mut spec_changed = false;
                    let mut tspec = self.canvas.texts.get(&mid).cloned();
                    let mut text_changed = false;
                    #[derive(PartialEq)]
                    enum MenuAct {
                        None,
                        Close,
                        ToggleHide,
                        Rotate,
                        DupLine,
                        DelLine,
                        DupText,
                        DelText,
                        RouteJack(String),
                        RouteDevice(String),
                        RouteMidi(String),
                        DisconnectMidi,
                    }
                    let mut act = MenuAct::None;
                    let mout = egui::Area::new(egui::Id::new("v2menu"))
                        .fixed_pos(mpos)
                        // Above the Foreground decoration layer (selection /
                        // overlap outlines, guides): without this they paint
                        // straight through the menu background.
                        .order(Order::Tooltip)
                        .show(ctx, |ui| {
                            egui::Frame::none()
                                .fill(PAPER)
                                .stroke(Stroke::new(1.5_f32, INK))
                                .rounding(6.0)
                                .inner_margin(egui::Margin::same(8.0))
                                .show(ui, |ui| {
                                    ui.vertical(|ui| {
                                        let title: &str = if is_jack {
                                            self.canvas
                                                .jacks
                                                .get(&mid)
                                                .map(|s| s.label.as_str())
                                                .unwrap_or(mid.as_str())
                                        } else if is_line {
                                            lspec.as_ref().map(|s| s.name.as_str()).unwrap_or("line")
                                        } else if is_text {
                                            "(text)"
                                        } else {
                                            mid.as_str()
                                        };
                                        ui.label(egui::RichText::new(if is_jack {
                                            title.to_string()
                                        } else {
                                            format!("{title} — {}", if is_hidden { "hidden" } else { "visible" })
                                        }).color(SOFT).size(10.0));
                                        if is_text {
                                        if let Some(t) = tspec.as_ref() {
                                            ui.label(
                                                egui::RichText::new(
                                                    t.content.lines().next().unwrap_or("(empty)"),
                                                )
                                                .color(SOFT)
                                                .size(10.0),
                                            );
                                        }
                                        }
                                        if edit {
                                            if ui
                                                .button(if is_hidden { "Show item" } else { "Hide item" })
                                                .clicked()
                                            {
                                                act = MenuAct::ToggleHide;
                                            }
                                            if can_rotate {
                                                let now_v = self.canvas.rot90.contains(&mid);
                                                if ui
                                                    .button(format!(
                                                        "Rotate 90° (now {})",
                                                        if now_v { "vertical" } else { "horizontal" }
                                                    ))
                                                    .clicked()
                                                {
                                                    act = MenuAct::Rotate;
                                                }
                                            } else if !is_line {
                                                ui.add_enabled(false, egui::Button::new("Rotate 90°"));
                                            }
                                        }
                                        if let Some(s) = lspec.as_mut() {
                                            ui.separator();
                                            ui.horizontal(|ui| {
                                                ui.label("Length");
                                                if ui.button("−").clicked() {
                                                    s.len = (s.len - 8.0).clamp(8.0, 1200.0);
                                                    spec_changed = true;
                                                }
                                                let mut v = s.len;
                                                if ui
                                                    .add(
                                                        egui::DragValue::new(&mut v)
                                                            .range(8.0..=1200.0)
                                                            .speed(1.0)
                                                            .max_decimals(0),
                                                    )
                                                    .changed()
                                                {
                                                    s.len = v.clamp(8.0, 1200.0);
                                                    spec_changed = true;
                                                }
                                                if ui.button("+").clicked() {
                                                    s.len = (s.len + 8.0).clamp(8.0, 1200.0);
                                                    spec_changed = true;
                                                }
                                            });
                                            ui.horizontal(|ui| {
                                                ui.label("Thick");
                                                if ui.button("−").clicked() {
                                                    s.thick = (s.thick - 1.0).clamp(1.0, 24.0);
                                                    spec_changed = true;
                                                }
                                                let mut v = s.thick;
                                                if ui
                                                    .add(
                                                        egui::DragValue::new(&mut v)
                                                            .range(1.0..=24.0)
                                                            .speed(0.2)
                                                            .max_decimals(0),
                                                    )
                                                    .changed()
                                                {
                                                    s.thick = v.clamp(1.0, 24.0);
                                                    spec_changed = true;
                                                }
                                                if ui.button("+").clicked() {
                                                    s.thick = (s.thick + 1.0).clamp(1.0, 24.0);
                                                    spec_changed = true;
                                                }
                                            });
                                            if ui
                                                .button(if s.vertical {
                                                    "→ Horizontal"
                                                } else {
                                                    "→ Vertical"
                                                })
                                                .clicked()
                                            {
                                                s.vertical = !s.vertical;
                                                spec_changed = true;
                                            }
                                            if ui.button("Duplicate line").clicked() {
                                                act = MenuAct::DupLine;
                                            }
                                            if ui.button("Delete line").clicked() {
                                                act = MenuAct::DelLine;
                                            }
                                        }
                                        if let Some(t) = tspec.as_mut() {
                                            ui.separator();
                                            // Multiline: Enter = new line, so
                                            // wording reflows to 1/2/N lines.
                                            ui.horizontal(|ui| {
                                                ui.label("Text");
                                                let mut v = t.content.clone();
                                                if ui
                                                    .add(
                                                        egui::TextEdit::multiline(&mut v)
                                                            .desired_width(140.0)
                                                            .desired_rows(2),
                                                    )
                                                    .changed()
                                                {
                                                    t.content = v;
                                                    text_changed = true;
                                                }
                                            });
                                            ui.horizontal(|ui| {
                                                ui.label("Size");
                                                if ui.button("−").clicked() {
                                                    t.size = (t.size - 1.0).clamp(8.0, 48.0);
                                                    text_changed = true;
                                                }
                                                let mut v = t.size;
                                                if ui
                                                    .add(
                                                        egui::DragValue::new(&mut v)
                                                            .range(8.0..=48.0)
                                                            .speed(0.2)
                                                            .max_decimals(0),
                                                    )
                                                    .changed()
                                                {
                                                    t.size = v.clamp(8.0, 48.0);
                                                    text_changed = true;
                                                }
                                                if ui.button("+").clicked() {
                                                    t.size = (t.size + 1.0).clamp(8.0, 48.0);
                                                    text_changed = true;
                                                }
                                            });
                                            if ui.button("Duplicate text").clicked() {
                                                act = MenuAct::DupText;
                                            }
                                            if ui.button("Delete text").clicked() {
                                                act = MenuAct::DelText;
                                            }
                                        }
                                        if is_jack {
                                            if let Some(jspec) = self.canvas.jacks.get(&mid).cloned() {
                                                if jspec.label == "MAIN" {
                                                    ui.separator();
                                                    ui.label(
                                                        egui::RichText::new("Computer audio output:")
                                                            .color(SOFT)
                                                            .size(10.0),
                                                    );
                                                    if self.audio_output_names.is_empty() {
                                                        ui.label("No output devices found");
                                                    }
                                                    for device_name in &self.audio_output_names {
                                                        if ui
                                                            .selectable_label(
                                                                jspec.device_name == *device_name,
                                                                device_name,
                                                            )
                                                            .clicked()
                                                        {
                                                            act = MenuAct::RouteDevice(device_name.clone());
                                                        }
                                                    }
                                                } else if jspec.label == "MIDI IN" {
                                                    ui.separator();
                                                    ui.label(
                                                        egui::RichText::new("MIDI input device:")
                                                            .color(SOFT)
                                                            .size(10.0),
                                                    );
                                                    if self.midi_input_names.is_empty() {
                                                        ui.label("No MIDI input devices found");
                                                    }
                                                    for device_name in &self.midi_input_names {
                                                        if ui
                                                            .selectable_label(
                                                                jspec.device_name == *device_name,
                                                                device_name,
                                                            )
                                                            .clicked()
                                                        {
                                                            act = MenuAct::RouteMidi(device_name.clone());
                                                        }
                                                    }
                                                    if !jspec.device_name.is_empty()
                                                        && ui.button("Disconnect").clicked()
                                                    {
                                                        act = MenuAct::DisconnectMidi;
                                                    }
                                                } else if jspec.is_input {
                                                    if jspec.label == "EXT" {
                                                        ui.separator();
                                                    ui.label(
                                                        egui::RichText::new("Computer audio input:")
                                                            .color(SOFT)
                                                            .size(10.0),
                                                    );
                                                    if self.audio_input_names.is_empty() {
                                                        ui.label("No input devices found");
                                                    }
                                                    for device_name in &self.audio_input_names {
                                                        if ui
                                                            .selectable_label(
                                                                jspec.device_name == *device_name,
                                                                device_name,
                                                            )
                                                            .clicked()
                                                         {
                                                             act = MenuAct::RouteDevice(device_name.clone());
                                                         }
                                                      }
                                                    }
                                                    ui.separator();
                                                    ui.label(
                                                        egui::RichText::new("Route from output:")
                                                            .color(SOFT)
                                                            .size(10.0),
                                                    );
                                                    let mut out_ids: Vec<String> = self
                                                        .canvas
                                                        .jacks
                                                        .iter()
                                                        .filter(|(id, s)| {
                                                            !s.is_input && compatible_output(&mid, id)
                                                        })
                                                        .map(|(k, _)| k.clone())
                                                        .collect();
                                                    out_ids.sort();
                                                    for oid in out_ids {
                                                        let olabel = self
                                                            .canvas
                                                            .jacks
                                                            .get(&oid)
                                                            .map(|s| s.label.clone())
                                                            .unwrap_or(oid.clone());
                                                        let sel = jspec.output_id == oid;
                                                        let color = output_color(&self.canvas, &oid);
                                                        let mut picked = false;
                                                        ui.horizontal(|ui| {
                                                            let (dot, dot_resp) = ui.allocate_exact_size(
                                                                Vec2::new(18.0, 22.0),
                                                                Sense::click(),
                                                            );
                                                            ui.painter().circle_filled(
                                                                dot.center(),
                                                                5.0,
                                                                color,
                                                            );
                                                            picked |= dot_resp.clicked();
                                                            picked |= ui.selectable_label(sel, olabel).clicked();
                                                        });
                                                        if picked {
                                                            act = MenuAct::RouteJack(oid);
                                                        }
                                                    }
                                                    if !jspec.output_id.is_empty() {
                                                        if ui.button("Disconnect").clicked() {
                                                            act = MenuAct::RouteJack(String::new());
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        if ui.button("Close").clicked() {
                                            act = MenuAct::Close;
                                        }
                                    });
                                });
                        });
                    self.menu_rect = Some(mout.response.rect);
                    if spec_changed {
                        if let Some(s) = lspec {
                            self.canvas.lines.insert(mid.clone(), s);
                            self.layout_dirty = true;
                        }
                    }
                    if text_changed {
                        if let Some(t) = tspec {
                            self.canvas.texts.insert(mid.clone(), t);
                            self.layout_dirty = true;
                        }
                    }
                    match act {
                        MenuAct::None => {}
                        MenuAct::Close => {
                            self.menu_for = None;
                        }
                        MenuAct::ToggleHide => {
                            if !self.canvas.hidden.remove(&mid) {
                                self.canvas.hidden.insert(mid.clone());
                            }
                            self.layout_dirty = true;
                            self.menu_for = None;
                        }
                        MenuAct::Rotate => {
                            if !self.canvas.rot90.remove(&mid) {
                                self.canvas.rot90.insert(mid.clone());
                            }
                            self.layout_dirty = true;
                            self.menu_for = None;
                        }
                        MenuAct::DupLine => {
                            if let Some(s) = self.canvas.lines.get(&mid).cloned() {
                                let n = self.canvas.line_seq + 1;
                                self.canvas.line_seq = n;
                                let nid = format!("line{n}");
                                let at = self
                                    .canvas
                                    .pos
                                    .get(&mid)
                                    .copied()
                                    .unwrap_or([800.0, 420.0]);
                                let g = self.canvas.grid;
                                self.canvas.pos.insert(
                                    nid.clone(),
                                    [
                                        layout::snap(at[0] + 16.0, g),
                                        layout::snap(at[1] + 16.0, g),
                                    ],
                                );
                                self.canvas.lines.insert(
                                    nid.clone(),
                                    layout::LineSpec {
                                        name: format!("Line {n}"),
                                        ..s
                                    },
                                );
                                self.selected.clear();
                                self.selected.insert(nid);
                                self.layout_dirty = true;
                            }
                            self.menu_for = None;
                        }
                        MenuAct::DelLine => {
                            self.canvas.lines.remove(&mid);
                            self.canvas.pos.remove(&mid);
                            self.selected.remove(&mid);
                            self.canvas.hidden.remove(&mid);
                            self.layout_dirty = true;
                            self.menu_for = None;
                        }
                        MenuAct::DupText => {
                            if let Some(t) = self.canvas.texts.get(&mid).cloned() {
                                let n = self.canvas.text_seq + 1;
                                self.canvas.text_seq = n;
                                let nid = format!("text{n}");
                                let at = self
                                    .canvas
                                    .pos
                                    .get(&mid)
                                    .copied()
                                    .unwrap_or([800.0, 420.0]);
                                let g = self.canvas.grid;
                                self.canvas.pos.insert(
                                    nid.clone(),
                                    [
                                        layout::snap(at[0] + 16.0, g),
                                        layout::snap(at[1] + 16.0, g),
                                    ],
                                );
                                self.canvas.texts.insert(
                                    nid.clone(),
                                    layout::TextSpec {
                                        content: t.content,
                                        size: t.size,
                                        wrap: t.wrap,
                                    },
                                );
                                self.selected.clear();
                                self.selected.insert(nid);
                                self.layout_dirty = true;
                            }
                            self.menu_for = None;
                        }
                        MenuAct::DelText => {
                            self.canvas.texts.remove(&mid);
                            self.canvas.pos.remove(&mid);
                            self.selected.remove(&mid);
                            self.canvas.hidden.remove(&mid);
                            self.layout_dirty = true;
                            self.menu_for = None;
                        }
                        MenuAct::RouteJack(oid) => {
                            if let Some(jspec) = self.canvas.jacks.get_mut(&mid) {
                                jspec.output_id = oid.clone();
                                self.layout_dirty = true;
                            }
                            let route_bit = match mid.as_str() {
                                "jack_mods" => Some(0),
                                "jack_osc1v" => Some(1),
                                "jack_lfocv" => Some(2),
                                "jack_ext" => Some(3),
                                "jack_cutcv" => Some(4),
                                "jack_fcgate" => Some(5),
                                "jack_lcgate" => Some(6),
                                "jack_loudcv" => Some(7),
                                _ => None,
                            };
                            if let Some(bit) = route_bit {
                                if oid == "jack_phones" {
                                    p.phones_route_mask |= 1 << bit;
                                } else {
                                    p.phones_route_mask &= !(1 << bit);
                                }
                                p.jack_routes[bit] = output_route_code(&oid);
                            }
                            self.menu_for = None;
                        }
                        MenuAct::RouteDevice(device_name) => {
                            if let Some(jspec) = self.canvas.jacks.get_mut(&mid) {
                                jspec.device_name = device_name;
                                self.layout_dirty = true;
                            }
                            self.menu_for = None;
                        }
                        MenuAct::RouteMidi(device_name) => {
                            if let Some(jspec) = self.canvas.jacks.get_mut(&mid) {
                                jspec.device_name = device_name.clone();
                                self.layout_dirty = true;
                            }
                            if let Ok(mut selected) = self.midi_selection.lock() {
                                *selected = Some(device_name);
                            }
                            self.menu_for = None;
                        }
                        MenuAct::DisconnectMidi => {
                            if let Some(jspec) = self.canvas.jacks.get_mut(&mid) {
                                jspec.device_name.clear();
                                self.layout_dirty = true;
                            }
                            if let Ok(mut selected) = self.midi_selection.lock() {
                                *selected = None;
                            }
                            self.menu_for = None;
                        }
                    }
                } else {
                    self.menu_rect = None;
                }
            } else {
                self.menu_rect = None;
            }
        });

        midi_learn_window(
            ctx,
            &mut self.learn_open,
            &mut self.learning,
            &mut self.learn_armed_seq,
            &mut self.learn_msg,
            &self.midi_status,
            cur_cc_seq,
            &mut p,
        );

        cheat_sheet_window(ctx, &mut self.cheat_open);

        if let Ok(mut s) = self.shared.lock() { s.params = p; }
        // Autosave last working state (throttled): restart restores it.
        if let Ok(json) = serde_json::to_string(&p) {
            if json != self.last_saved_json
                && self.last_autosave.elapsed() > std::time::Duration::from_millis(1500)
            {
                presets::save_last(&p, Some(&self.canvas));
                self.last_saved_json = json;
                self.last_autosave = std::time::Instant::now();
            }
        }
        // Autosave canvas layout (throttled): arrangement survives restart.
        if self.layout_dirty
            && self.last_layout_save.elapsed() > std::time::Duration::from_millis(800)
        {
            layout::save(&self.canvas);
            self.layout_dirty = false;
            self.last_layout_save = std::time::Instant::now();
        }
        // Commit a layout-undo snapshot when an edit settles: on the frame a
        // drag begins (capturing the pre-drag state) or on any discrete
        // change while not dragging. A whole drag is one undo step.
        let dragging_now = self.dragging_id.is_some();
        if self.skip_undo_commit {
            // Undo/redo already rewrote the layout this frame; don't record it.
            self.skip_undo_commit = false;
        } else if dragging_now && !self.was_dragging {
            self.push_undo(frame_start_canvas.clone());
        } else if !dragging_now && !self.was_dragging && self.canvas != frame_start_canvas {
            self.push_undo(frame_start_canvas.clone());
        }
        self.was_dragging = dragging_now;

        ctx.request_repaint_after(std::time::Duration::from_millis(40));
    }
}

/// MIDI-learn panel: bind any learnable parameter to a MIDI CC.
///
/// The panel is purely an editor over [`Params::midi_cc`]; the actual CC
/// event routing happens in `midi.rs`, and bindings persist inside the
/// patch file. `cur_cc_seq` is the live CC counter used to detect "the
/// user just moved a control" while a parameter is armed.
fn midi_learn_window(
    ctx: &egui::Context,
    open: &mut bool,
    learning: &mut Option<u8>,
    armed_seq: &mut u32,
    msg: &mut String,
    midi_status: &str,
    cur_cc_seq: u32,
    p: &mut params::Params,
) {
    if !*open {
        return;
    }
    let mut visible = true;
    egui::Window::new("MIDI LEARN")
        .open(&mut visible)
        .default_width(380.0)
        .resizable(true)
        .show(ctx, |ui| {
            ui.label(
                egui::RichText::new(
                    "Click Learn next to a control, then move a knob/fader on your MIDI device.",
                )
                .size(11.0),
            );
            if !midi_status.is_empty() {
                ui.label(egui::RichText::new(midi_status).color(SOFT).size(10.0));
            }
            if let Some(param) = *learning {
                let name = learn::P::from_id(param).map(|x| x.name()).unwrap_or("?");
                ui.horizontal(|ui| {
                    ui.colored_label(RED_ON, format!("Waiting for CC… ({name})"));
                    if ui.button("Cancel").clicked() {
                        *learning = None;
                        *msg = "Learn cancelled.".to_string();
                    }
                });
            } else if !msg.is_empty() {
                ui.label(egui::RichText::new(msg.as_str()).color(SOFT).size(11.0));
            }
            ui.separator();
            egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
                let mut section = "";
                for &param in learn::P::ALL {
                    if param.section() != section {
                        section = param.section();
                        ui.add_space(6.0);
                        ui.label(egui::RichText::new(section).strong().size(11.0));
                    }
                    ui.horizontal(|ui| {
                        // Current live value (ON/OFF for toggles).
                        let val = learn::get(p, param.id()).unwrap_or(0.0);
                        let val_text = if param.is_toggle() {
                            if val >= 5.0 { "ON".to_string() } else { "OFF".to_string() }
                        } else {
                            format!("{val:.1}")
                        };
                        ui.label(egui::RichText::new(param.name()).size(11.0));
                        ui.label(
                            egui::RichText::new(format!("[{val_text:>4}]"))
                                .monospace()
                                .color(SOFT)
                                .size(10.0),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let bound = learn::cc_for(p, param.id());
                            let cc_text = bound
                                .map(|c| format!("CC {c:>3}"))
                                .unwrap_or_else(|| "  —  ".to_string());
                            ui.label(egui::RichText::new(cc_text).monospace().size(11.0));
                            let is_armed = *learning == Some(param.id());
                            let btn = if is_armed { "●" } else { "Learn" };
                            if ui.button(btn).clicked() {
                                if is_armed {
                                    *learning = None;
                                    *msg = "Learn cancelled.".to_string();
                                } else {
                                    *learning = Some(param.id());
                                    *armed_seq = cur_cc_seq;
                                    *msg = format!("Arming {}…", param.name());
                                }
                            }
                            if ui
                                .add_enabled(bound.is_some(), egui::Button::new("Clear"))
                                .clicked()
                            {
                                learn::unbind(p, param.id());
                                if *learning == Some(param.id()) {
                                    *learning = None;
                                }
                                *msg = format!("Cleared {}.", param.name());
                            }
                        });
                    });
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Clear all bindings").clicked() {
                    learn::clear_all(p);
                    *learning = None;
                    *msg = "Cleared all MIDI bindings.".to_string();
                }
                let used = p.midi_cc.iter().filter(|b| b.param != 0).count();
                ui.label(
                    egui::RichText::new(format!("{used}/{} slots", params::CC_SLOTS))
                        .color(SOFT)
                        .size(11.0),
                );
            });
        });
    if !visible {
        *open = false;
    }
}

/// Static reference for the CHEAT SHEET window: every knob, toggle, selector
/// and jack with a one-line explanation. Sections are `(title, rows)`.
const CHEAT: &[(&str, &[(&str, &str)])] = &[
    (
        "CONTROLLERS",
        &[
            ("TUNE", "Master pitch trim. Centered = concert pitch."),
            ("GLIDE", "Portamento glide time between notes. 0 = none."),
            ("MOD MIX", "Crossfades the two modulation sources (A ↔ B)."),
            ("MOD DEPTH", "How much modulation (vibrato / mod wheel) is applied."),
            ("LFO RATE", "Speed of the low-frequency oscillator."),
            ("TRIANGLE / SQUARE", "LFO waveform shape."),
            ("OSCILLATOR MODULATION", "Enables the LFO modulating the oscillators."),
        ],
    ),
    (
        "OSCILLATORS & MIXER",
        &[
            ("RANGE", "Octave range: LO, 32', 16', 8', 4', 2'. Left-click up, right-click down."),
            ("WAVE", "Waveform. Left-click cycles forward, right-click backward."),
            ("FREQUENCY (OSC 2/3)", "Detune of OSC 2 / OSC 3 against OSC 1."),
            ("OSC 3 CONTROL", "OSC 3 keyboard tracking. OFF = free-running."),
            ("OSC n VOL (knob)", "Mixer level of oscillator n."),
            ("OSC n VOL (switch)", "Places oscillator n into the mixer."),
            ("EXT IN", "Feeds the computer's audio input into the mixer."),
            ("EXT IN VOLUME", "Level of the external input."),
            ("NOISE", "Turns the noise generator on."),
            ("NOISE VOLUME", "Noise level in the mixer."),
            ("WHITE / PINK", "Noise colour: WHITE (flat) or PINK (darker)."),
        ],
    ),
    (
        "FILTER",
        &[
            ("CUTOFF", "Low-pass cutoff. Lower = darker."),
            ("EMPHASIS", "Resonance boost at the cutoff. High values self-oscillate."),
            ("CONTOUR", "How far the filter envelope opens the cutoff (0..6 octaves)."),
            ("ATTACK", "Filter envelope attack time."),
            ("DECAY", "Filter envelope decay time."),
            ("SUSTAIN", "Filter envelope sustain level."),
            ("FILTER MOD", "Sends the modulation bus to the cutoff."),
            ("FILTER LO / HI", "Cutoff range switch: LO normal, HI brighter."),
            ("KEY 1 (1/3)", "Adds 1/3 keyboard tracking to the cutoff."),
            ("KEY 2 (2/3)", "Adds 2/3 keyboard tracking (both = full)."),
            ("FILTER DECAY", "ON = filter release follows DECAY; OFF = quick release."),
        ],
    ),
    (
        "LOUDNESS",
        &[
            ("ATTACK", "Loudness (amplifier) envelope attack time."),
            ("DECAY", "Loudness envelope decay time."),
            ("SUSTAIN", "Loudness envelope sustain level."),
            ("LOUD DECAY", "ON = loudness release follows DECAY; OFF = quick release."),
        ],
    ),
    (
        "OUTPUT",
        &[
            ("VOLUME", "Main output level."),
            ("MAIN OUT", "Turns the main audio output on/off."),
            ("A-440", "Adds a 440 Hz reference tone to the output."),
            ("VOLUME (phones)", "Headphone trim (disabled in this build)."),
        ],
    ),
    (
        "SEQUENCER",
        &[
            ("SEQ PLAY", "Runs the built-in 16-step sequencer."),
            ("TEMPO", "Sequencer tempo in BPM."),
        ],
    ),
    (
        "PATCH BAY — INPUTS",
        &[
            ("MOD SOURCE", "Modulation-bus input. Patch LFO SAW/SQR here."),
            ("OSC1V/OCT", "Pitch CV input for OSC 1."),
            ("LFO CV", "CV input for the LFO rate."),
            ("EXT", "External audio input (accepts MIX, PHONES or MAIN)."),
            ("CUT CV", "CV input for the filter cutoff."),
            ("FC GATE", "Gate for the filter contour (accepts LFO SQR)."),
            ("LC GATE", "Gate for the loudness contour (accepts LFO SQR)."),
            ("LOUD CV", "CV input for loudness."),
            ("MIDI IN", "5-pin DIN MIDI note + CC input."),
        ],
    ),
    (
        "PATCH BAY — OUTPUTS",
        &[
            ("LFO SAW", "LFO sawtooth output."),
            ("LFO SQR", "LFO square output (also usable as a gate)."),
            ("MIX", "Raw oscillator/noise mixer output."),
            ("FILT CONT", "Filter envelope output."),
            ("LOUD CONT", "Loudness envelope output."),
            ("PHONES", "Headphone output."),
            ("MAIN", "Main audio output."),
        ],
    ),
];

/// Reference window describing what every control and jack does.
fn cheat_sheet_window(ctx: &egui::Context, open: &mut bool) {
    if !*open {
        return;
    }
    let mut visible = true;
    egui::Window::new("CHEAT SHEET")
        .open(&mut visible)
        .default_width(460.0)
        .default_height(600.0)
        .resizable(true)
        .show(ctx, |ui| {
            ui.label(
                egui::RichText::new(
                    "RANGE / WAVE switches: left-click steps up, right-click steps down.",
                )
                .size(11.0),
            );
            ui.separator();
            // auto_shrink(false) horizontally keeps the scroll area full-width
            // so the scrollbar sits at the window edge instead of hugging the text.
            egui::ScrollArea::vertical()
                .auto_shrink([false, true])
                .show(ui, |ui| {
                for (section, rows) in CHEAT {
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new(*section).strong().size(12.0));
                    for (name, desc) in rows.iter() {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(egui::RichText::new(format!("{name} — ")).strong().size(11.0));
                            ui.label(egui::RichText::new(*desc).size(11.0));
                        });
                    }
                }
            });
        });
    if !visible {
        *open = false;
    }
}

pub fn run(
    shared: Arc<Mutex<Shared>>,
    midi_status: String,
    midi_selection: Arc<Mutex<Option<String>>>,
) {
    // Unicorn logo as the OS window icon (title bar + taskbar).
    let win_icon = image::load_from_memory(include_bytes!("../400x400icon.jpg"))
        .ok()
        .map(|img| {
            let img = img.to_rgba8();
            let (w, h) = (img.width(), img.height());
            std::sync::Arc::new(egui::IconData {
                rgba: img.into_raw(),
                width: w,
                height: h,
            })
        });
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1720.0, 980.0])
        .with_min_inner_size([1720.0, 980.0])
        .with_max_inner_size([1720.0, 980.0])
        .with_resizable(false);
    if let Some(icon) = win_icon {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    let shared2 = shared;
    let _ = eframe::run_native(
        concat!("MiniUnicorn v", env!("CARGO_PKG_VERSION")),
        options,
        Box::new(|cc| {
            // Match the OS scale factor so glyphs render at native resolution
            // instead of being upscaled blurry from a 1x atlas.
            let scale = cc.egui_ctx.native_pixels_per_point().unwrap_or(1.0);
            if (scale - 1.0).abs() > 0.01 {
                cc.egui_ctx.set_pixels_per_point(scale);
            }
            // Decode the app icon once with the `image` crate (no egui image
            // loader needed) and upload it as a texture.
            let icon = image::load_from_memory(include_bytes!("../MINIUNICORN.jpg"))
                .ok()
                .map(|img| {
                    let mut img = img.to_rgba8();
                    // Trim the banner's white left margin (the art lives in
                    // the right ~420px) so the bar's free space is real and
                    // the edit tools fit beside it at full height.
                    let (w0, h0) = (img.width(), img.height());
                    let mut left = 0;
                    'scan: for x in 0..w0 {
                        for y in (0..h0).step_by(2) {
                            let p = img.get_pixel(x, y).0;
                            if p[0] < 230 || p[1] < 230 || p[2] < 230 {
                                left = x;
                                break 'scan;
                            }
                        }
                    }
                    if left > 12 {
                        img = image::imageops::crop_imm(&img, left - 8, 0, w0 - left + 8, h0)
                            .to_image();
                    }
                    let (w, h) = (img.width() as usize, img.height() as usize);
                    let pixels = img.into_raw();
                    let color =
                        egui::ColorImage::from_rgba_unmultiplied([w, h], &pixels);
                    cc.egui_ctx.load_texture("app-icon", color, Default::default())
                });
            Ok(Box::new(ModelDApp::new(shared2, midi_status, midi_selection, icon)))
        }),
    );
}
