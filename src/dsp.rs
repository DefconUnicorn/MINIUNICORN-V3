//! Mono Model-D-style DSP core.
//!
//! - 3x PolyBLEP oscillators (triangle / saw / square) with drift + range + LO mode
//! - White/pink noise
//! - Saturating mixer (tanh) with OVERLOAD level
//! - Huovilainen-inspired nonlinear transistor ladder filter, 2x oversampled
//! - Separate FILTER CONTOUR and LOUDNESS CONTOUR ADS+R, exponential glide
//!
//! Dependency-free, no allocation in `render()`.

use crate::params::{
    self, midi_note_to_freq, OscWave, Params, Runtime,
};
use std::f32::consts::PI;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn clamp01(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

fn poly_blep(phase: f32, dt: f32) -> f32 {
    if phase < dt {
        let t = phase / dt;
        2.0 * t - t * t - 1.0
    } else if phase > 1.0 - dt {
        let t = (phase - 1.0) / dt;
        t * t + 2.0 * t + 1.0
    } else {
        0.0
    }
}

fn pulse_sample(phase: f32, dt: f32, pw: f32) -> f32 {
    let pw = pw.clamp(0.03, 0.97);
    let mut naive = if phase < pw { 1.0 } else { -1.0 };
    naive += poly_blep(phase, dt);
    let phase2 = (phase + 1.0 - pw) % 1.0;
    naive -= poly_blep(phase2, dt);
    naive * 0.9
}

fn tri_sample(phase: f32) -> f32 {
    4.0 * (phase - (phase + 0.25).floor() - 0.25).abs() - 1.0
}

fn osc_sample(wave: OscWave, phase: f32, dt: f32) -> f32 {
    match wave {
        OscWave::Triangle => tri_sample(phase),
        OscWave::TriSaw => {
            // Manual §2: triangular/saw blend (OSC 1+2)
            let saw = 2.0 * phase - 1.0 - poly_blep(phase, dt);
            0.5 * (tri_sample(phase) + saw)
        }
        OscWave::Saw => {
            let naive = 2.0 * phase - 1.0;
            naive - poly_blep(phase, dt)
        }
        OscWave::ReverseSaw => {
            // Manual §2: reverse saw (OSC 3)
            let naive = 1.0 - 2.0 * phase;
            naive + poly_blep(phase, dt)
        }
        OscWave::Square => pulse_sample(phase, dt, 0.5),
        OscWave::WidePulse => pulse_sample(phase, dt, 0.3),
        OscWave::NarrowPulse => pulse_sample(phase, dt, 0.1),
    }
}

// ---------------------------------------------------------------------------
// ADS+R envelope
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EnvStage {
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}

#[derive(Debug, Clone)]
pub struct AdsEnv {
    stage: EnvStage,
    level: f32,
    sample_rate: f32,
}

impl AdsEnv {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            stage: EnvStage::Idle,
            level: 0.0,
            sample_rate,
        }
    }

    pub fn set_sample_rate(&mut self, sr: f32) {
        self.sample_rate = sr;
    }

    pub fn gate_on(&mut self) {
        self.stage = EnvStage::Attack;
    }

    pub fn gate_off(&mut self) {
        if self.stage != EnvStage::Idle {
            self.stage = EnvStage::Release;
        }
    }

    fn coeff(&self, secs: f32) -> f32 {
        let t = secs.max(0.001);
        (-1.0 / (t * self.sample_rate)).exp()
    }

    pub fn process(&mut self, attack: f32, decay: f32, sustain: f32, release: f32) -> f32 {
        let a = attack.max(0.001);
        let d = decay.max(0.005);
        let s = clamp01(sustain);
        let r = release.max(0.005);

        match self.stage {
            EnvStage::Idle => self.level = 0.0,
            EnvStage::Attack => {
                let step = 1.0 / (a * self.sample_rate);
                self.level += step;
                if self.level >= 1.0 {
                    self.level = 1.0;
                    self.stage = EnvStage::Decay;
                }
            }
            EnvStage::Decay => {
                let c = self.coeff(d);
                self.level = s + (self.level - s) * c;
                if (self.level - s).abs() < 0.0008 {
                    self.level = s;
                    self.stage = EnvStage::Sustain;
                }
            }
            EnvStage::Sustain => self.level = s,
            EnvStage::Release => {
                let c = self.coeff(r);
                self.level *= c;
                if self.level < 0.0004 {
                    self.level = 0.0;
                    self.stage = EnvStage::Idle;
                }
            }
        }
        self.level
    }

    pub fn is_active(&self) -> bool {
        self.stage != EnvStage::Idle
    }
}

// ---------------------------------------------------------------------------
// Nonlinear ladder filter (Huovilainen-inspired)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LadderFilter {
    s: [f32; 4],
    g: f32,
    k: f32,
    sample_rate_os: f32,
}

impl LadderFilter {
    pub fn new(sample_rate_os: f32) -> Self {
        Self {
            s: [0.0; 4],
            g: 0.1,
            k: 1.0,
            sample_rate_os,
        }
    }

    pub fn set_sample_rate_os(&mut self, sr_os: f32) {
        self.sample_rate_os = sr_os;
    }

    fn update_coeffs(&mut self, cutoff: f32, resonance: f32) {
        let fc = cutoff.clamp(20.0, 18000.0);
        let g = 1.0 - (-2.0 * PI * fc / self.sample_rate_os).exp();
        self.g = g.clamp(0.01, 0.9);
        self.k = (clamp01(resonance) * 4.0).clamp(0.0, 3.95);
    }

