use ffmpeg_next::{
    format::{input, Pixel},
    media::Type,
    software::scaling::{context::Context, flag::Flags},
    util::frame::video::Video,
};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use rodio::{Decoder, OutputStream, Sink, Source};
use std::fs::File;
use std::io::BufReader;

#[derive(Debug, Clone)]
pub struct VideoInfo {
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub duration: Duration,
    pub has_audio: bool,
}

#[derive(Debug, Clone)]
pub struct VideoFrame {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub timestamp: Duration,
}

pub struct FFmpegDecoder {
    input: ffmpeg_next::format::context::Input,
    video_stream_index: usize,
    audio_stream_index: Option<usize>,
    video_decoder: ffmpeg_next::decoder::Video,
    audio_decoder: Option<ffmpeg_next::decoder::Audio>,
    scaler: Context,
    current_frame: Option<VideoFrame>,
    audio_sink: Option<Arc<Mutex<Sink>>>,
    _stream: Option<OutputStream>,
}

impl FFmpegDecoder {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        ffmpeg_next::init().map_err(|e| format!("FFmpeg init error: {:?}", e))?;
        
        let input = input(&path).map_err(|e| format!("Failed to open input: {:?}", e))?;
        
        let video_stream = input
            .streams()
            .best(Type::Video)
            .ok_or("No video stream found")?;
        let video_stream_index = video_stream.index();
        
        let audio_stream = input.streams().best(Type::Audio);
        let audio_stream_index = audio_stream.as_ref().map(|s| s.index());
        
        let video_context_decoder = ffmpeg_next::codec::context::Context::from_parameters(video_stream.parameters())
            .map_err(|e| format!("Failed to create video context: {:?}", e))?;
        let video_decoder = video_context_decoder.decoder().video()
            .map_err(|e| format!("Failed to create video decoder: {:?}", e))?;
        
        let audio_decoder = if let Some(audio_stream) = audio_stream {
            match ffmpeg_next::codec::context::Context::from_parameters(audio_stream.parameters()) {
                Ok(audio_context_decoder) => {
                    match audio_context_decoder.decoder().audio() {
                        Ok(decoder) => Some(decoder),
                        Err(_) => None,
                    }
                }
                Err(_) => None,
            }
        } else {
            None
        };
        
        let scaler = Context::get(
            video_decoder.format(),
            video_decoder.width(),
            video_decoder.height(),
            Pixel::RGBA,
            video_decoder.width(),
            video_decoder.height(),
            Flags::BILINEAR,
        ).map_err(|e| format!("Failed to create scaler: {:?}", e))?;
        
        // Initialize audio
        let (_stream, audio_sink) = if audio_decoder.is_some() {
            match OutputStream::try_default() {
                Ok((stream, handle)) => {
                    match Sink::try_new(&handle) {
                        Ok(sink) => (Some(stream), Some(Arc::new(Mutex::new(sink)))),
                        Err(_) => (None, None)
                    }
                }
                Err(_) => (None, None)
            }
        } else {
            (None, None)
        };
        
        Ok(FFmpegDecoder {
            input,
            video_stream_index,
            audio_stream_index,
            video_decoder,
            audio_decoder,
            scaler,
            current_frame: None,
            audio_sink,
            _stream,
        })
    }
    
    pub fn get_video_info(&self) -> VideoInfo {
        let stream = &self.input.streams().nth(self.video_stream_index).unwrap();
        let duration = if stream.duration() > 0 {
            let time_base: f64 = stream.time_base().into();
            Duration::from_secs_f64(stream.duration() as f64 * time_base)
        } else {
            Duration::from_micros(self.input.duration() as u64)
        };
        
        let fps: f64 = stream.avg_frame_rate().into();
        
        VideoInfo {
            width: self.video_decoder.width(),
            height: self.video_decoder.height(),
            fps,
            duration,
            has_audio: self.audio_stream_index.is_some(),
        }
    }
    
    pub fn seek_to_time(&mut self, time: Duration) -> Result<(), Box<dyn std::error::Error>> {
        let stream = &self.input.streams().nth(self.video_stream_index).unwrap();
        let time_base: f64 = stream.time_base().into();
        let timestamp = (time.as_secs_f64() / time_base) as i64;
        
        self.input.seek(timestamp, ..timestamp)
            .map_err(|e| format!("Seek failed: {:?}", e))?;
        
        Ok(())
    }
    
    pub fn read_frame(&mut self) -> Option<VideoFrame> {
        let mut frame = Video::empty();
        let mut decoded = false;
        let video_stream_index = self.video_stream_index;
        
        // Get the time base before borrowing self.input mutably
        let time_base: f64 = {
            let stream = self.input.streams().nth(video_stream_index).unwrap();
            stream.time_base().into()
        };
        
        for (stream, packet) in self.input.packets() {
            if stream.index() == video_stream_index {
                if self.video_decoder.send_packet(&packet).is_ok() {
                    while self.video_decoder.receive_frame(&mut frame).is_ok() {
                        let mut rgb_frame = Video::empty();
                        if self.scaler.run(&frame, &mut rgb_frame).is_ok() {
                            let data = rgb_frame.data(0).to_vec();
                            
                            let timestamp = if frame.timestamp().is_some() {
                                Duration::from_secs_f64(frame.timestamp().unwrap() as f64 * time_base)
                            } else {
                                Duration::from_secs(0)
                            };
                            
                            let video_frame = VideoFrame {
                                data,
                                width: rgb_frame.width(),
                                height: rgb_frame.height(),
                                timestamp,
                            };
                            
                            self.current_frame = Some(video_frame.clone());
                            decoded = true;
                            break;
                        }
                    }
                }
            }
            
            if decoded {
                break;
            }
        }
        
        self.current_frame.clone()
    }
    
    pub fn get_current_frame(&self) -> Option<&VideoFrame> {
        self.current_frame.as_ref()
    }
    
    pub fn play_audio(&self) {
        if let Some(sink) = &self.audio_sink {
            if let Ok(sink) = sink.lock() {
                sink.play();
            }
        }
    }
    
    pub fn pause_audio(&self) {
        if let Some(sink) = &self.audio_sink {
            if let Ok(sink) = sink.lock() {
                sink.pause();
            }
        }
    }
    
    pub fn stop_audio(&self) {
        if let Some(sink) = &self.audio_sink {
            if let Ok(sink) = sink.lock() {
                sink.stop();
            }
        }
    }
    
    pub fn is_audio_playing(&self) -> bool {
        if let Some(sink) = &self.audio_sink {
            if let Ok(sink) = sink.lock() {
                return !sink.is_paused();
            }
        }
        false
    }
}

pub fn load_audio_from_video<P: AsRef<Path>>(path: P) -> Result<Box<dyn Source<Item = f32> + Send>, Box<dyn std::error::Error>> {
    // For now, we'll try to load audio using rodio's built-in decoders
    // In a more complete implementation, we'd extract audio using FFmpeg
    let file = File::open(path)?;
    let source = Decoder::new(BufReader::new(file))?;
    Ok(Box::new(source.convert_samples()))
}