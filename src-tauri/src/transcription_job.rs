use anyhow::Result;
use chrono::Utc;
use hound::{SampleFormat, WavSpec, WavWriter};
use rubato::{Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction};
use serde::{Deserialize, Serialize};
use std::fs;
use std::fs::File;
use std::path::{Path, PathBuf};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TranscriptionStatus {
    Queued,
    Processing,
    Completed,
    Failed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionJob {
    pub id: String,
    pub file_path: PathBuf,
    pub status: TranscriptionStatus,
    pub progress: f32, // 0.0 to 1.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionJobQueue {
    pub jobs: Vec<TranscriptionJob>,
    path: PathBuf,
}

impl TranscriptionJobQueue {
    pub fn new(path: PathBuf) -> Self {
        let mut queue = Self {
            jobs: Vec::new(),
            path,
        };
        queue.load().unwrap_or_default();
        queue
    }

    pub fn add_job(&mut self, job: TranscriptionJob) {
        self.jobs.push(job);
        self.save().unwrap_or_default();
    }

    pub fn get_next_job(&self) -> Option<&TranscriptionJob> {
        self.jobs
            .iter()
            .find(|j| j.status == TranscriptionStatus::Queued)
    }

    pub fn update_job_status(&mut self, id: &str, status: TranscriptionStatus) {
        if let Some(job) = self.jobs.iter_mut().find(|j| j.id == id) {
            job.status = status;
            self.save().unwrap_or_default();
        }
    }

    pub fn update_job_progress(&mut self, id: &str, progress: f32) {
        if let Some(job) = self.jobs.iter_mut().find(|j| j.id == id) {
            job.progress = progress;
            self.save().unwrap_or_default();
        }
    }

    pub fn cancel_job(&mut self, id: &str) -> bool {
        let initial_len = self.jobs.len();
        self.jobs.retain(|j| j.id != id);
        let changed = self.jobs.len() != initial_len;
        if changed {
            self.save().unwrap_or_default();
        }
        changed
    }

    fn load(&mut self) -> Result<(), std::io::Error> {
        if self.path.exists() {
            let content = fs::read_to_string(&self.path)?;
            self.jobs = serde_json::from_str(&content)?;
        }
        Ok(())
    }

    fn save(&self) -> Result<(), std::io::Error> {
        let content = serde_json::to_string_pretty(&self.jobs)?;
        fs::write(&self.path, content)?;
        Ok(())
    }
}

pub fn normalize_audio(input_path: &Path, temp_dir: &Path) -> Result<PathBuf> {
    let src = File::open(input_path)?;
    let mss = MediaSourceStream::new(Box::new(src), Default::default());
    let mut hint = Hint::new();
    if let Some(extension) = input_path.extension().and_then(|s| s.to_str()) {
        hint.with_extension(extension);
    }
    let meta_opts: MetadataOptions = Default::default();
    let fmt_opts: FormatOptions = Default::default();
    let probed =
        symphonia::default::get_probe().format(&hint, mss, &fmt_opts, &meta_opts)?;
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| anyhow::anyhow!("No supported audio track found"))?;

    let dec_opts: DecoderOptions = Default::default();
    let mut decoder =
        symphonia::default::get_codecs().make(&track.codec_params, &dec_opts)?;
    let track_id = track.id;
    let input_sample_rate = track.codec_params.sample_rate.unwrap();

    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };

    let mut resampler = SincFixedIn::<f32>::new(
        16000 as f64 / input_sample_rate as f64,
        2.0,
        params,
        1024,
        1,
    ).unwrap();

    let spec = WavSpec {
        channels: 1,
        sample_rate: 16000,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };

    let timestamp = Utc::now().format("%Y%m%d%H%M%S");
    let output_filename = format!("normalized_{}.wav", timestamp);
    let output_path = temp_dir.join(output_filename);
    let mut writer = WavWriter::create(&output_path, spec)?;

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(Error::IoError(ref err))
                if err.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(err) => {
                return Err(err.into());
            }
        };

        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(audio_buf) => {
                let mut f32_samples = Vec::new();
                let mut sample_buf = SampleBuffer::new(audio_buf.capacity() as u64, *audio_buf.spec());
                sample_buf.copy_interleaved_ref(audio_buf);
                for sample in sample_buf.samples() {
                    f32_samples.push(*sample);
                }

                let waves_in = vec![f32_samples];
                let waves_out = resampler.process(&waves_in, None).unwrap();

                for sample in waves_out[0].iter() {
                    writer.write_sample((*sample * 32767.0) as i16)?;
                }
            }
            Err(Error::DecodeError(err)) => {
                log::warn!("Decode error: {}", err);
                continue;
            }
            Err(err) => {
                return Err(err.into());
            }
        }
    }

    writer.finalize()?;
    Ok(output_path)
}