    #[inline]
    pub fn process_one_os(&mut self, x: f32) -> f32 {
        let g = self.g;
        let k = self.k;
        let u = (x - k * self.s[3]).tanh();
        self.s[0] += g * (u - self.s[0].tanh());
        self.s[1] += g * (self.s[0].tanh() - self.s[1].tanh());
        self.s[2] += g * (self.s[1].tanh() - self.s[2].tanh());
        self.s[3] += g * (self.s[2].tanh() - self.s[3].tanh());
        self.s[3]
    }

    pub fn process(&mut self, x: f32, cutoff: f32, resonance: f32) -> f32 {
        self.update_coeffs(cutoff, resonance);
        let y1 = self.process_one_os(x);
        let y2 = self.process_one_os(x);
        // Resonance feedback reduces the passband gain by 1/(1+k); compensate
        // so EMPHASIS adds the resonant peak without dropping the level.
        0.5 * (y1 + y2) * (1.0 + self.k)
    }
}

// Simple deterministic white noise (xorshift) + pink-ish lowpassed version
#[derive(Debug, Clone)]
struct NoiseGen {
    state: u32,
    pink_b: [f32; 6],
}

impl NoiseGen {
    fn new() -> Self {
        Self {
            state: 0x12345678,
            pink_b: [0.0; 6],
        }
    }
    fn white(&mut self) -> f32 {
        // xorshift32 -> -1..1
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
    fn pink(&mut self) -> f32 {
        // Paul Kellet-ish pink approximation
        let w = self.white();
        let b = &mut self.pink_b;
        b[0] = 0.99886 * b[0] + w * 0.0555179;
        b[1] = 0.99332 * b[1] + w * 0.0750759;
        b[2] = 0.96900 * b[2] + w * 0.1538520;
        b[3] = 0.86650 * b[3] + w * 0.3104856;
        b[4] = 0.55000 * b[4] + w * 0.5329522;
        b[5] = -0.7616 * b[5] - w * 0.0168980;
        (b[0] + b[1] + b[2] + b[3] + b[4] + b[5] + b[5] + w * 0.5362) * 0.11
    }
}

// ---------------------------------------------------------------------------
// Mono voice
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Voice {
    sample_rate: f32,
    phase: [f32; 3],
    drift_phase: [f32; 3],
    glide_freq: f32,
    last_gate: bool,
    filter: LadderFilter,
    amp_env: AdsEnv,
    filt_env: AdsEnv,
    smooth_cutoff: f32,
    noise: NoiseGen,
    lfo_phase: f32,
    mod_noise_hold: f32,
    prev_out: f32,
    a440_phase: f32,
    seq_pos: usize,
    seq_counter: u32,
    pub last_mix_peak: f32,
    pub last_seq_step: u8,
}

impl Voice {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            phase: [0.0, 0.33, 0.66],
            drift_phase: [0.0, 0.37, 0.71],
            glide_freq: 55.0,
            last_gate: false,
            filter: LadderFilter::new(sample_rate * 2.0),
            amp_env: AdsEnv::new(sample_rate),
            filt_env: AdsEnv::new(sample_rate),
            smooth_cutoff: 1200.0,
            noise: NoiseGen::new(),
            lfo_phase: 0.0,
            mod_noise_hold: 0.0,
            prev_out: 0.0,
            a440_phase: 0.0,
            seq_pos: 0,
            seq_counter: 0,
            last_mix_peak: 0.0,
            last_seq_step: 0,
        }
    }

    pub fn set_sample_rate(&mut self, sr: f32) {
        self.sample_rate = sr;
        self.filter.set_sample_rate_os(sr * 2.0);
        self.amp_env.set_sample_rate(sr);
        self.filt_env.set_sample_rate(sr);
    }

    fn glide_coeff(&self, secs: f32) -> f32 {
        if secs <= 0.0005 {
            return 1.0;
        }
        (-1.0 / (secs * self.sample_rate)).exp()
    }

    pub fn render(&mut self, p: &Params, r: &Runtime) -> f32 {
        if !p.power_on {
            return 0.0;
        }
        // The phones jack is an internal post-output bus. It is deliberately
        // independent of MAIN volume, and can be patched back into inputs.
        let phone_routes = p.phones_route_mask;
        let phone_bus = (self.prev_out * params::knob_0_10_to_volume(p.phones_0_10))
            .clamp(-1.2, 1.2);
        let phones_to = |index: usize| {
            p.jack_routes[index] == 6
                || (p.jack_routes[index] == 0 && phone_routes & (1 << index) != 0)
        };
        let phone_loud_gain = if p.jack_routes[7] == 0 && phones_to(7) {
            (1.0 + phone_bus * 0.5).clamp(0.0, 2.0)
        } else {
            1.0
        };
        // --- 16-step sequencer overrides keys when ON ---
        // 16th notes at seq BPM. Rest (note 0) = gate off.
        let mut seq_note: Option<u8> = None;
        let mut seq_gate = false;
        if p.seq_on {
            let bpm = params::knob_0_10_to_bpm(p.seq_bpm_0_10).clamp(30.0, 300.0);
            let step_samples = ((self.sample_rate * 60.0 / bpm / 4.0) as u32).max(64);
            self.seq_counter += 1;
            if self.seq_counter >= step_samples {
                self.seq_counter = 0;
                self.seq_pos = (self.seq_pos + 1) % 16;
            }
            let n = p.seq_notes[self.seq_pos];
            if n > 0 {
                seq_note = Some(n);
                seq_gate = true;
            }
            self.last_seq_step = self.seq_pos as u8;
        } else {
            self.seq_pos = 0;
            self.seq_counter = 0;
            self.last_seq_step = 0;
        }
        let eff_note = seq_note.or(r.note);
        let phone_gate = (phones_to(5) || phones_to(6))
            && phone_bus.abs() > 0.05;
        let eff_gate = (if p.seq_on { seq_gate } else { r.gate }) || phone_gate;
        if eff_gate && !self.last_gate {
            self.amp_env.gate_on();
            self.filt_env.gate_on();
            // Reset oscillator phases on note-on to avoid clicks on pitch changes.
            self.phase = [0.0, 0.33, 0.66];
        } else if !eff_gate && self.last_gate {
            self.amp_env.gate_off();
            self.filt_env.gate_off();
        }
        let note_on = eff_gate && !self.last_gate;
        self.last_gate = eff_gate;

        // --- note + tune (A-440 no longer hijacks the voice; it is a
        // separate solid reference drone mixed at the output below) ---
        let note = eff_note.unwrap_or(48) as f32;
        let mut target = midi_note_to_freq(note as u8);
        let master_tune = params::knob_0_10_to_tune_semi(p.tune_0_10);
        target *= 2.0f32.powf((master_tune + r.bend_semi) / 12.0);

        let glide = params::knob_0_10_to_glide(p.glide_0_10);
        let c = self.glide_coeff(glide);
        if c >= 1.0 {
            self.glide_freq = target;
        } else {
            self.glide_freq += (target - self.glide_freq) * (1.0 - c);
        }
        let mut base = self.glide_freq.max(20.0).min(4000.0);

        // --- Envelopes first (FILTER EG can be a mod source, manual §4.8) ---
        // v2: RELEASE knob removed — fixed middle value (≈25 ms).
        let rel_knob = params::knob_0_10_to_decay(2.0);
        let fa = params::knob_0_10_to_attack(p.filt_attack_0_10);
        let fd = params::knob_0_10_to_decay(p.filt_decay_0_10);
        let fs = p.filt_sustain_0_10 / 10.0;
        let aa = params::knob_0_10_to_attack(p.amp_attack_0_10);
        let ad = params::knob_0_10_to_decay(p.amp_decay_0_10);
        let asus = p.amp_sustain_0_10 / 10.0;
        // Manual (30): DECAY ON = release takes DECAY time; OFF = immediate.
        // Fixed RELEASE trim extends release when switch is ON.
        let rel_filt = if p.filter_decay_on {
            fd.max(rel_knob * 0.5)
        } else {
            0.015
        };
        let rel_amp = if p.loud_decay_on {
            ad.max(rel_knob * 0.5)
        } else {
            0.015
        };
        // Peek envelopes without advancing twice: advance once here, reuse
        // values below (filter contour needed as mod source).
        // NOTE: AdsEnv advances per call; call once per sample here.
        let f_env_peek = self.filt_env.process(fa, fd, fs, rel_filt);
        let a_env_peek = self.amp_env.process(aa, ad, asus, rel_amp);
        let f_env = f_env_peek;
        let a_env = a_env_peek;

        // --- Modulation bus (manual §3.1.2, §4.8) ---
        // Source A (4): OSC3 audio vs Filter Envelope.
        // Source B (9): Noise (or ext) vs dedicated LFO (11: tri/square, 12: rate).
        // (8) MOD MIX blends A <> B. (10) MOD DEPTH (+ mod wheel) = amount.
        // (7) OSC MOD routes bus to osc pitch; (32) FILTER MOD routes to filter.
        let sr_pre = self.sample_rate;
        let lfo_hz = params::knob_0_10_to_lfo_hz(p.lfo_rate_0_10).clamp(0.02, 300.0);
        self.lfo_phase += lfo_hz / sr_pre;
        if self.lfo_phase >= 1.0 {
            self.lfo_phase -= 1.0;
        }
        let lfo = if p.lfo_square {
            if self.lfo_phase < 0.5 { 1.0 } else { -1.0 }
        } else {
            // triangle from phase
            if self.lfo_phase < 0.5 {
                4.0 * self.lfo_phase - 1.0
            } else {
                3.0 - 4.0 * self.lfo_phase
            }
        };
        // smooth noise source so vibrato isn't harsh FM
        self.mod_noise_hold = self.mod_noise_hold * 0.998 + self.noise.white() * 0.002;
        // OSC3 audio sample for source A: compute its phase now (without
        // pitch-mod on itself to avoid feedback; manual modulates all three
        // but self-FM at audio rate is unstable in naive DSP).
        let fine3 = params::knob_0_10_to_fine_semi(p.osc3_freq_0_10);
        let range3 = p.osc3_range_oct as f32 * 12.0;
        let mut f3 = base * 2.0f32.powf((range3 + fine3) / 12.0);
        // RANGE = LO (-3): sub-audio drone. Without keyboard control it is a
        // free drone (~6 Hz); there is no separate LO switch on the hardware.
        if p.osc3_range_oct <= -3 {
            f3 = if p.osc3_kb_ctrl {
                (base * 2.0f32.powf((range3 + fine3) / 12.0)).max(0.3)
            } else {
                6.0 * 2.0f32.powf(fine3 / 12.0)
            };
        }
        let dt3 = (f3 / sr_pre).clamp(0.00005, 0.25);
        // advance osc3 phase + drift (drift applied in main loop too; keep
        // simple by advancing here and reusing sample)
        // v2: DRIFT knob removed — fixed gentle amount (was 1.5/10).
        let drift_amt_pre = 0.15;
        self.drift_phase[2] += 0.5 / sr_pre;
        if self.drift_phase[2] >= 1.0 {
            self.drift_phase[2] -= 1.0;
        }
        let wob3 = (2.0 * PI * self.drift_phase[2]).sin();
        let f3d = f3 * 2.0f32.powf(wob3 * 4.0 * drift_amt_pre / 1200.0);
        let dt3d = (f3d / sr_pre).clamp(0.00005, 0.25);
        self.phase[2] += dt3d;
        if self.phase[2] >= 1.0 {
            self.phase[2] -= 1.0;
        }
        let osc3_sample = osc_sample(p.osc3_wave, self.phase[2], dt3d);
        let _ = (dt3, f3);

        let src_a = if p.mod_src_a_is_filter_eg {
            f_env * 2.0 - 1.0 // bipolar envelope
        } else {
            osc3_sample
        };
        let src_b = if p.mod_src_b_is_lfo {
            lfo
        } else {
            self.mod_noise_hold.clamp(-1.0, 1.0) * 3.0
        };
        let mix_t = (p.mod_mix_0_10 / 10.0).clamp(0.0, 1.0);
        let mod_bus = src_a * (1.0 - mix_t) + src_b * mix_t;
        let lfo_saw = self.lfo_phase * 2.0 - 1.0;
        let lfo_sqr = if self.lfo_phase < 0.5 { 1.0 } else { -1.0 };
        let route_signal = |code: u8, mix_signal: f32| match code {
            1 => lfo_saw,
            2 => lfo_sqr,
            3 => mix_signal,
            4 => f_env * 2.0 - 1.0,
            5 => a_env,
            6 => phone_bus,
            7 => self.prev_out,
            _ => 0.0,
        };
        let loud_route_gain = if p.jack_routes[7] != 0 {
            (1.0 + route_signal(p.jack_routes[7], 0.0) * 0.5).clamp(0.0, 2.0)
        } else {
            1.0
        };
        let depth_total = ((p.mod_depth_0_10 / 10.0) + r.mod_wheel).clamp(0.0, 1.5);
        let mut mod_cents = 0.0;
        let mut mod_filter_oct = 0.0;
        if p.osc_mod_on && depth_total > 0.001 {
            // Manual (7): oscillators modulated by mix. ±80 cents max.
            mod_cents = mod_bus * depth_total * 80.0;
        }
        if p.filter_mod_on && depth_total > 0.001 {
            // Manual (32): filter modulated by mix. ±2 oct max.
            mod_filter_oct = mod_bus * depth_total * 2.0;
        }
        // OSC 1 voltage-control input (or PHONES fed to it) shifts the
        // oscillator bank by up to ±1 octave. Applied to `base` (the bank
        // reference) — previously this multiplied `target` after the glide
        // value had already been taken, so the routing had no audible effect.
        if p.jack_routes[1] != 0 {
            base *= 2.0f32.powf(route_signal(p.jack_routes[1], 0.0));
        } else if phones_to(1) {
            base *= 2.0f32.powf(phone_bus);
        }
        base *= 2.0f32.powf(mod_cents / 1200.0);
        let base = base.max(20.0).min(4000.0);

        let fine = [
            0.0, // OSC 1 is the reference: no detune knob on the hardware
            params::knob_0_10_to_fine_semi(p.osc2_freq_0_10),
            params::knob_0_10_to_fine_semi(p.osc3_freq_0_10),
        ];
        let ranges = [
            p.osc1_range_oct as f32 * 12.0,
            p.osc2_range_oct as f32 * 12.0,
            p.osc3_range_oct as f32 * 12.0,
        ];
        let waves = [p.osc1_wave, p.osc2_wave, p.osc3_wave];
        let ons = [p.osc1_on, p.osc2_on, p.osc3_on];
        let levels = [
            params::knob_0_10_to_level(p.osc1_level_0_10),
            params::knob_0_10_to_level(p.osc2_level_0_10),
            params::knob_0_10_to_level(p.osc3_level_0_10),
        ];

        let sr = self.sample_rate;
        // v2: DRIFT knob removed — fixed gentle amount (was 1.5/10).
        let drift_amt = 0.15;
        let mut mix = 0.0;
        for i in 0..2 {
            if !ons[i] {
                continue;
            }
            let f = base * 2.0f32.powf((ranges[i] + fine[i]) / 12.0);
            self.drift_phase[i] += 0.5 / sr;
            if self.drift_phase[i] >= 1.0 {
                self.drift_phase[i] -= 1.0;
            }
            let wob = (2.0 * PI * self.drift_phase[i]).sin()
                + 0.5 * (2.0 * PI * self.drift_phase[i] * 2.7 + 1.3).sin();
            let cents = wob * 4.0 * drift_amt;
            let f_d = f * 2.0f32.powf(cents / 1200.0);

            let dt = (f_d / sr).clamp(0.00005, 0.25);
            self.phase[i] += dt;
            if self.phase[i] >= 1.0 {
                self.phase[i] -= 1.0;
            }
            mix += osc_sample(waves[i], self.phase[i], dt) * levels[i];
        }
        // OSC3: reuse sample computed for mod bus (phase already advanced).
        if p.osc3_on {
            mix += osc3_sample * levels[2];
        }

        // Noise (22)
        if p.noise_on {
                let n = if p.noise_pink {
                    self.noise.pink() * 2.0
                } else {
                    self.noise.white()
                };
            mix += n * params::knob_0_10_to_level(p.noise_level_0_10) * 0.9;
        }

        // EXT feedback (25): nothing patched -> main out feeds back for
        // extra bass/crunch. Gain <1 keeps it stable through the tanh.
        // The manual's EXT selector chooses external input when ON; with no
        // external source selected, the main output feeds back into EXT.
        let ext_has_patch_source = p.ext_on && p.jack_routes[3] != 0;
        if !ext_has_patch_source {
            let fb = (self.prev_out
                * params::knob_0_10_to_level(p.ext_level_0_10)
                * 0.85)
                .clamp(-1.0, 1.0);
            mix += fb;
        }
        if p.ext_on && p.jack_routes[3] != 0 {
            mix += route_signal(p.jack_routes[3], mix)
                * params::knob_0_10_to_level(p.ext_level_0_10)
                * 0.85;
        } else if p.ext_on && phones_to(3) {
            mix += phone_bus * params::knob_0_10_to_level(p.ext_level_0_10) * 0.85;
        }

        // Mixer saturation + overload tracking (24)
        let pre = mix * 0.55 * 1.8;
        self.last_mix_peak = self.last_mix_peak * 0.999 + pre.abs() * 0.001;
        let driven = pre.tanh() * 1.2;

        // (f_env/a_env already advanced once at top of render for mod bus.)

        // Filter cutoff: knob + contour + velocity + kb tracking (manual §4.6)
        let base_cut = params::knob_0_10_to_cutoff(p.cutoff_0_10);
        let env_oct = (p.contour_amt_0_10 / 10.0) * 6.0; // 0..6 octaves
        let mut cutoff_mod = 2.0f32.powf(f_env * env_oct);
        if p.jack_routes[4] != 0 {
            let cv = route_signal(p.jack_routes[4], mix);
            cutoff_mod *= 2.0f32.powf(cv * 2.0);
        } else if phones_to(4) {
            cutoff_mod *= 2.0f32.powf(phone_bus * 2.0);
        }
        if !p.filter_mod_on {
            // (32) OFF keeps contour; mod-bus wobble still applied below
            // only via mod_filter_oct when FILTER MOD is on. Zero it here.
            mod_filter_oct = 0.0;
            let _ = &mut cutoff_mod;
        }
        // (31) Keyboard tracking: OFF/OFF=none, 1=1/3, 2=2/3, both=max(full)
        let track: f32 = match (p.kb_control_1, p.kb_control_2) {
            (false, false) => 0.0,
            (true, false) => 1.0 / 3.0,
            (false, true) => 2.0 / 3.0,
            (true, true) => 1.0,
        };
        let kb_mult = 2.0f32.powf(track * (note - 60.0) / 12.0);
        let mut cutoff = base_cut * cutoff_mod * kb_mult * (0.75 + 0.25 * r.velocity.clamp(0.0, 1.0));
        // (32) filter mod bus (±2 oct pre-scaled in mod section)
        cutoff *= 2.0f32.powf(mod_filter_oct);
        if p.filter_mode_hi {
            cutoff *= 1.5; // HI range lift (hardware filter mode switch)
        }
        cutoff = cutoff.clamp(30.0, 17000.0);

        let smooth_c = if note_on {
            0.0 // snap to the setting on note-on (no cutoff zipper at note start)
        } else {
            (-1.0 / (0.005 * sr)).exp()
        };
        self.smooth_cutoff = self.smooth_cutoff * smooth_c + cutoff * (1.0 - smooth_c);

        let reso = p.reso_0_10 / 10.0;

        // (44)(45)(47) VOLUME + MAIN ON. Standalone has one stereo out;
        // PHONES trim is disabled in the UI (kept in place, no-op).
        let out_gain = params::knob_0_10_to_volume(p.volume_0_10)
            * if p.main_on { 1.0 } else { 0.0 };

        // A-440 reference: solid 440 Hz sine, independent of gate/filter/
        // envelope so oscs can be tuned to it by ear. Scaled by output
        // gain (VOLUME/MAIN) like the hardware output bus.
        let a440 = if p.a440_on {
            self.a440_phase += 440.0 / sr;
            if self.a440_phase >= 1.0 {
                self.a440_phase -= 1.0;
            }
            (2.0 * PI * self.a440_phase).sin() * 0.3 * out_gain * 2.0
        } else {
            0.0
        };

        if !self.amp_env.is_active() && !eff_gate {
            let tail = self.filter.process(0.0, self.smooth_cutoff, reso);
            let o = tail * a_env * out_gain * 2.0 * phone_loud_gain * loud_route_gain + a440;
            self.prev_out = o.clamp(-1.2, 1.2);
            return self.prev_out;
        }

        let lp = self.filter.process(driven, self.smooth_cutoff, reso);

        let vel = if r.velocity <= 0.0 { 0.9 } else { r.velocity.clamp(0.1, 1.0) };
        let vca = (lp * (0.6 + 0.9 * a_env * vel) * 1.4).tanh();
        let o = vca * a_env.max(0.0) * out_gain * 2.4 * phone_loud_gain * loud_route_gain + a440;
        self.prev_out = o.clamp(-1.2, 1.2);
        self.prev_out
    }
}

