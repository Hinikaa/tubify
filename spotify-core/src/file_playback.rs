/*
 * Swiftify, a spotify player for desktop
 * Copyright (C) 2026  shred0s
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

//! a music file played from disk, of the person's own: decoded by rodio,
//! counted for the position, fed to the spectrum and the equalizer on its way
//! to the output.

use std::{
    collections::VecDeque,
    fs, io,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

use rodio::Source;

use crate::dsp::{DspProcessor, SharedDsp};
use crate::output::BlockSource;
use crate::spectrum::{ANALYSIS_SAMPLE_COUNT, SharedSpectrum};

// ---------------------------------------------------------------------------
// local playback
// ---------------------------------------------------------------------------

/// active local-file playback session.
#[derive(Clone)]
pub(crate) struct LocalPlayback {
    /// on-disk path of the flac file being played (needed for re-decoding on seek).
    pub(crate) path: PathBuf,
    /// total duration in milliseconds.
    pub(crate) duration_ms: u64,
    /// number of decoded samples (all channels) already consumed.
    pub(crate) pushed_frames: Arc<AtomicU64>,
    /// absolute position represented by the first sample in the active source.
    pub(crate) base_position_ms: u64,
    /// number of audio channels in the decoded stream.
    pub(crate) channels: u32,
    /// sample rate of the decoded stream.
    pub(crate) sample_rate: u32,
    /// linear gain from the file's replaygain tags (1.0 when untagged).
    pub(crate) replay_gain: f32,
    /// signals an in-flight decode to stop (on a seek/track change).
    pub(crate) cancel: Arc<AtomicBool>,
}

impl LocalPlayback {
    pub(crate) fn position_ms(&self) -> u32 {
        let channels = self.channels.max(1) as u64;
        let sample_rate = self.sample_rate.max(1) as u64;
        let samples = self.pushed_frames.load(Ordering::Relaxed);
        self.base_position_ms
            .saturating_add(samples / channels * 1_000 / sample_rate)
            .min(u32::MAX as u64) as u32
    }
}

/// wraps a rodio decoder so decoded samples are forwarded to the live sink, fed into the
/// spectrum analyzer, and counted for position tracking.
const DSP_BLOCK: usize = 4_096;

pub(crate) struct LocalTrackSource {
    inner: rodio::Decoder<io::BufReader<fs::File>>,
    spectrum: SharedSpectrum,
    pushed_frames: Arc<AtomicU64>,
    cancel: Arc<AtomicBool>,
    replay_gain: f32,
    normalise: Arc<AtomicBool>,
    analysis_samples: [f32; ANALYSIS_SAMPLE_COUNT],
    analysis_index: usize,
    analysis_enabled: bool,
    dsp: DspProcessor,
    /// processed samples waiting to be handed out one at a time.
    processed: VecDeque<f32>,
    drained: bool,
}

impl LocalTrackSource {
    pub(crate) fn new(
        decoder: rodio::Decoder<io::BufReader<fs::File>>,
        spectrum: SharedSpectrum,
        pushed_frames: Arc<AtomicU64>,
        cancel: Arc<AtomicBool>,
        dsp: SharedDsp,
        replay_gain: f32,
        normalise: Arc<AtomicBool>,
    ) -> Self {
        Self {
            inner: decoder,
            spectrum,
            pushed_frames,
            cancel,
            replay_gain,
            normalise,
            analysis_samples: [0.0; ANALYSIS_SAMPLE_COUNT],
            analysis_index: 0,
            analysis_enabled: false,
            dsp: DspProcessor::new(dsp),
            processed: VecDeque::new(),
            drained: false,
        }
    }

    /// decodes a block and runs the chain over it. pushed_frames counts what
    /// was read from the file, not what came out: the progress bar tracks the
    /// position in the recording, which a tempo change does not move.
    fn fill(&mut self) {
        while self.processed.is_empty() && !self.drained {
            let mut block = Vec::with_capacity(DSP_BLOCK);
            for _ in 0..DSP_BLOCK {
                match self.inner.next() {
                    Some(sample) => block.push(sample),
                    None => {
                        self.drained = true;
                        break;
                    }
                }
            }
            if block.is_empty() {
                return;
            }
            self.pushed_frames
                .fetch_add(block.len() as u64, Ordering::Relaxed);
            if self.normalise.load(Ordering::Relaxed) && (self.replay_gain - 1.0).abs() > 1e-4 {
                for sample in &mut block {
                    *sample *= self.replay_gain;
                }
            }
            self.processed.extend(self.dsp.process(block));
        }
    }
}

impl LocalTrackSource {
    /// hands the analysis what is about to play, a run at a time
    fn analyse(&mut self, mut samples: &[f32]) {
        while !samples.is_empty() {
            if self.analysis_index == 0 {
                self.analysis_enabled = self.spectrum.is_enabled();
            }
            let taken = samples
                .len()
                .min(ANALYSIS_SAMPLE_COUNT - self.analysis_index);
            if self.analysis_enabled {
                self.analysis_samples[self.analysis_index..self.analysis_index + taken]
                    .copy_from_slice(&samples[..taken]);
            }
            self.analysis_index += taken;
            samples = &samples[taken..];
            if self.analysis_index == ANALYSIS_SAMPLE_COUNT {
                if self.analysis_enabled {
                    self.spectrum
                        .queue_interleaved_f32(self.analysis_samples, 1.0);
                }
                self.analysis_index = 0;
            }
        }
    }
}

impl Iterator for LocalTrackSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let mut sample = [0.0];
        (self.read(&mut sample) == 1).then_some(sample[0])
    }
}

impl BlockSource for LocalTrackSource {
    fn read(&mut self, out: &mut [f32]) -> usize {
        if self.cancel.load(Ordering::Relaxed) {
            return 0;
        }
        let mut written = 0;
        while written < out.len() {
            if self.processed.is_empty() {
                self.fill();
                if self.processed.is_empty() {
                    break;
                }
            }
            let (front, _) = self.processed.as_slices();
            let taken = front.len().min(out.len() - written);
            out[written..written + taken].copy_from_slice(&front[..taken]);
            self.processed.drain(..taken);
            written += taken;
        }
        self.analyse(&out[..written]);
        written
    }
}

impl Source for LocalTrackSource {
    fn current_span_len(&self) -> Option<usize> {
        self.inner.current_span_len()
    }

    fn channels(&self) -> rodio::ChannelCount {
        self.inner.channels()
    }

    fn sample_rate(&self) -> rodio::SampleRate {
        self.inner.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_position_includes_seek_offset() {
        let playback = LocalPlayback {
            path: PathBuf::new(),
            duration_ms: 120_000,
            pushed_frames: Arc::new(AtomicU64::new(44_100 as u64 * 2)),
            base_position_ms: 30_000,
            channels: 2,
            sample_rate: 44_100,
            replay_gain: 1.0,
            cancel: Arc::new(AtomicBool::new(false)),
        };
        assert_eq!(playback.position_ms(), 31_000);
    }
}
