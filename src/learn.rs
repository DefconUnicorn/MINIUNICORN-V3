//! MIDI-learn parameter registry.
//!
//! Maps a stable numeric id (the [`P`] discriminant) to a field of
//! [`Params`], so a CC binding can read and write the live value without
//! the UI having to thread an id through every widget call. Ids are
//! persisted inside [`crate::params::CcBinding`]; **never renumber an
//! existing variant** — only append.

use crate::params::Params;

/// A learnable parameter. Discriminants start at 1 (0 means "unbound").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum P {
    // Continuous knobs (0..10 unless noted)
    Tune = 1,
    Glide,
    ModMix,
    ModDepth,
    LfoRate,
    Osc1Level,
    Osc2Level,
    Osc3Level,
    NoiseLevel,
    ExtLevel,
    Cutoff,
    Resonance,
    Contour,
    FiltAttack,
    FiltDecay,
    FiltSustain,
    AmpAttack,
    AmpDecay,
    AmpSustain,
    Volume,
    Osc2Freq,
    Osc3Freq,
    Tempo,
    // Toggles (CC >= 64 engages)
    Osc1On,
    Osc2On,
    Osc3On,
    NoiseOn,
    ExtOn,
    NoisePink,
    FiltMod,
    FiltModeHi,
    KbCtrl1,
    KbCtrl2,
    FiltDecayOn,
    LoudDecayOn,
    LfoShape,
    OscMod,
    Osc3Ctrl,
    MainOn,
    A440,
    Power,
    SeqPlay,
}