/// Soft limiter: linear below the threshold, smoothly compressed toward a
/// fixed ceiling above it (so poly voices never hard-clip).
fn soft_limit(x: f32) -> f32 {
    const T: f32 = 0.85;
    const C: f32 = 1.15;
    if x.abs() <= T {
        x
    } else {
        let over = (x.abs() - T) / (C - T);
        let s = if x < 0.0 { -1.0 } else { 1.0 };
        s * (T + (C - T) * over.tanh())
    }
}

// ---------------------------------------------------------------------------
// Voice pool: polyphonic allocation over a fixed set of `Voice`s.
//
// The runtime carries the currently held notes (`held` / `held_len`). Each
// sample we (re)assign any unassigned held note to a free voice, release
// voices whose note is no longer held, and sum the voices. Voice count is
// driven by `Params::voice_count` (1..=8); a count of 1 matches the original
// mono behaviour.
// ---------------------------------------------------------------------------
pub struct VoicePool {
    voices: Vec<Voice>,
    /// MIDI note currently assigned to each voice (`None` = free/releasing).
    assigned: Vec<Option<u8>>,
    /// Velocity for the note assigned to each voice.
    vels: Vec<f32>,
    sample_rate: f32,
    /// Smoothed polyphony normalization gain (~1/sqrt of sounding voices).
    norm_gain: f32,
    /// Peak mixer level across voices (OVERLOAD lamp).
    pub last_mix_peak: f32,
    /// Sequencer step (written by voice 0).
    pub last_seq_step: u8,
}

