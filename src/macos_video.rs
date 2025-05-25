// src/macos_video.rs
use cocoa::base::{id, nil};
use cocoa::foundation::{NSAutoreleasePool, NSString, NSURL};
use core_foundation::base::TCFType;
use core_graphics::color_space::CGColorSpace;
use core_graphics::context::CGContext;
use core_graphics::image::CGImageAlphaInfo;
use objc::{class, msg_send, sel, sel_impl};
use objc::runtime::{Object, Sel};
use std::ffi::c_void;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[link(name = "AVFoundation", kind = "framework")]
#[link(name = "CoreMedia", kind = "framework")]
#[link(name = "CoreVideo", kind = "framework")]
extern "C" {}

pub struct VideoFrame {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub timestamp: Duration,
}

pub struct MacOSVideoDecoder {
    asset: id,
    asset_reader: id,
    video_track: id,
    video_output: id,
    duration: Duration,
    width: u32,
    height: u32,
    fps: f64,
}

impl MacOSVideoDecoder {
    pub fn new(path: &Path) -> Result<Self, String> {
        unsafe {
            let pool = NSAutoreleasePool::new(nil);
            
            // Create NSURL from path
            let path_str = path.to_str().ok_or("Invalid path")?;
            let ns_path = NSString::alloc(nil).init_str(path_str);
            let url: id = msg_send![class!(NSURL), fileURLWithPath:ns_path];
            
            // Create AVAsset
            let asset: id = msg_send![class!(AVURLAsset), assetWithURL:url];
            if asset == nil {
                return Err("Failed to create AVAsset".to_string());
            }
            
            // Get video track
            let tracks_key = NSString::alloc(nil).init_str("tracks");
            let _: () = msg_send![asset, loadValuesAsynchronouslyForKeys:[tracks_key] completionHandler:nil];
            
            // Wait for loading (simplified - in production use proper async handling)
            std::thread::sleep(std::time::Duration::from_millis(100));
            
            let video_tracks: id = msg_send![asset, tracksWithMediaType:AVMediaTypeVideo()];
            let count: usize = msg_send![video_tracks, count];
            
            if count == 0 {
                return Err("No video tracks found".to_string());
            }
            
            let video_track: id = msg_send![video_tracks, objectAtIndex:0];
            
            // Get video properties
            let natural_size: CGSize = msg_send![video_track, naturalSize];
            let width = natural_size.width as u32;
            let height = natural_size.height as u32;
            
            let frame_rate: f32 = msg_send![video_track, nominalFrameRate];
            let fps = frame_rate as f64;
            
            let duration_value: CMTime = msg_send![asset, duration];
            let duration_seconds = CMTimeGetSeconds(duration_value);
            let duration = Duration::from_secs_f64(duration_seconds);
            
            // Create asset reader
            let mut error: id = nil;
            let asset_reader: id = msg_send![class!(AVAssetReader), alloc];
            let asset_reader: id = msg_send![asset_reader, initWithAsset:asset error:&mut error];
            
            if asset_reader == nil || error != nil {
                return Err("Failed to create asset reader".to_string());
            }
            
            // Configure video output settings
            let pixel_format_key = NSString::alloc(nil).init_str("kCVPixelBufferPixelFormatTypeKey");
            let pixel_format_value: u32 = kCVPixelFormatType_32BGRA;
            let settings: id = msg_send![class!(NSDictionary), dictionaryWithObject:pixel_format_value forKey:pixel_format_key];
            
            let video_output: id = msg_send![class!(AVAssetReaderTrackOutput), alloc];
            let video_output: id = msg_send![video_output, initWithTrack:video_track outputSettings:settings];
            
            let _: () = msg_send![asset_reader, addOutput:video_output];
            let _: () = msg_send![asset_reader, startReading];
            
            let _: () = msg_send![pool, drain];
            
            Ok(Self {
                asset,
                asset_reader,
                video_track,
                video_output,
                duration,
                width,
                height,
                fps,
            })
        }
    }
    
    pub fn seek_to_time(&mut self, time: Duration) -> Result<(), String> {
        unsafe {
            let pool = NSAutoreleasePool::new(nil);
            
            // Stop current reading session
            let _: () = msg_send![self.asset_reader, cancelReading];
            
            // Create new asset reader at specified time
            let mut error: id = nil;
            let asset_reader: id = msg_send![class!(AVAssetReader), alloc];
            let asset_reader: id = msg_send![asset_reader, initWithAsset:self.asset error:&mut error];
            
            if asset_reader == nil || error != nil {
                return Err("Failed to create asset reader for seek".to_string());
            }
            
            // Configure time range
            let seek_time = CMTimeMakeWithSeconds(time.as_secs_f64(), 600);
            let duration = CMTimeMakeWithSeconds(self.duration.as_secs_f64(), 600);
            let time_range = CMTimeRangeMake(seek_time, duration);
            
            let _: () = msg_send![asset_reader, setTimeRange:time_range];
            
            // Recreate video output
            let pixel_format_key = NSString::alloc(nil).init_str("kCVPixelBufferPixelFormatTypeKey");
            let pixel_format_value: u32 = kCVPixelFormatType_32BGRA;
            let settings: id = msg_send![class!(NSDictionary), dictionaryWithObject:pixel_format_value forKey:pixel_format_key];
            
            let video_output: id = msg_send![class!(AVAssetReaderTrackOutput), alloc];
            let video_output: id = msg_send![video_output, initWithTrack:self.video_track outputSettings:settings];
            
            let _: () = msg_send![asset_reader, addOutput:video_output];
            let _: () = msg_send![asset_reader, startReading];
            
            self.asset_reader = asset_reader;
            self.video_output = video_output;
            
            let _: () = msg_send![pool, drain];
            
            Ok(())
        }
    }
    