impl P {
    /// Every parameter, in menu order.
    pub const ALL: &'static [P] = &[
        P::Tune,
        P::Glide,
        P::ModMix,
        P::ModDepth,
        P::LfoRate,
        P::Osc1Level,
        P::Osc2Level,
        P::Osc3Level,
        P::NoiseLevel,
        P::ExtLevel,
        P::Cutoff,
        P::Resonance,
        P::Contour,
        P::FiltAttack,
        P::FiltDecay,
        P::FiltSustain,
        P::AmpAttack,
        P::AmpDecay,
        P::AmpSustain,
        P::Volume,
        P::Osc2Freq,
        P::Osc3Freq,
        P::Tempo,
        P::Osc1On,
        P::Osc2On,
        P::Osc3On,
        P::NoiseOn,
        P::ExtOn,
        P::NoisePink,
        P::FiltMod,
        P::FiltModeHi,
        P::KbCtrl1,
        P::KbCtrl2,
        P::FiltDecayOn,
        P::LoudDecayOn,
        P::LfoShape,
        P::OscMod,
        P::Osc3Ctrl,
        P::MainOn,
        P::A440,
        P::Power,
        P::SeqPlay,
    ];

    pub fn id(self) -> u8 {
        self as u8
    }

    pub fn from_id(id: u8) -> Option<P> {
        P::ALL.iter().copied().find(|p| p.id() == id)
    }

    /// Panel section header for grouping in the learn window.
    pub fn section(self) -> &'static str {
        match self {
            P::Tune | P::Glide | P::ModMix | P::ModDepth | P::LfoRate => "CONTROLLERS",
            P::Osc1Level
            | P::Osc2Level
            | P::Osc3Level
            | P::NoiseLevel
            | P::ExtLevel
            | P::Osc1On
            | P::Osc2On
            | P::Osc3On
            | P::NoiseOn
            | P::ExtOn
            | P::NoisePink
            | P::Osc2Freq
            | P::Osc3Freq
            | P::Osc3Ctrl => "OSCILLATORS & MIXER",
            P::Cutoff
            | P::Resonance
            | P::Contour
            | P::FiltAttack
            | P::FiltDecay
            | P::FiltSustain
            | P::FiltMod
            | P::FiltModeHi
            | P::KbCtrl1
            | P::KbCtrl2
            | P::FiltDecayOn => "FILTER",
            P::AmpAttack | P::AmpDecay | P::AmpSustain | P::LoudDecayOn => "LOUDNESS",
            P::Volume | P::MainOn | P::A440 | P::Power => "OUTPUT",
            P::Tempo | P::SeqPlay => "SEQUENCER",
            P::LfoShape | P::OscMod => "CONTROLLERS",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            P::Tune => "TUNE",
            P::Glide => "GLIDE",
            P::ModMix => "MOD MIX",
            P::ModDepth => "MOD DEPTH",
            P::LfoRate => "LFO RATE",
            P::Osc1Level => "OSC 1 LEVEL",
            P::Osc2Level => "OSC 2 LEVEL",
            P::Osc3Level => "OSC 3 LEVEL",
            P::NoiseLevel => "NOISE LEVEL",
            P::ExtLevel => "EXT LEVEL",
            P::Cutoff => "CUTOFF",
            P::Resonance => "RESONANCE",
            P::Contour => "CONTOUR",
            P::FiltAttack => "FILTER ATTACK",
            P::FiltDecay => "FILTER DECAY",
            P::FiltSustain => "FILTER SUSTAIN",
            P::AmpAttack => "LOUD ATTACK",
            P::AmpDecay => "LOUD DECAY",
            P::AmpSustain => "LOUD SUSTAIN",
            P::Volume => "VOLUME",
            P::Osc2Freq => "OSC 2 FREQ",
            P::Osc3Freq => "OSC 3 FREQ",
            P::Tempo => "TEMPO",
            P::Osc1On => "OSC 1 ON",
            P::Osc2On => "OSC 2 ON",
            P::Osc3On => "OSC 3 ON",
            P::NoiseOn => "NOISE ON",
            P::ExtOn => "EXT ON",
            P::NoisePink => "NOISE PINK",
            P::FiltMod => "FILTER MOD",
            P::FiltModeHi => "FILTER HI",
            P::KbCtrl1 => "KB CTRL 1",
            P::KbCtrl2 => "KB CTRL 2",
            P::FiltDecayOn => "FILTER DECAY ON",
            P::LoudDecayOn => "LOUD DECAY ON",
            P::LfoShape => "LFO SHAPE",
            P::OscMod => "OSC MOD",
            P::Osc3Ctrl => "OSC 3 CTRL",
            P::MainOn => "MAIN OUT",
            P::A440 => "A-440",
            P::Power => "POWER",
            P::SeqPlay => "SEQ PLAY",
        }
    }

    /// True for on/off parameters (CC >= 64 engages).
    pub fn is_toggle(self) -> bool {
        matches!(
            self,
            P::Osc1On
                | P::Osc2On
                | P::Osc3On
                | P::NoiseOn
                | P::ExtOn
                | P::NoisePink
                | P::FiltMod
                | P::FiltModeHi
                | P::KbCtrl1
                | P::KbCtrl2
                | P::FiltDecayOn
                | P::LoudDecayOn
                | P::LfoShape
                | P::OscMod
                | P::Osc3Ctrl
                | P::MainOn
                | P::A440
                | P::Power
                | P::SeqPlay
        )
    }
}

/// Read the current value of a learnable parameter (0..10, or 0/10 for
/// toggles). Returns `None` for an unknown id.
pub fn get(p: &Params, id: u8) -> Option<f32> {
    let v = match P::from_id(id)? {
        P::Tune => p.tune_0_10,
        P::Glide => p.glide_0_10,
        P::ModMix => p.mod_mix_0_10,
        P::ModDepth => p.mod_depth_0_10,
        P::LfoRate => p.lfo_rate_0_10,
        P::Osc1Level => p.osc1_level_0_10,
        P::Osc2Level => p.osc2_level_0_10,
        P::Osc3Level => p.osc3_level_0_10,
        P::NoiseLevel => p.noise_level_0_10,
        P::ExtLevel => p.ext_level_0_10,
        P::Cutoff => p.cutoff_0_10,
        P::Resonance => p.reso_0_10,
        P::Contour => p.contour_amt_0_10,
        P::FiltAttack => p.filt_attack_0_10,
        P::FiltDecay => p.filt_decay_0_10,
        P::FiltSustain => p.filt_sustain_0_10,
        P::AmpAttack => p.amp_attack_0_10,
        P::AmpDecay => p.amp_decay_0_10,
        P::AmpSustain => p.amp_sustain_0_10,
        P::Volume => p.volume_0_10,
        P::Osc2Freq => p.osc2_freq_0_10,
        P::Osc3Freq => p.osc3_freq_0_10,
        P::Tempo => p.seq_bpm_0_10,
        P::Osc1On => bool_val(p.osc1_on),
        P::Osc2On => bool_val(p.osc2_on),
        P::Osc3On => bool_val(p.osc3_on),
        P::NoiseOn => bool_val(p.noise_on),
        P::ExtOn => bool_val(p.ext_on),
        P::NoisePink => bool_val(p.noise_pink),
        P::FiltMod => bool_val(p.filter_mod_on),
        P::FiltModeHi => bool_val(p.filter_mode_hi),
        P::KbCtrl1 => bool_val(p.kb_control_1),
        P::KbCtrl2 => bool_val(p.kb_control_2),
        P::FiltDecayOn => bool_val(p.filter_decay_on),
        P::LoudDecayOn => bool_val(p.loud_decay_on),
        P::LfoShape => bool_val(p.lfo_square),
        P::OscMod => bool_val(p.osc_mod_on),
        P::Osc3Ctrl => bool_val(p.osc3_kb_ctrl),
        P::MainOn => bool_val(p.main_on),
        P::A440 => bool_val(p.a440_on),
        P::Power => bool_val(p.power_on),
        P::SeqPlay => bool_val(p.seq_on),
    };
    Some(v)
}