impl VoicePool {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            voices: Vec::new(),
            assigned: Vec::new(),
            vels: Vec::new(),
            sample_rate,
            norm_gain: 1.0,
            last_mix_peak: 0.0,
            last_seq_step: 0,
        }
    }

    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    pub fn set_sample_rate(&mut self, sr: f32) {
        self.sample_rate = sr;
        for v in self.voices.iter_mut() {
            v.set_sample_rate(sr);
        }
    }

    fn resize(&mut self, n: usize) {
        while self.voices.len() < n {
            self.voices.push(Voice::new(self.sample_rate));
            self.assigned.push(None);
            self.vels.push(0.0);
        }
        // Shrinking drops the highest-index voices (they just stop).
        self.voices.truncate(n);
        self.assigned.truncate(n);
        self.vels.truncate(n);
    }

    /// Assign/release voices to match the held-note set, then render and sum.
    pub fn render(&mut self, p: &Params, r: &Runtime, n_voices: usize) -> f32 {
        // The sequencer generates its own mono note stream, so it takes over
        // a single voice rather than stacking.
        let n = if p.seq_on { 1 } else { n_voices.clamp(1, 8) };
        self.resize(n);

        // Snapshot held notes from the runtime.
        let held_len = (r.held_len as usize).min(r.held.len());
        let mut held: [(u8, f32); 16] = [(0, 0.0); 16];
        for i in 0..held_len {
            held[i] = (r.held[i], r.held_vel[i]);
        }
        // Backward-compatible fallback: a caller that sets `note`/`gate`
        // directly (no held list) still sounds on voice 0.
        let mut held_len = held_len;
        if held_len == 0 && r.gate {
            if let Some(note) = r.note {
                held[0] = (note, r.velocity);
                held_len = 1;
            }
        }

        // Release voices whose assigned note is no longer held.
        for a in self.assigned.iter_mut() {
            if let Some(note) = *a {
                if !held[..held_len].iter().any(|&(n, _)| n == note) {
                    *a = None;
                }
            }
        }

        // Assign each held note that isn't already sounding to a free voice.
        for i in 0..held_len {
            let (note, vel) = held[i];
            if self.assigned.iter().any(|a| *a == Some(note)) {
                // Already playing; keep its velocity fresh.
                if let Some(idx) = self.assigned.iter().position(|a| *a == Some(note)) {
                    self.vels[idx] = vel;
                }
                continue;
            }
            // Prefer a free voice; otherwise steal the lowest playing note.
            let free = self.assigned.iter().position(|a| a.is_none());
            let idx = match free {
                Some(f) => f,
                None => {
                    let mut best = 0usize;
                    for (k, a) in self.assigned.iter().enumerate() {
                        if a.unwrap_or(255) < self.assigned[best].unwrap_or(255) {
                            best = k;
                        }
                    }
                    best
                }
            };
            self.assigned[idx] = Some(note);
            self.vels[idx] = vel;
        }

        // Render and sum each voice, gated by whether it holds a note.
        let mut sum = 0.0f32;
        for k in 0..n {
            let note = self.assigned[k];
            let mut vr = *r;
            vr.note = note;
            vr.gate = note.is_some();
            vr.velocity = self.vels[k];
            sum += self.voices[k].render(p, &vr);
        }
        // Track OVERLOAD lamp + sequencer step across the pool.
        self.last_mix_peak = self.voices.iter().map(|v| v.last_mix_peak).fold(0.0f32, f32::max);
        self.last_seq_step = self.voices.first().map(|v| v.last_seq_step).unwrap_or(0);

        // Normalize by the number of *sounding* voices (not the configured
        // count) so one note keeps the same level at any voice setting. The
        // curve is user-selectable: NATURAL sums, GENTLE is mild, CONSTANT
        // holds overall loudness fixed. Smoothed over a few ms.
        let sounding = self.assigned.iter().filter(|a| a.is_some()).count().max(1) as f32;
        let target = match p.poly_level_mode {
            1 => 1.0 / sounding.powf(0.25), // GENTLE
            2 => 1.0 / sounding.sqrt(),     // CONSTANT
            _ => 1.0,                       // NATURAL
        };
        let coeff = 1.0 - (-1.0 / (self.sample_rate * 0.005)).exp();
        self.norm_gain += (target - self.norm_gain) * coeff;
        // Soft limiter keeps summed voices from hard-clipping.
        soft_limit(sum * self.norm_gain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::{Params, Runtime};

    // Headroom probe: render a fixed duration and report how much wall-clock
    // time one core spends per second of audio, for 1/2/4/8 voices.
    //   cargo test --release bench_voice_cpu -- --ignored --nocapture
    #[test]
    #[ignore]
    fn bench_voice_cpu() {
        use std::time::Instant;
        let p = Params::default();
        let r = Runtime {
            note: Some(69),
            velocity: 0.9,
            gate: true,
            ..Runtime::default()
        };
        let sr = 44100.0_f32;
        let seconds = 5.0_f64;
        let frames = (sr as f64 * seconds) as usize;

        // Warm up caches / branch predictors.
        let mut warm = Voice::new(sr);
        for _ in 0..(sr as usize) {
            warm.render(&p, &r);
        }
        std::hint::black_box(&mut warm);

        println!("--- voice CPU headroom (1 core = 100%) ---");
        println!("data: {seconds:.0}s audio, default patch, note 69, {sr:.0} Hz");        for &voices in &[1usize, 2, 4, 8] {
            let mut pool: Vec<Voice> = (0..voices).map(|_| Voice::new(sr)).collect();
            let start = Instant::now();
            let mut acc = 0.0f32;
            for _ in 0..frames {
                for v in pool.iter_mut() {
                    acc += v.render(&p, &r);
                }
            }
            let elapsed = start.elapsed().as_secs_f64();
            std::hint::black_box(acc);

            let xrt = seconds / elapsed;
            let pct = elapsed / seconds * 100.0;
            println!(
                "  {voices} voice(s): {elapsed:.3}s cpu -> {xrt:6.1}x realtime ({pct:5.1}% of one core)"
            );
        }
    }

    #[test]
    fn voice_renders_finite_with_gate() {
        let mut v = Voice::new(44100.0);
        let p = Params::default();
        let r = Runtime {
            note: Some(69),
            velocity: 0.9,
            bend_semi: 0.0,
            mod_wheel: 0.0,
            gate: true,
            overload: 0.0,
            seq_step: 0,
            ..Runtime::default()
        };
        let mut rms = 0.0;
        for _ in 0..44100 {
            let s = v.render(&p, &r);
            assert!(s.is_finite(), "non-finite sample: {s}");
            rms += s * s;
        }
        rms = (rms / 44100.0).sqrt();
        assert!(rms > 0.001, "voice too quiet, rms={rms}");
        assert!(rms < 2.0, "voice blowing up, rms={rms}");
    }

    #[test]
    fn pool_renders_poly_finite() {
        let p = Params::default();
        let mut rt = Runtime::default();
        rt.note_on(60, 0.9);
        rt.note_on(64, 0.9);
        rt.note_on(67, 0.9);
        let mut pool = VoicePool::new(44100.0);
        let mut peak = 0.0f32;
        for _ in 0..44100 {
            let s = pool.render(&p, &rt, 4);
            assert!(s.is_finite(), "non-finite pool sample: {s}");
            peak = peak.max(s.abs());
        }
        assert!(peak > 0.01, "poly pool too quiet, peak={peak}");
        assert!(peak < 2.0, "poly pool blowing up, peak={peak}");

        // Releasing all notes should decay back to silence.
        rt.all_notes_off();
        let mut tail = 0.0f32;
        for _ in 0..(44100 * 3) {
            tail = pool.render(&p, &rt, 4).abs();
        }
        assert!(tail < 0.01, "pool not silent after release, tail={tail}");
    }

    // Probe: RMS of a sustained note vs EMPHASIS setting. Run explicitly:
    //   cargo test --release probe_emphasis -- --ignored --nocapture
    #[test]
    #[ignore]
    fn probe_emphasis() {
        for reso in [0.0f32, 2.5, 5.0, 7.5, 10.0] {
            let mut p = Params::default();
            p.reso_0_10 = reso;
            let mut v = Voice::new(44100.0);
            let r = Runtime {
                note: Some(60),
                velocity: 0.9,
                gate: true,
                ..Runtime::default()
            };
            // Let the envelope settle, then measure.
            for _ in 0..8820 {
                v.render(&p, &r);
            }
            let mut sum = 0.0f64;
            for _ in 0..44100 {
                let s = v.render(&p, &r) as f64;
                sum += s * s;
            }
            let rms = (sum / 44100.0).sqrt();
            println!("EMPHASIS {:>4.1}  rms={rms:.4}  ({:.1} dB)", reso, 20.0 * rms.log10());
        }
    }

    // RMS of a steady sine driven through the ladder filter directly (bypasses
    // the voice's output tanh, which otherwise re-adds harmonics and masks the
    // filter's effect on brightness).
    fn filter_rms(cutoff: f32, freq: f32, resonance: f32) -> f32 {
        let mut f = LadderFilter::new(88200.0);
        let mut ph = 0.0f32;
        let dp = 2.0 * std::f32::consts::PI * freq / 44100.0;
        let n = 44100usize;
        let mut sum = 0.0f64;
        for i in 0..n {
            let x = ph.sin();
            ph += dp;
            if ph > std::f32::consts::PI * 2.0 {
                ph -= std::f32::consts::PI * 2.0;
            }
            let y = f.process(x, cutoff, resonance);
            if i >= n / 2 {
                sum += (y as f64) * (y as f64);
            }
        }
        (sum / (n as f64 / 2.0)).sqrt() as f32
    }

    // CUTOFF: opening the filter must pass more high-frequency content.
    #[test]
    fn cutoff_controls_filter_response() {
        // A 3 kHz tone is passed when the cutoff is above it, cut below it.
        let closed = filter_rms(400.0, 3000.0, 0.0);
        let open = filter_rms(8000.0, 3000.0, 0.0);
        println!("CUTOFF 400Hz -> rms={closed:.4}   8000Hz -> rms={open:.4} (3kHz tone)");
        assert!(
            open > closed * 3.0,
            "opening cutoff should pass highs: open={open} closed={closed}"
        );

        // Response to a 2 kHz tone must rise monotonically with the cutoff.
        let mut prev = -1.0f32;
        for cut in [300.0f32, 1000.0, 3000.0, 9000.0, 15000.0] {
            let r = filter_rms(cut, 2000.0, 0.0);
            println!("CUTOFF {cut:>6.0}Hz -> rms={r:.4} (2kHz tone)");
            assert!(
                r > prev,
                "response must rise with cutoff: cut={cut} r={r} prev={prev}"
            );
            prev = r;
        }
    }

    // CONTOUR: filter-envelope amount must open the cutoff on top of the knob.
    #[test]
    fn contour_opens_filter() {
        let peak_cutoff = |contour: f32| {
            let mut p = Params::default();
            p.cutoff_0_10 = 2.0;
            p.contour_amt_0_10 = contour;
            p.amp_attack_0_10 = 0.0;
            p.filt_attack_0_10 = 0.0;
            p.filt_sustain_0_10 = 10.0; // hold the envelope open so it settles
            let mut v = Voice::new(44100.0);
            let r = Runtime {
                note: Some(60),
                velocity: 0.9,
                gate: true,
                ..Runtime::default()
            };
            // Render past the smoothing transient, then read the settled cutoff.
            for _ in 0..44100 {
                v.render(&p, &r);
            }
            let settled = v.smooth_cutoff;
            println!(
                "  contour={contour} base={:.1} settled_cutoff={settled:.1} Hz",
                params::knob_0_10_to_cutoff(p.cutoff_0_10)
            );
            settled
        };
        let c0 = peak_cutoff(0.0);
        let c10 = peak_cutoff(10.0);
        println!("CONTOUR  0 settled_cutoff={c0:.1} Hz");
        println!("CONTOUR 10 settled_cutoff={c10:.1} Hz");
        assert!(
            c10 > c0 * 30.0,
            "contour should open the filter ~6 octaves: c0={c0} c10={c10}"
        );
    }

    #[test]
    fn filter_stable_at_high_reso() {        let mut f = LadderFilter::new(88200.0);
        for _ in 0..10000 {
            let y = f.process(0.8, 8000.0, 0.95);
            assert!(y.is_finite());
            assert!(y.abs() < 5.0, "filter unstable: {y}");
        }
    }
}

// ---------------------------------------------------------------------------
// Golden-file DSP regression tests.
//
// The engine is fully deterministic (fixed xorshift noise seed + fixed drift
// phases), so a fixed patch + note renders identical samples on a given
// machine/compiler. Each test renders a short block, reduces it to a small set
// of aggregate metrics, and compares them against a checked-in golden file in
// `tests/golden/`. If a file is missing (or `UPDATE_GOLDEN=1` is set) it is
// (re)written instead of asserted, so goldens can be regenerated after an
// intentional DSP change.
// ---------------------------------------------------------------------------
#[cfg(test)]
mod golden {
    use super::*;
    use crate::params::{Params, Runtime};
    use std::collections::HashMap;
    use std::path::Path;

    const SR: f32 = 44100.0;

    fn render(p: &Params, note: u8, samples: usize, gate_len: usize) -> Vec<f32> {
        let mut v = Voice::new(SR);
        let mut out = Vec::with_capacity(samples);
        for i in 0..samples {
            let r = Runtime {
                note: Some(note),
                velocity: 0.9,
                bend_semi: 0.0,
                mod_wheel: 0.0,
                gate: i < gate_len,
                overload: 0.0,
                seq_step: 0,
                ..Runtime::default()
            };
            out.push(v.render(p, &r));
        }
        out
    }

    fn fingerprint(s: &[f32]) -> Vec<(&'static str, f64)> {
        let n = s.len() as f64;
        let rms = (s.iter().map(|x| (*x as f64) * (*x as f64)).sum::<f64>() / n).sqrt();
        let peak = s.iter().map(|x| x.abs() as f64).fold(0.0, f64::max);
        let dc = s.iter().map(|x| *x as f64).sum::<f64>() / n;
        let zeros = s.windows(2).filter(|w| (w[0] < 0.0) != (w[1] < 0.0)).count();
        let half = s.len() / 2;
        let e1 = s[..half].iter().map(|x| (*x as f64) * (*x as f64)).sum::<f64>();
        let e2 = s[half..].iter().map(|x| (*x as f64) * (*x as f64)).sum::<f64>();
        let tail_ratio = e2 / (e1 + 1e-12);
        vec![
            ("rms", rms),
            ("peak", peak),
            ("dc", dc),
            ("zeros", zeros as f64),
            ("tail_ratio", tail_ratio),
        ]
    }

    fn check_golden(name: &str, p: &Params, note: u8, samples: usize, gate_len: usize) {
        let out = render(p, note, samples, gate_len);
        assert!(
            out.iter().all(|x| x.is_finite()),
            "{name}: produced a non-finite sample"
        );
        let fp = fingerprint(&out);

        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("golden");
        let path = dir.join(format!("{name}.txt"));
        let produced = fp
            .iter()
            .map(|(k, v)| format!("{k}={v:.6}"))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";

        if std::env::var("UPDATE_GOLDEN").is_ok() || !path.exists() {
            std::fs::create_dir_all(&dir).expect("create golden dir");
            std::fs::write(&path, &produced).expect("write golden");
            eprintln!("golden: wrote {}", path.display());
            return;
        }

        let text = std::fs::read_to_string(&path).expect("read golden");
        let want: HashMap<&str, f64> = text
            .lines()
            .filter_map(|l| l.split_once('='))
            .filter_map(|(k, v)| v.trim().parse::<f64>().ok().map(|v| (k.trim(), v)))
            .collect();

        for (k, got) in &fp {
            let w = *want
                .get(k)
                .unwrap_or_else(|| panic!("{name}: golden is missing metric `{k}`"));
            if *k == "zeros" {
                assert_eq!(
                    got.round() as i64,
                    w.round() as i64,
                    "{name}: zero-crossings changed ({got} vs {w})"
                );
            } else {
                let tol = (w.abs() * 0.005).max(1e-4);
                assert!(
                    (got - w).abs() <= tol,
                    "{name}: metric `{k}` drifted ({got:.6} vs {w:.6}; tol {tol:.6})"
                );
            }
        }
    }

    #[test]
    fn golden_default_sustained() {
        check_golden("default_sustained", &Params::default(), 69, 8192, 8192);
    }

    #[test]
    fn golden_default_release() {
        // Gate for the first 1/8 of the block, then release: exercises the
        // amp/filter envelope tails and the "inactive" render path.
        check_golden("default_release", &Params::default(), 69, 8192, 1024);
    }

    #[test]
    fn golden_noise_white() {
        let mut p = Params::default();
        p.noise_on = true;
        p.noise_pink = false;
        p.noise_level_0_10 = 8.0;
        check_golden("noise_white", &p, 60, 8192, 8192);
    }

    #[test]
    fn golden_resonant_saw() {
        let mut p = Params::default();
        p.osc1_wave = OscWave::Saw;
        p.osc1_on = true;
        p.cutoff_0_10 = 3.5;
        p.reso_0_10 = 8.5;
        check_golden("resonant_saw", &p, 62, 8192, 8192);
    }
}
