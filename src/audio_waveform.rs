// src/audio_waveform.rs
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use std::fs::File;
use std::path::Path;

pub struct AudioWaveform {
    samples: Vec<f32>,
    sample_rate: u32,
    channels: usize,
}

impl AudioWaveform {
    pub fn from_file(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let file = File::open(path)?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        
        let hint = Hint::new();
        let meta_opts: MetadataOptions = Default::default();
        let fmt_opts: FormatOptions = Default::default();
        
        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &fmt_opts, &meta_opts)?;
        
        let mut format = probed.format;
        
        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
            .ok_or("no supported audio tracks")?;
        
        let dec_opts: DecoderOptions = Default::default();
        let mut decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &dec_opts)?;
        
        let track_id = track.id;
        let sample_rate = track.codec_params.sample_rate.unwrap_or(44100);
        let channels = track.codec_params.channels.map(|c| c.count()).unwrap_or(2);
        
        let mut samples = Vec::new();
        let mut sample_buf = None;
        
        // Читаем аудио данные
        while let Ok(packet) = format.next_packet() {
            if packet.track_id() != track_id {
                continue;
            }
            
            match decoder.decode(&packet) {
                Ok(decoded) => {
                    if sample_buf.is_none() {
                        let spec = *decoded.spec();
                        let duration = decoded.capacity() as u64;
                        sample_buf = Some(SampleBuffer::<f32>::new(duration, spec));
                    }
                    
                    if let Some(buf) = &mut sample_buf {
                        buf.copy_interleaved_ref(decoded);
                        samples.extend_from_slice(buf.samples());
                    }
                }
                Err(Error::DecodeError(_)) => continue,
                Err(_) => break,
            }
        }
        
        Ok(Self {
            samples,
            sample_rate,
            channels,
        })
    }
    
    pub fn get_peaks(&self, num_peaks: usize) -> Vec<f32> {
        if self.samples.is_empty() || num_peaks == 0 {
            return vec![];
        }
        
        let samples_per_peak = self.samples.len() / num_peaks;
        let mut peaks = Vec::with_capacity(num_peaks);
        
        for i in 0..num_peaks {
            let start = i * samples_per_peak;
            let end = ((i + 1) * samples_per_peak).min(self.samples.len());
            
            let peak = self.samples[start..end]
                .iter()
                .map(|s| s.abs())
                .fold(0.0f32, |a, b| a.max(b));
            
            peaks.push(peak);
        }
        
        peaks
    }
    
    pub fn get_duration(&self) -> f32 {
        self.samples.len() as f32 / (self.sample_rate as f32 * self.channels as f32)
    }
}