/// Apply a MIDI-learn value (0..10) to a parameter. Toggles engage at the
/// midpoint. Unknown ids are ignored.
pub fn set(p: &mut Params, id: u8, value: f32) {
    let toggle = value >= 5.0;
    match P::from_id(id) {
        Some(P::Tune) => p.tune_0_10 = value.clamp(0.0, 10.0),
        Some(P::Glide) => p.glide_0_10 = value.clamp(0.0, 10.0),
        Some(P::ModMix) => p.mod_mix_0_10 = value.clamp(0.0, 10.0),
        Some(P::ModDepth) => p.mod_depth_0_10 = value.clamp(0.0, 10.0),
        Some(P::LfoRate) => p.lfo_rate_0_10 = value.clamp(0.0, 10.0),
        Some(P::Osc1Level) => p.osc1_level_0_10 = value.clamp(0.0, 10.0),
        Some(P::Osc2Level) => p.osc2_level_0_10 = value.clamp(0.0, 10.0),
        Some(P::Osc3Level) => p.osc3_level_0_10 = value.clamp(0.0, 10.0),
        Some(P::NoiseLevel) => p.noise_level_0_10 = value.clamp(0.0, 10.0),
        Some(P::ExtLevel) => p.ext_level_0_10 = value.clamp(0.0, 10.0),
        Some(P::Cutoff) => p.cutoff_0_10 = value.clamp(0.0, 10.0),
        Some(P::Resonance) => p.reso_0_10 = value.clamp(0.0, 10.0),
        Some(P::Contour) => p.contour_amt_0_10 = value.clamp(0.0, 10.0),
        Some(P::FiltAttack) => p.filt_attack_0_10 = value.clamp(0.0, 10.0),
        Some(P::FiltDecay) => p.filt_decay_0_10 = value.clamp(0.0, 10.0),
        Some(P::FiltSustain) => p.filt_sustain_0_10 = value.clamp(0.0, 10.0),
        Some(P::AmpAttack) => p.amp_attack_0_10 = value.clamp(0.0, 10.0),
        Some(P::AmpDecay) => p.amp_decay_0_10 = value.clamp(0.0, 10.0),
        Some(P::AmpSustain) => p.amp_sustain_0_10 = value.clamp(0.0, 10.0),
        Some(P::Volume) => p.volume_0_10 = value.clamp(0.0, 10.0),
        Some(P::Osc2Freq) => p.osc2_freq_0_10 = value.clamp(0.0, 10.0),
        Some(P::Osc3Freq) => p.osc3_freq_0_10 = value.clamp(0.0, 10.0),
        Some(P::Tempo) => p.seq_bpm_0_10 = value.clamp(0.0, 10.0),
        Some(P::Osc1On) => p.osc1_on = toggle,
        Some(P::Osc2On) => p.osc2_on = toggle,
        Some(P::Osc3On) => p.osc3_on = toggle,
        Some(P::NoiseOn) => p.noise_on = toggle,
        Some(P::ExtOn) => p.ext_on = toggle,
        Some(P::NoisePink) => p.noise_pink = toggle,
        Some(P::FiltMod) => p.filter_mod_on = toggle,
        Some(P::FiltModeHi) => p.filter_mode_hi = toggle,
        Some(P::KbCtrl1) => p.kb_control_1 = toggle,
        Some(P::KbCtrl2) => p.kb_control_2 = toggle,
        Some(P::FiltDecayOn) => p.filter_decay_on = toggle,
        Some(P::LoudDecayOn) => p.loud_decay_on = toggle,
        Some(P::LfoShape) => p.lfo_square = toggle,
        Some(P::OscMod) => p.osc_mod_on = toggle,
        Some(P::Osc3Ctrl) => p.osc3_kb_ctrl = toggle,
        Some(P::MainOn) => p.main_on = toggle,
        Some(P::A440) => p.a440_on = toggle,
        Some(P::Power) => p.power_on = toggle,
        Some(P::SeqPlay) => p.seq_on = toggle,
        None => {}
    }
}

