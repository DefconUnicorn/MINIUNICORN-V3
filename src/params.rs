//! Shared synth parameters + runtime note state, laid out like a Model D.
//!
//! Audio thread copies this once per callback (small, cheap).
//! Knobs in the UI are 0..10 like the hardware; conversion helpers
//! below map them to Hz / seconds so you learn what the hardware does.

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum OscWave {
    Triangle,
    TriSaw, // triangular/saw blend (OSC 1+2)
    Saw,
    ReverseSaw, // OSC 3
    Square,
    WidePulse,
    NarrowPulse,
}

impl OscWave {
    /// Waves available per oscillator per manual §2 (OSC1/2 vs OSC3 differ).
    pub fn for_osc(osc_idx: usize) -> [OscWave; 6] {
        if osc_idx == 2 {
            [
                Self::Triangle,
                Self::ReverseSaw,
                Self::Saw,
                Self::Square,
                Self::WidePulse,
                Self::NarrowPulse,
            ]
        } else {
            [
                Self::Triangle,
                Self::TriSaw,
                Self::Saw,
                Self::Square,
                Self::WidePulse,
                Self::NarrowPulse,
            ]
        }
    }
}

/// One MIDI CC -> parameter binding for MIDI learn. `param` is a
/// [`crate::learn::P`] discriminant; `param == 0` marks the slot empty.
/// The array length is the maximum number of simultaneous bindings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CcBinding {
    pub cc: u8,
    pub param: u8,
}

impl Default for CcBinding {
    fn default() -> Self {
        Self { cc: 0, param: 0 }
    }
}

/// Number of MIDI-learn CC slots.
pub const CC_SLOTS: usize = 32;

fn default_cc_map() -> [CcBinding; CC_SLOTS] {
    [CcBinding::default(); CC_SLOTS]
}

