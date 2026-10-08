//! The audio backend: PipeWire in production, `FakeAudio` in tests.

use crate::error::AudioError;
use crate::model::AudioSnapshot;

pub trait Audio: Send {
    fn snapshot(&mut self) -> Result<AudioSnapshot, AudioError>;
    fn set_default(&mut self, id: u32) -> Result<(), AudioError>;
    /// `id` names an endpoint or a stream; the volume is clamped by the caller.
    fn set_volume(&mut self, id: u32, volume: f32) -> Result<(), AudioError>;
    fn set_muted(&mut self, id: u32, muted: bool) -> Result<(), AudioError>;
    fn set_profile(&mut self, card_id: u32, profile: &str) -> Result<(), AudioError>;
}