/// Find the CC currently bound to a parameter, if any.
pub fn cc_for(p: &Params, id: u8) -> Option<u8> {
    p.midi_cc
        .iter()
        .find(|b| b.param == id && b.param != 0)
        .map(|b| b.cc)
}

/// Find the parameter bound to a CC, if any.
pub fn param_for_cc(p: &Params, cc: u8) -> Option<u8> {
    p.midi_cc
        .iter()
        .find(|b| b.cc == cc && b.param != 0)
        .map(|b| b.param)
}

/// Bind `cc` to parameter `id`, replacing any existing binding for either
/// the CC or the parameter. Returns `false` if the table is full.
pub fn bind(p: &mut Params, cc: u8, id: u8) -> bool {
    for b in p.midi_cc.iter_mut() {
        if b.cc == cc || b.param == id {
            b.cc = 0;
            b.param = 0;
        }
    }
    for b in p.midi_cc.iter_mut() {
        if b.param == 0 {
            b.cc = cc;
            b.param = id;
            return true;
        }
    }
    false
}

/// Remove any binding for `id`.
pub fn unbind(p: &mut Params, id: u8) {
    for b in p.midi_cc.iter_mut() {
        if b.param == id {
            b.cc = 0;
            b.param = 0;
        }
    }
}

/// Remove every binding.
pub fn clear_all(p: &mut Params) {
    for b in p.midi_cc.iter_mut() {
        b.cc = 0;
        b.param = 0;
    }
}

fn bool_val(b: bool) -> f32 {
    if b {
        10.0
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_roundtrip() {
        let mut seen = std::collections::HashSet::new();
        for &p in P::ALL {
            assert!(p.id() != 0, "id 0 is reserved for unbound");
            assert!(seen.insert(p.id()), "duplicate id {}", p.id());
            assert_eq!(P::from_id(p.id()), Some(p));
        }
    }

    #[test]
    fn get_set_roundtrip() {
        let mut p = Params::default();
        set(&mut p, P::Cutoff.id(), 7.5);
        assert_eq!(get(&p, P::Cutoff.id()), Some(7.5));
        set(&mut p, P::Osc1On.id(), 0.0);
        assert_eq!(p.osc1_on, false);
        set(&mut p, P::Osc1On.id(), 10.0);
        assert_eq!(p.osc1_on, true);
    }

    #[test]
    fn binding_replaces_conflicts() {
        let mut p = Params::default();
        assert!(bind(&mut p, 74, P::Cutoff.id()));
        assert!(bind(&mut p, 71, P::Resonance.id()));
        assert_eq!(cc_for(&p, P::Resonance.id()), Some(71));
        // Rebinding the same param to a new CC frees the old CC.
        assert!(bind(&mut p, 72, P::Resonance.id()));
        assert_eq!(cc_for(&p, P::Resonance.id()), Some(72));
        assert_eq!(param_for_cc(&p, 71), None);
        // Rebinding a used CC steals it from its previous param.
        assert!(bind(&mut p, 74, P::Volume.id()));
        assert_eq!(param_for_cc(&p, 74), Some(P::Volume.id()));
        assert_eq!(cc_for(&p, P::Cutoff.id()), None);
    }
}
