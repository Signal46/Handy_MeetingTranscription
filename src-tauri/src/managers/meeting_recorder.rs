use anyhow::Result;
use chrono::Utc;
use hound::{WavWriter, WavSpec, SampleFormat};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Stream, Sample};

pub struct MeetingRecorder {
    writer: Arc<Mutex<Option<WavWriter<File>>>>,
    stream: Option<Stream>,
    file_path: Option<PathBuf>,
}

impl MeetingRecorder {
    pub fn new() -> Self {
        Self {
            writer: Arc::new(Mutex::new(None)),
            stream: None,
            file_path: None,
        }
    }

    pub fn start_recording(&mut self, output_dir: &Path) -> Result<()> {
        let timestamp = Utc::now().format("%Y%m%d%H%M%S");
        let filename = format!("meeting_{}.wav", timestamp);
        let file_path = output_dir.join(filename);

        let spec = WavSpec {
            channels: 1,
            sample_rate: 16000,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        };

        let writer = WavWriter::create(&file_path, spec)?;
        *self.writer.lock().unwrap() = Some(writer);
        self.file_path = Some(file_path);

        let host = cpal::default_host();
        let device = host.default_input_device().ok_or_else(|| anyhow::anyhow!("No input device found"))?;
        let config = device.default_input_config()?;

        let writer_clone = self.writer.clone();
        let stream = device.build_input_stream(
            &config.into(),
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                if let Some(writer) = writer_clone.lock().unwrap().as_mut() {
                    for &sample in data {
                        let i16_sample = sample.to_i16();
                        writer.write_sample(i16_sample).unwrap();
                    }
                }
            },
            |err| log::error!("An error occurred on stream: {}", err),
            None
        )?;

        stream.play()?;
        self.stream = Some(stream);
        Ok(())
    }

    pub fn stop_recording(&mut self) -> Result<PathBuf> {
        if let Some(stream) = self.stream.take() {
            stream.pause()?;
        }
        if let Some(writer) = self.writer.lock().unwrap().take() {
            writer.finalize()?;
        }
        self.file_path.clone().ok_or_else(|| anyhow::anyhow!("No file path found"))
    }
}
