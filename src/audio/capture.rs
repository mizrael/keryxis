use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

/// Captures audio from the default microphone
pub struct AudioCapture {
    is_recording: Arc<AtomicBool>,
    audio_buffer: Arc<Mutex<Vec<f32>>>,
    sample_rate: u32,
}

impl AudioCapture {
    pub fn new(target_sample_rate: u32) -> Self {
        Self {
            is_recording: Arc::new(AtomicBool::new(false)),
            audio_buffer: Arc::new(Mutex::new(Vec::new())),
            sample_rate: target_sample_rate,
        }
    }

    /// Start recording audio from the microphone.
    /// Returns a handle that can be used to stop recording and retrieve audio.
    pub fn start_recording(&self) -> Result<RecordingHandle> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| anyhow::anyhow!("No input device available"))?;

        tracing::info!("Using input device: {}", device.name()?);

        let supported_config = device.default_input_config()?;
        let device_sample_rate = supported_config.sample_rate().0;
        let device_channels = supported_config.channels() as usize;

        tracing::info!(
            "Device config: {}Hz, {} channels, {:?}",
            device_sample_rate,
            device_channels,
            supported_config.sample_format()
        );

        self.is_recording.store(true, Ordering::SeqCst);
        {
            let mut buf = self.audio_buffer.lock().unwrap();
            buf.clear();
        }

        let is_recording = self.is_recording.clone();
        let audio_buffer = self.audio_buffer.clone();
        let target_rate = self.sample_rate;

        let stream = device.build_input_stream(
            &supported_config.into(),
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                if !is_recording.load(Ordering::SeqCst) {
                    return;
                }

                // Convert to mono if needed
                let mono: Vec<f32> = if device_channels > 1 {
                    data.chunks(device_channels)
                        .map(|frame| frame.iter().sum::<f32>() / device_channels as f32)
                        .collect()
                } else {
                    data.to_vec()
                };

                // Simple resampling if needed (linear interpolation)
                let resampled = if device_sample_rate != target_rate {
                    resample(&mono, device_sample_rate, target_rate)
                } else {
                    mono
                };

                let mut buf = audio_buffer.lock().unwrap();
                buf.extend_from_slice(&resampled);
            },
            |err| {
                tracing::error!("Audio input error: {}", err);
            },
            None,
        )?;

        stream.play()?;
        tracing::info!("Recording started");

        Ok(RecordingHandle {
            _stream: stream,
            is_recording: self.is_recording.clone(),
            audio_buffer: self.audio_buffer.clone(),
        })
    }

    /// Check if currently recording
    pub fn is_recording(&self) -> bool {
        self.is_recording.load(Ordering::SeqCst)
    }
}

/// Handle to an active recording session
pub struct RecordingHandle {
    _stream: cpal::Stream,
    is_recording: Arc<AtomicBool>,
    audio_buffer: Arc<Mutex<Vec<f32>>>,
}

impl RecordingHandle {
    /// Stop recording and return the captured audio samples
    pub fn stop(self) -> Vec<f32> {
        self.is_recording.store(false, Ordering::SeqCst);
        let buf = self.audio_buffer.lock().unwrap();
        tracing::info!("Recording stopped: {} samples captured", buf.len());
        buf.clone()
    }

    /// Get a snapshot of the current audio buffer (for VAD checking)
    pub fn current_samples(&self) -> Vec<f32> {
        let buf = self.audio_buffer.lock().unwrap();
        buf.clone()
    }

    /// Get the number of samples captured so far
    pub fn sample_count(&self) -> usize {
        let buf = self.audio_buffer.lock().unwrap();
        buf.len()
    }
}

/// Simple linear interpolation resampling
fn resample(input: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate || input.is_empty() {
        return input.to_vec();
    }

    let ratio = from_rate as f64 / to_rate as f64;
    let output_len = (input.len() as f64 / ratio) as usize;
    let mut output = Vec::with_capacity(output_len);

    for i in 0..output_len {
        let src_idx = i as f64 * ratio;
        let idx = src_idx as usize;
        let frac = src_idx - idx as f64;

        let sample = if idx + 1 < input.len() {
            input[idx] as f64 * (1.0 - frac) + input[idx + 1] as f64 * frac
        } else {
            input[idx.min(input.len() - 1)] as f64
        };

        output.push(sample as f32);
    }

    output
}
