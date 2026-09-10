//! Spatial mixing: turn a listener position and the current directory entries
//! into per-voice parameter targets.
//!
//! This is deliberately Bevy-free and deterministic so it can be unit-tested
//! without a window, an audio device, or a GPU. The caller converts
//! `FileNode`s / arena cells into [`NodePoint`]s and feeds the returned
//! [`VoiceTarget`]s to the Glicol engine.

use super::theme::{ModeProfile, MusicTheme};
use crate::config;

const MAX: usize = config::MUSIC_MAX_VOICES;

/// Where the music is "listening" from, on the ground plane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Listener {
    pub x: f32,
    pub z: f32,
}

/// One filesystem entry's position and musical identity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NodePoint {
    /// Index into the navigator's entry list, reported back in [`VoiceTarget`].
    pub index: usize,
    pub x: f32,
    pub z: f32,
    pub node_seed: u64,
    pub is_dir: bool,
}

/// One voice to push to Glicol, already smoothed toward its target.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoiceTarget {
    /// The entry this voice is following, or `None` while a released voice
    /// fades out.
    pub node_index: Option<usize>,
    pub slot: usize,
    pub freq: f32,
    pub cutoff: f32,
    pub gain: f32,
    pub pan: f32,
}

/// Inverse-square-style falloff. `1.0` at the listener, `0.5` at `radius`, and
/// asymptotically zero beyond it.
pub fn falloff(distance: f32, radius: f32) -> f32 {
    if distance <= 0.0 {
        return 1.0;
    }
    let r2 = radius * radius;
    r2 / (r2 + distance * distance)
}

/// Assigns nearby entries to a fixed set of voice slots and smooths their
/// parameters over time.
///
/// Slots persist across frames, so a voice does not jump between two
/// equidistant blocks. A slot is only released when its entry leaves the
/// proximity zone, and only the profile's budget of slots may be occupied.
#[derive(Debug)]
pub struct VoiceMixer {
    slots: [Option<usize>; MAX],
    freq: [f32; MAX],
    cutoff: [f32; MAX],
    gain: [f32; MAX],
    pan: [f32; MAX],
}

impl Default for VoiceMixer {
    fn default() -> Self {
        Self {
            slots: [None; MAX],
            freq: [0.0; MAX],
            cutoff: [0.0; MAX],
            gain: [0.0; MAX],
            pan: [0.0; MAX],
        }
    }
}