fn default_voice_count() -> u8 {
    1
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Params {
    // --- Controllers — manual §3.1.2 items (4)-(12), specs p34 ---
    pub tune_0_10: f32,       // TUNE -2..+2 (specs)
    pub glide_0_10: f32,      // (5) GLIDE 0..10
    pub mod_mix_0_10: f32,    // (8) MOD MIX: OSC3/FILT EG <> NOISE/LFO
    pub mod_depth_0_10: f32,  // (10) MOD DEPTH 0..10 (+ mod wheel)
    pub lfo_rate_0_10: f32,   // (12) LFO RATE 0.05..200 Hz (specs)
    pub lfo_square: bool,     // (11) WAVE SHAPE: false=triangle, true=square
    pub osc_mod_on: bool,     // (7) OSCILLATOR MODULATION ON
    // (4) OSC3/FILTER EG: false = OSC3, true = Filter Envelope as source A
    pub mod_src_a_is_filter_eg: bool,
    // (9) NOISE/LFO: false = Noise (or ext), true = LFO as source B
    pub mod_src_b_is_lfo: bool,

    // --- Oscillator bank ---
    // Range in octaves: LO (-3), 32' (-2), 16' (-1), 8' (0), 4' (+1), 2' (+2)
    pub osc1_range_oct: i32,
    pub osc2_range_oct: i32,
    pub osc3_range_oct: i32,
    // OSC 2/3 frequency: 5 = centered (+/-7 semitones). OSC 1 has no
    // detune knob on the hardware; it is the reference.
    pub osc2_freq_0_10: f32,
    pub osc3_freq_0_10: f32,
    pub osc1_wave: OscWave,
    pub osc2_wave: OscWave,
    pub osc3_wave: OscWave,
    pub osc3_kb_ctrl: bool, // (13) OSC 3 CONTROL: keyboard tracks OSC3 or not

    // --- Mixer — manual §3.1.4 items (20)-(25) ---
    pub osc1_on: bool,
    pub osc2_on: bool,
    pub osc3_on: bool,
    pub noise_on: bool,
    pub ext_on: bool,
    pub osc1_level_0_10: f32,
    pub osc2_level_0_10: f32,
    pub osc3_level_0_10: f32,
    pub noise_level_0_10: f32,
    pub ext_level_0_10: f32, // (25) EXT IN VOLUME: 0 = dry, up = output feedback crunch
    pub noise_pink: bool, // (23) WHITE/PINK (true = pink, thumb down)
    pub filter_mode_hi: bool, // (33) FILTER MODE LO/HI (in mixer row on hardware)

    // --- Modifiers: filter ---
    pub cutoff_0_10: f32,
    pub reso_0_10: f32,       // emphasis
    pub contour_amt_0_10: f32,
    pub filter_mod_on: bool, // (32) FILTER MODULATION ON
    pub kb_control_1: bool, // (31) 1/3 when only this ON
    pub kb_control_2: bool, // (31) 2/3 when only this ON, max when both ON
    pub filter_decay_on: bool, // (30) FILTER DECAY
    pub filt_attack_0_10: f32, // (35) 1ms..10s
    pub filt_decay_0_10: f32,  // (35) 4ms..35s+
    pub filt_sustain_0_10: f32,

    // --- Modifiers: loudness — manual (30),(34) ---
    pub loud_decay_on: bool, // (30) LOUDNESS DECAY
    pub amp_attack_0_10: f32,
    pub amp_decay_0_10: f32,
    pub amp_sustain_0_10: f32,

    // --- Output ---
    pub volume_0_10: f32,
    // Disabled in the UI (kept in place, no-op); retained for preset compat.
    pub phones_0_10: f32,
    /// PHONES routed to input jacks: OSC1V, EXT, CUT CV, FC GATE,
    /// FILT CONT, LC GATE, LOUD CONT.
    #[serde(default)]
    pub phones_route_mask: u8,
    /// Output route for OSC1V, EXT, CUT CV, FC GATE, FILT CONT, LC GATE,
    /// LOUD CONT. 0 = unpatched; output ids use the UI's jack route codes.
    #[serde(default)]
    pub jack_routes: [u8; 8],
    pub main_on: bool,
    pub a440_on: bool,
    pub power_on: bool,

    // --- Keys + 16-step sequencer (extra section below the diagram) ---
    pub seq_on: bool,
    pub seq_bpm_0_10: f32, // 0..10 -> 40..240 BPM
    pub seq_notes: [u8; 16], // 0 = rest, else MIDI note

    /// Number of voices (1 = monophonic). Paraphonic/voice count selection.
    #[serde(default = "default_voice_count")]
    pub voice_count: u8,

    /// Polyphonic loudness handling: 0 = NATURAL (sum, no dip), 1 = GENTLE
    /// (1/N^0.25), 2 = CONSTANT (1/sqrt(N), fixed total level).
    #[serde(default)]
    pub poly_level_mode: u8,

    // Removed knobs (v2): kept only so old preset/last.json files still
    // load (v1 shares them). DSP uses fixed defaults; UI ignores these.
    pub drift_0_10: f32,
    pub release_0_10: f32,

    /// MIDI-learn bindings (CC -> parameter). Persisted in patches.
    #[serde(default = "default_cc_map")]
    pub midi_cc: [CcBinding; CC_SLOTS],
}

impl Default for Params {
    fn default() -> Self {
        Self {
            tune_0_10: 5.0,
            glide_0_10: 1.0,
            mod_mix_0_10: 5.0,
            mod_depth_0_10: 0.0,
            lfo_rate_0_10: 3.0,
            lfo_square: false,
            osc_mod_on: true, // hardware print shows ON
            mod_src_a_is_filter_eg: false, // OSC3 (manual §4.8 starter)
            mod_src_b_is_lfo: false,       // Noise default per manual p6

            osc1_range_oct: 0,
            osc2_range_oct: 0,
            osc3_range_oct: 0,
            osc2_freq_0_10: 5.35,
            osc3_freq_0_10: 4.8,
            osc1_wave: OscWave::Saw,
            osc2_wave: OscWave::Saw,
            osc3_wave: OscWave::Saw,
            osc3_kb_ctrl: true,

            osc1_on: true,
            osc2_on: true,
            osc3_on: true,
            noise_on: false,
            ext_on: false,
            osc1_level_0_10: 7.0,
            osc2_level_0_10: 7.0,
            osc3_level_0_10: 6.0,
            noise_level_0_10: 0.0,
            ext_level_0_10: 0.0,
            noise_pink: false,
            filter_mode_hi: false,

            cutoff_0_10: 6.0,
            reso_0_10: 3.5,
            contour_amt_0_10: 6.0,
            filter_mod_on: true,
            kb_control_1: true,
            kb_control_2: true,
            filter_decay_on: true,
            filt_attack_0_10: 0.5,
            filt_decay_0_10: 3.0,
            filt_sustain_0_10: 8.0,

            loud_decay_on: true,
            amp_attack_0_10: 0.5,
            amp_decay_0_10: 3.0,
            amp_sustain_0_10: 8.0,

            volume_0_10: 6.0,
            phones_0_10: 5.0,
            phones_route_mask: 0,
            jack_routes: [0; 8],
            main_on: true,
            a440_on: false,
            power_on: true,

            seq_on: false,
            seq_bpm_0_10: 5.0, // ~120 BPM
            // C minor-ish ascent across 16 steps, C4=60 base
            seq_notes: [48, 0, 51, 0, 53, 0, 55, 0, 58, 0, 55, 53, 51, 48, 46, 0],

            voice_count: 1,
            poly_level_mode: 0,
            drift_0_10: 1.5,
            release_0_10: 2.0,
            midi_cc: default_cc_map(),
        }
    }
}

// --- 0..10 conversions (what the hardware knob positions mean) ---

pub fn knob_0_10_to_cutoff(k: f32) -> f32 {
    // Full audible filter range: 10 Hz..20 kHz on a logarithmic sweep.
    let t = (k.clamp(0.0, 10.0) / 10.0) as f64;
    (10.0f64 * (20000.0f64 / 10.0f64).powf(t)) as f32
}

pub fn knob_0_10_to_attack(k: f32) -> f32 {
    // Specs p34: 1ms..10s
    let t = k.clamp(0.0, 10.0) / 10.0;
    0.001 * (10000.0f32).powf(t)
}

pub fn knob_0_10_to_decay(k: f32) -> f32 {
    // Specs p34: 4ms..35s+
    let t = k.clamp(0.0, 10.0) / 10.0;
    0.004 * (9000.0f32).powf(t)
}

pub fn knob_0_10_to_glide(k: f32) -> f32 {
    let t = k.clamp(0.0, 10.0) / 10.0;
    if t <= 0.01 {
        0.0
    } else {
        0.005 * (80.0f32).powf(t) // ~5ms..400ms
    }
}

pub fn knob_0_10_to_level(k: f32) -> f32 {
    k.clamp(0.0, 10.0) / 10.0
}

pub fn knob_0_10_to_fine_semi(k: f32) -> f32 {
    // 5 = 0, ends ≈ +/-7 semitones
    (k.clamp(0.0, 10.0) - 5.0) * 1.4
}

pub fn knob_0_10_to_tune_semi(k: f32) -> f32 {
    (k.clamp(0.0, 10.0) - 5.0) * 0.4 // specs p34: TUNE -2..+2
}

pub fn knob_0_10_to_volume(k: f32) -> f32 {
    (k.clamp(0.0, 10.0) / 10.0).powf(1.5)
}

pub fn knob_0_10_to_lfo_hz(k: f32) -> f32 {
    // Specs p34: LFO 0.05..200 Hz
    let t = k.clamp(0.0, 10.0) / 10.0;
    0.05 * (4000.0f32).powf(t)
}

pub fn knob_0_10_to_bpm(k: f32) -> f32 {
    // Sequencer tempo 40..240 BPM log-ish
    let t = k.clamp(0.0, 10.0) / 10.0;
    40.0 * (6.0f32).powf(t)
}

pub fn note_name(midi: u8) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    format!("{}{}", NAMES[(midi % 12) as usize], (midi as i32 / 12) - 1)
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Runtime {
    /// Currently held MIDI note (None = no gate)
    pub note: Option<u8>,
    /// MIDI velocity 0..1
    pub velocity: f32,
    /// Pitch bend in semitones, -2..+2
    pub bend_semi: f32,
    /// Mod wheel 0..1 (manual: also controls MOD DEPTH)
    pub mod_wheel: f32,
    /// Monophonic gate
    pub gate: bool,
    /// Current sequencer step 0..15 (written by audio thread for UI highlight)
    pub seq_step: u8,
    /// Peak mixer level for OVERLOAD lamp (written by audio thread occasionally)
    pub overload: f32,
    /// Held (sounding) MIDI notes for polyphony. `held_len` valid entries.
    pub held: [u8; 16],
    /// Per-held-note velocity, parallel to `held`.
    pub held_vel: [f32; 16],
    /// Number of valid entries in `held`.
    pub held_len: u8,
    /// Queue of MIDI-learn CC updates drained by the UI each frame.
    pub cc_events: [CcUpdate; 16],
    /// Number of valid entries in `cc_events`.
    pub cc_len: u8,
    /// Monotonic count of every CC message seen (for learn arming).
    pub cc_seq: u32,
    /// Most recent CC number received (for MIDI learn).
    pub last_cc: u8,
}

/// A pending parameter change produced by a MIDI-learn CC.
#[derive(Debug, Clone, Copy, Default)]
pub struct CcUpdate {
    pub param: u8,
    pub value: f32,
}

impl Runtime {
    /// Add a sounding note (last-note priority for the mono `note` field).
    pub fn note_on(&mut self, note: u8, velocity: f32) {
        let velocity = velocity.clamp(0.0, 1.0);
        // Replace an existing entry for the same note (retrigger).
        self.note_off(note);
        let i = self.held_len as usize;
        if i < self.held.len() {
            self.held[i] = note;
            self.held_vel[i] = velocity;
            self.held_len += 1;
        }
        self.note = Some(note);
        self.velocity = velocity;
        self.gate = true;
    }

    /// Remove a sounding note; falls back to the most recent remaining one.
    pub fn note_off(&mut self, note: u8) {
        let n = self.held_len as usize;
        for i in 0..n {
            if self.held[i] == note {
                for j in i..n - 1 {
                    self.held[j] = self.held[j + 1];
                    self.held_vel[j] = self.held_vel[j + 1];
                }
                self.held_len -= 1;
                break;
            }
        }
        let n = self.held_len as usize;
        if n == 0 {
            self.gate = false;
            self.note = None;
        } else {
            self.note = Some(self.held[n - 1]);
            self.velocity = self.held_vel[n - 1];
        }
    }

    /// Clear all held notes (panic).
    pub fn all_notes_off(&mut self) {
        self.held_len = 0;
        self.gate = false;
        self.note = None;
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Shared {
    pub params: Params,
    pub runtime: Runtime,
}

impl Default for Shared {
    fn default() -> Self {
        Self {
            params: Params::default(),
            runtime: Runtime::default(),
        }
    }
}

pub fn midi_note_to_freq(note: u8) -> f32 {
    440.0 * 2.0f32.powf((note as f32 - 69.0) / 12.0)
}