    pub fn read_frame(&mut self) -> Option<VideoFrame> {
        unsafe {
            let pool = NSAutoreleasePool::new(nil);
            
            let sample_buffer: id = msg_send![self.video_output, copyNextSampleBuffer];
            if sample_buffer == nil {
                let _: () = msg_send![pool, drain];
                return None;
            }
            
            // Get image buffer from sample
            let image_buffer: CVImageBufferRef = CMSampleBufferGetImageBuffer(sample_buffer);
            if image_buffer.is_null() {
                CFRelease(sample_buffer as *const c_void);
                let _: () = msg_send![pool, drain];
                return None;
            }
            
            // Lock the base address of the pixel buffer
            CVPixelBufferLockBaseAddress(image_buffer, 0);
            
            let width = CVPixelBufferGetWidth(image_buffer) as u32;
            let height = CVPixelBufferGetHeight(image_buffer) as u32;
            let bytes_per_row = CVPixelBufferGetBytesPerRow(image_buffer);
            let base_address = CVPixelBufferGetBaseAddress(image_buffer);
            
            // Get timestamp
            let presentation_time = CMSampleBufferGetPresentationTimeStamp(sample_buffer);
            let timestamp_seconds = CMTimeGetSeconds(presentation_time);
            let timestamp = Duration::from_secs_f64(timestamp_seconds);
            
            // Copy pixel data
            let data_size = (height as usize) * bytes_per_row;
            let mut data = vec![0u8; data_size];
            
            if !base_address.is_null() {
                std::ptr::copy_nonoverlapping(
                    base_address as *const u8,
                    data.as_mut_ptr(),
                    data_size
                );
            }
            
            // Unlock pixel buffer
            CVPixelBufferUnlockBaseAddress(image_buffer, 0);
            
            // Release sample buffer
            CFRelease(sample_buffer as *const c_void);
            
            let _: () = msg_send![pool, drain];
            
            Some(VideoFrame {
                data,
                width,
                height,
                timestamp,
            })
        }
    }
    
    pub fn get_video_info(&self) -> (u32, u32, f64, Duration) {
        (self.width, self.height, self.fps, self.duration)
    }
}

impl Drop for MacOSVideoDecoder {
    fn drop(&mut self) {
        unsafe {
            if self.asset_reader != nil {
                let _: () = msg_send![self.asset_reader, cancelReading];
            }
        }
    }
}

// Helper functions and types
#[repr(C)]
struct CGSize {
    width: f64,
    height: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CMTime {
    value: i64,
    timescale: i32,
    flags: u32,
    epoch: i64,
}

#[repr(C)]
struct CMTimeRange {
    start: CMTime,
    duration: CMTime,
}

type CVImageBufferRef = *mut c_void;
type CVPixelBufferRef = CVImageBufferRef;

const kCVPixelFormatType_32BGRA: u32 = 0x42475241; // 'BGRA'

extern "C" {
    fn CMTimeGetSeconds(time: CMTime) -> f64;
    fn CMTimeMakeWithSeconds(seconds: f64, timescale: i32) -> CMTime;
    fn CMTimeRangeMake(start: CMTime, duration: CMTime) -> CMTimeRange;
    fn CMSampleBufferGetImageBuffer(sample_buffer: id) -> CVImageBufferRef;
    fn CMSampleBufferGetPresentationTimeStamp(sample_buffer: id) -> CMTime;
    fn CVPixelBufferLockBaseAddress(pixel_buffer: CVPixelBufferRef, lock_flags: u64) -> i32;
    fn CVPixelBufferUnlockBaseAddress(pixel_buffer: CVPixelBufferRef, unlock_flags: u64) -> i32;
    fn CVPixelBufferGetWidth(pixel_buffer: CVPixelBufferRef) -> usize;
    fn CVPixelBufferGetHeight(pixel_buffer: CVPixelBufferRef) -> usize;
    fn CVPixelBufferGetBytesPerRow(pixel_buffer: CVPixelBufferRef) -> usize;
    fn CVPixelBufferGetBaseAddress(pixel_buffer: CVPixelBufferRef) -> *mut c_void;
    fn CFRelease(cf: *const c_void);
}

fn AVMediaTypeVideo() -> id {
    unsafe {
        let av_media_type_video: id = msg_send![class!(AVMediaType), video];
        av_media_type_video
    }
}

// Frame converter for egui
pub fn convert_frame_to_rgba(frame: &VideoFrame) -> Vec<u8> {
    let mut rgba_data = Vec::with_capacity((frame.width * frame.height * 4) as usize);
    
    // Convert BGRA to RGBA
    for chunk in frame.data.chunks_exact(4) {
        rgba_data.push(chunk[2]); // R
        rgba_data.push(chunk[1]); // G
        rgba_data.push(chunk[0]); // B
        rgba_data.push(chunk[3]); // A
    }
    
    rgba_data
}