impl VoiceMixer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Forgets every assignment, e.g. after a directory change.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Number of slots currently holding an entry.
    pub fn active_slots(&self) -> usize {
        self.slots.iter().filter(|slot| slot.is_some()).count()
    }

    /// Advances the mixer by `dt` seconds and returns the voices to send.
    pub fn update(
        &mut self,
        listener: Listener,
        nodes: &[NodePoint],
        theme: &MusicTheme,
        profile: ModeProfile,
        dt: f32,
    ) -> Vec<VoiceTarget> {
        let dt = dt.clamp(0.0, 1.0);
        let radius = profile.proximity_radius();
        let cutoff_distance = radius * config::MUSIC_PROXIMITY_CUTOFF_MULTIPLIER;
        let budget = profile.voice_budget().min(MAX);
        let ceiling = profile.gain_ceiling();
        let tau = profile.smoothing_tau();
        let alpha = if tau > 0.0 {
            (1.0 - (-dt / tau).exp()).clamp(0.0, 1.0)
        } else {
            1.0
        };

        // Entries inside the zone, strongest first.
        let mut candidates: Vec<(usize, f32)> = Vec::new();
        for (position, node) in nodes.iter().enumerate() {
            let dx = node.x - listener.x;
            let dz = node.z - listener.z;
            let distance = (dx * dx + dz * dz).sqrt();
            if distance > cutoff_distance {
                continue;
            }
            candidates.push((position, falloff(distance, radius)));
        }
        candidates.sort_by(|a, b| b.1.total_cmp(&a.1));

        // Release slots whose entry has faded, or left the zone entirely.
        for slot in self.slots.iter_mut() {
            if let Some(position) = *slot {
                let weight = candidates
                    .iter()
                    .find(|(candidate, _)| *candidate == position)
                    .map_or(0.0, |(_, weight)| *weight);
                if weight < config::MUSIC_SLOT_RELEASE_THRESHOLD {
                    *slot = None;
                }
            }
        }

        // Fill free slots, up to the profile budget, with the strongest new
        // entries. Already-assigned entries keep their slot, and a newcomer must
        // clear the acquire threshold so the release band cannot flicker.
        for &(position, weight) in &candidates {
            if weight < config::MUSIC_SLOT_ACQUIRE_THRESHOLD || self.slots.contains(&Some(position))
            {
                continue;
            }
            let Some(free) = (0..budget).find(|slot| self.slots[*slot].is_none()) else {
                break;
            };
            self.slots[free] = Some(position);
        }

        // Smooth toward targets and emit messages.
        let mut targets = Vec::with_capacity(MAX);
        for slot in 0..MAX {
            let (node_index, target_freq, target_cutoff, target_gain, target_pan) =
                match self.slots[slot] {
                    Some(position) => {
                        let node = &nodes[position];
                        let weight = candidates
                            .iter()
                            .find(|(candidate, _)| *candidate == position)
                            .map_or(0.0, |(_, weight)| *weight);
                        let direction = ((node.x - listener.x) / radius).clamp(-1.0, 1.0);
                        let level = if node.is_dir { 0.8 } else { 1.0 };
                        let cutoff = (theme.node_cutoff(node.node_seed)
                            + weight * config::MUSIC_VOICE_CUTOFF_SPAN)
                            .clamp(config::MUSIC_VOICE_CUTOFF_MIN, 12_000.0);
                        (
                            Some(node.index),
                            theme.node_hz(node.node_seed, node.is_dir),
                            cutoff,
                            weight * ceiling * level,
                            direction * config::MUSIC_PAN_RANGE,
                        )
                    }
                    None => (
                        None,
                        self.freq[slot],
                        self.cutoff[slot],
                        0.0,
                        self.pan[slot],
                    ),
                };

            self.freq[slot] += (target_freq - self.freq[slot]) * alpha;
            self.cutoff[slot] += (target_cutoff - self.cutoff[slot]) * alpha;
            self.gain[slot] += (target_gain - self.gain[slot]) * alpha;
            self.pan[slot] += (target_pan - self.pan[slot]) * alpha;

            if node_index.is_some() || self.gain[slot] > 0.0005 {
                targets.push(VoiceTarget {
                    node_index,
                    slot,
                    freq: self.freq[slot],
                    cutoff: self.cutoff[slot],
                    gain: self.gain[slot],
                    pan: self.pan[slot],
                });
            } else {
                self.gain[slot] = 0.0;
            }
        }
        targets
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::music::theme::MusicTheme;

    fn node(index: usize, x: f32, z: f32, is_dir: bool) -> NodePoint {
        NodePoint {
            index,
            x,
            z,
            node_seed: index as u64 * 0x9e37_79b9,
            is_dir,
        }
    }

    fn theme() -> MusicTheme {
        MusicTheme::from_seed(0xfeed_face)
    }

    #[test]
    fn falloff_is_one_at_the_listener_and_monotonic() {
        assert_eq!(falloff(0.0, 6.0), 1.0);
        let near = falloff(2.0, 6.0);
        let far = falloff(5.0, 6.0);
        assert!(near > far);
        assert!(far > 0.0 && far < 1.0);
        assert!((falloff(6.0, 6.0) - 0.5).abs() < 0.001);
    }

    #[test]
    fn the_nearest_entry_gets_the_loudest_voice() {
        let mut mixer = VoiceMixer::new();
        let nodes = [node(0, 1.0, 0.0, false), node(1, 9.0, 0.0, false)];
        let listener = Listener { x: 0.0, z: 0.0 };

        let mut targets = Vec::new();
        for _ in 0..60 {
            targets = mixer.update(listener, &nodes, &theme(), ModeProfile::Calm, 1.0 / 60.0);
        }

        let near = targets
            .iter()
            .find(|target| target.node_index == Some(0))
            .expect("near voice");
        let far = targets
            .iter()
            .find(|target| target.node_index == Some(1))
            .expect("far voice");
        assert!(near.gain > far.gain, "near {} far {}", near.gain, far.gain);
    }

    #[test]
    fn slots_are_stable_for_a_settled_entry() {
        let mut mixer = VoiceMixer::new();
        let nodes = [node(0, 0.0, 0.0, false), node(1, 2.0, 0.0, false)];
        let listener = Listener { x: 0.0, z: 0.0 };

        for _ in 0..30 {
            mixer.update(listener, &nodes, &theme(), ModeProfile::Calm, 1.0 / 60.0);
        }
        let slot = mixer
            .update(listener, &nodes, &theme(), ModeProfile::Calm, 1.0 / 60.0)
            .iter()
            .find(|target| target.node_index == Some(0))
            .unwrap()
            .slot;

        for _ in 0..30 {
            mixer.update(listener, &nodes, &theme(), ModeProfile::Calm, 1.0 / 60.0);
        }
        let later = mixer
            .update(listener, &nodes, &theme(), ModeProfile::Calm, 1.0 / 60.0)
            .iter()
            .find(|target| target.node_index == Some(0))
            .unwrap()
            .slot;
        assert_eq!(slot, later);
    }

    #[test]
    fn a_voice_fades_out_once_the_listener_leaves() {
        let mut mixer = VoiceMixer::new();
        let near = [node(0, 0.0, 0.0, false)];
        let listener = Listener { x: 0.0, z: 0.0 };

        for _ in 0..60 {
            mixer.update(listener, &near, &theme(), ModeProfile::Calm, 1.0 / 60.0);
        }
        assert_eq!(mixer.active_slots(), 1);

        // Move far away and let the release + fade settle.
        let far = Listener {
            x: 1000.0,
            z: 1000.0,
        };
        let mut last = Vec::new();
        for _ in 0..240 {
            last = mixer.update(far, &near, &theme(), ModeProfile::Calm, 1.0 / 60.0);
        }
        assert_eq!(mixer.active_slots(), 0);
        assert!(last.iter().all(|target| target.gain < 0.001));
    }

    #[test]
    fn action_engages_more_voices_than_calm() {
        let mut calm = VoiceMixer::new();
        let mut action = VoiceMixer::new();
        // Entries spread across the action radius but beyond the calm one.
        let nodes: Vec<NodePoint> = (0..6)
            .map(|i| node(i, i as f32 * 2.0, 0.0, false))
            .collect();
        let listener = Listener { x: 0.0, z: 0.0 };

        for _ in 0..60 {
            calm.update(listener, &nodes, &theme(), ModeProfile::Calm, 1.0 / 60.0);
            action.update(listener, &nodes, &theme(), ModeProfile::Action, 1.0 / 60.0);
        }
        assert!(action.active_slots() > calm.active_slots());
    }
}
