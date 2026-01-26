use crate::config::ApiConfig;
use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use image::DynamicImage;
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::process::Command;
use std::thread;
use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("HTTP request failed: {0}")]
    RequestFailed(String),

    #[error("API returned error: {status} - {message}")]
    ApiError { status: i32, message: String },

    #[error("Failed to encode image: {0}")]
    ImageEncodeError(#[from] image::ImageError),

    #[error("Failed to decode base64: {0}")]
    Base64DecodeError(#[from] base64::DecodeError),

    #[error("Prediction timed out after {0} seconds")]
    Timeout(u64),

    #[error("Prediction failed: {0}")]
    PredictionFailed(String),

    #[error("Unknown backend: {0}")]
    UnknownBackend(String),

    #[error("Missing API key - set REPLICATE_API_KEY env var or api_key in config")]
    MissingApiKey,

    #[error("Missing model version for Replicate backend")]
    MissingModel,

    #[error("ffmpeg failed: {0}")]
    FfmpegFailed(String),

    #[error("No frames extracted from video")]
    NoFramesExtracted,
}

pub struct ApiClient {
    config: ApiConfig,
}

// Replicate API types for fofr/tooncrafter
#[derive(Debug, Serialize)]
struct ReplicateCreatePrediction {
    version: String,
    input: ReplicateInput,
}

#[derive(Debug, Serialize)]
struct ReplicateInput {
    image_1: String,                      // data URI or URL
    image_2: String,                      // data URI or URL
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt: Option<String>,               // optional text prompt
    #[serde(skip_serializing_if = "Option::is_none")]
    max_width: Option<u32>,               // default 512, max 768
    #[serde(skip_serializing_if = "Option::is_none")]
    max_height: Option<u32>,              // default 512, max 768
    #[serde(skip_serializing_if = "Option::is_none")]
    interpolate: Option<bool>,            // enable 2x interpolation with FILM
    #[serde(rename = "loop", skip_serializing_if = "Option::is_none")]
    loop_video: Option<bool>,             // loop the video
    #[serde(skip_serializing_if = "Option::is_none")]
    color_correction: Option<bool>,       // default true
    #[serde(skip_serializing_if = "Option::is_none")]
    seed: Option<i64>,                    // for reproducibility
}

#[derive(Debug, Deserialize)]
struct ReplicatePrediction {
    id: String,
    status: String,
    output: Option<serde_json::Value>, // Can be array of URLs or single URL
    error: Option<String>,
}

// Local/serverless API types
#[derive(Debug, Serialize)]
struct LocalGenerateRequest {
    frame_a: String, // Base64 encoded PNG
    frame_b: String,
    num_frames: u32,
    style_strength: f32,
    resolution: u32,
}

#[derive(Debug, Deserialize)]
struct LocalGenerateResponse {
    frames: Vec<String>, // Base64 encoded PNGs
    #[allow(dead_code)]
    processing_time_ms: Option<u64>,
}

impl ApiClient {
    pub fn new(config: &ApiConfig) -> Result<Self> {
        Ok(Self {
            config: config.clone(),
        })
    }

    /// Generate inbetween frames from two keyframes
    pub fn generate_inbetweens(
        &self,
        frame_a: &DynamicImage,
        frame_b: &DynamicImage,
        num_frames: u32,
    ) -> Result<Vec<DynamicImage>> {
        match self.config.backend.as_str() {
            "replicate" => self.generate_via_replicate(frame_a, frame_b, num_frames),
            "local" | "serverless" => self.generate_via_http(frame_a, frame_b, num_frames),
            other => Err(ApiError::UnknownBackend(other.to_string()).into()),
        }
    }

    fn generate_via_replicate(
        &self,
        frame_a: &DynamicImage,
        frame_b: &DynamicImage,
        num_frames: u32,
    ) -> Result<Vec<DynamicImage>> {
        // Check env var first, then config
        let api_key = std::env::var("REPLICATE_API_KEY")
            .ok()
            .or_else(|| self.config.api_key.clone())
            .ok_or(ApiError::MissingApiKey)?;

        // Encode images as data URIs
        let data_uri_a = self.image_to_data_uri(frame_a)?;
        let data_uri_b = self.image_to_data_uri(frame_b)?;

        log::info!("Creating Replicate prediction (requesting {} frames)", num_frames);

        // Build input - ToonCrafter generates 16 frames as video
        // We'll extract the number of frames the user wants afterward
        let input = ReplicateInput {
            image_1: data_uri_a,
            image_2: data_uri_b,
            prompt: None,
            max_width: Some(512),
            max_height: Some(512),
            interpolate: if num_frames > 8 { Some(true) } else { Some(false) },
            loop_video: Some(false),
            color_correction: Some(true),
            seed: None,
        };

        // Use version field with full hash for community models
        let create_request = ReplicateCreatePrediction {
            version: "0486ff07368e816ec3d5c69b9581e7a09b55817f567a0d74caad9395c9295c77".to_string(),
            input,
        };

        let body = serde_json::to_string(&create_request)?;

        let response = minreq::post("https://api.replicate.com/v1/predictions")
            .with_header("Authorization", format!("Bearer {api_key}"))
            .with_header("Content-Type", "application/json")
            .with_header("Prefer", "wait")  // Wait up to 60s for result
            .with_body(body)
            .with_timeout(self.config.timeout_secs)
            .send()
            .map_err(|e| ApiError::RequestFailed(e.to_string()))?;

        if response.status_code < 200 || response.status_code >= 300 {
            return Err(ApiError::ApiError {
                status: response.status_code,
                message: response.as_str().unwrap_or("").to_string(),
            }
            .into());
        }

        let prediction: ReplicatePrediction = response
            .json()
            .context("Failed to parse Replicate response")?;

        log::info!("Created prediction: {}", prediction.id);

        // Poll for completion
        let poll_url = format!("https://api.replicate.com/v1/predictions/{}", prediction.id);
        let start_time = std::time::Instant::now();
        let timeout = Duration::from_secs(self.config.timeout_secs);

        loop {
            if start_time.elapsed() > timeout {
                return Err(ApiError::Timeout(self.config.timeout_secs).into());
            }

            thread::sleep(Duration::from_secs(2));

            let poll_response = minreq::get(&poll_url)
                .with_header("Authorization", format!("Bearer {api_key}"))
                .with_timeout(30)
                .send()
                .map_err(|e| ApiError::RequestFailed(e.to_string()))?;

            let prediction: ReplicatePrediction = poll_response
                .json()
                .context("Failed to parse poll response")?;

            log::debug!("Prediction status: {}", prediction.status);

            match prediction.status.as_str() {
                "succeeded" => {
                    log::info!("Prediction succeeded");
                    return self.process_output(prediction.output, num_frames);
                }
                "failed" | "canceled" => {
                    let error = prediction.error.unwrap_or_else(|| "Unknown error".to_string());
                    return Err(ApiError::PredictionFailed(error).into());
                }
                _ => continue, // "starting" or "processing"
            }
        }
    }

    /// Process the output from Replicate - could be video URL(s) or image URL(s)
    fn process_output(&self, output: Option<serde_json::Value>, num_frames: u32) -> Result<Vec<DynamicImage>> {
        let output = output.ok_or(ApiError::NoFramesExtracted)?;

        // Output could be:
        // - Array of URLs (video files or images)
        // - Single URL string
        let urls: Vec<String> = match output {
            serde_json::Value::Array(arr) => {
                arr.into_iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            }
            serde_json::Value::String(s) => vec![s],
            _ => return Err(ApiError::NoFramesExtracted.into()),
        };

        if urls.is_empty() {
            return Err(ApiError::NoFramesExtracted.into());
        }

        log::info!("Got {} output URL(s)", urls.len());

        // Check if output is video or images
        let first_url = &urls[0];
        if first_url.contains(".mp4") || first_url.contains("video") {
            // It's a video - download and extract frames
            self.download_video_and_extract_frames(first_url, num_frames)
        } else {
            // It's images - download directly
            self.download_frames(&urls)
        }
    }

    /// Download video and extract frames using ffmpeg
    fn download_video_and_extract_frames(&self, video_url: &str, num_frames: u32) -> Result<Vec<DynamicImage>> {
        log::info!("Downloading video from {}", video_url);

        // Create temp directory for frames
        let temp_dir = std::env::temp_dir().join(format!("gp_inbetween_{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir)?;

        let video_path = temp_dir.join("output.mp4");
        let frames_pattern = temp_dir.join("frame_%04d.png");

        // Download video
        let response = minreq::get(video_url)
            .with_timeout(120)
            .send()
            .map_err(|e| ApiError::RequestFailed(e.to_string()))?;

        std::fs::write(&video_path, response.as_bytes())?;
        log::info!("Video saved to {:?}", video_path);

        // Extract frames with ffmpeg
        // ToonCrafter outputs 16 frames at 8fps = 2 second video
        // We'll extract all frames then select the ones we need
        let ffmpeg_result = Command::new("ffmpeg")
            .args([
                "-i", video_path.to_str().unwrap(),
                "-vsync", "0",
                frames_pattern.to_str().unwrap(),
            ])
            .output();

        let output = ffmpeg_result.map_err(|e| ApiError::FfmpegFailed(format!("Failed to run ffmpeg: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ApiError::FfmpegFailed(format!("ffmpeg failed: {}", stderr)).into());
        }

        // Load extracted frames
        let mut all_frames: Vec<DynamicImage> = Vec::new();
        for i in 1..=100 {  // Max 100 frames
            let frame_path = temp_dir.join(format!("frame_{:04}.png", i));
            if frame_path.exists() {
                let img = image::open(&frame_path)?;
                all_frames.push(img);
            } else {
                break;
            }
        }

        log::info!("Extracted {} frames from video", all_frames.len());

        // Clean up temp files
        let _ = std::fs::remove_dir_all(&temp_dir);

        if all_frames.is_empty() {
            return Err(ApiError::NoFramesExtracted.into());
        }

        // Select evenly spaced frames to match requested count
        // Skip first and last frame (those are the input keyframes)
        let inner_frames: Vec<DynamicImage> = if all_frames.len() > 2 {
            all_frames[1..all_frames.len()-1].to_vec()
        } else {
            all_frames
        };

        if inner_frames.is_empty() {
            return Err(ApiError::NoFramesExtracted.into());
        }

        // If we have more frames than requested, sample evenly
        let selected = if inner_frames.len() as u32 > num_frames {
            let step = inner_frames.len() as f32 / num_frames as f32;
            (0..num_frames)
                .map(|i| {
                    let idx = (i as f32 * step) as usize;
                    inner_frames[idx.min(inner_frames.len() - 1)].clone()
                })
                .collect()
        } else {
            inner_frames
        };

        log::info!("Returning {} frames", selected.len());
        Ok(selected)
    }

    fn generate_via_http(
        &self,
        frame_a: &DynamicImage,
        frame_b: &DynamicImage,
        num_frames: u32,
    ) -> Result<Vec<DynamicImage>> {
        let b64_a = self.image_to_base64(frame_a)?;
        let b64_b = self.image_to_base64(frame_b)?;

        let request = LocalGenerateRequest {
            frame_a: b64_a,
            frame_b: b64_b,
            num_frames,
            style_strength: self.config.style_strength,
            resolution: 1024,
        };

        let body = serde_json::to_string(&request)?;

        let mut req = minreq::post(&self.config.endpoint)
            .with_header("Content-Type", "application/json")
            .with_body(body)
            .with_timeout(self.config.timeout_secs);

        if let Some(api_key) = &self.config.api_key {
            req = req.with_header("Authorization", format!("Bearer {api_key}"));
        }

        let response = req
            .send()
            .map_err(|e| ApiError::RequestFailed(e.to_string()))?;

        if response.status_code < 200 || response.status_code >= 300 {
            return Err(ApiError::ApiError {
                status: response.status_code,
                message: response.as_str().unwrap_or("").to_string(),
            }
            .into());
        }

        let generate_response: LocalGenerateResponse = response
            .json()
            .context("Failed to parse API response")?;

        // Decode frames from base64
        let mut frames = Vec::new();
        for b64_frame in &generate_response.frames {
            let bytes = STANDARD
                .decode(b64_frame)
                .context("Failed to decode base64 frame")?;

            let img =
                image::load_from_memory(&bytes).context("Failed to load image from bytes")?;

            frames.push(img);
        }

        Ok(frames)
    }

    fn download_frames(&self, urls: &[String]) -> Result<Vec<DynamicImage>> {
        let mut frames = Vec::new();

        for url in urls {
            log::debug!("Downloading frame from {}", url);

            let response = minreq::get(url)
                .with_timeout(60)
                .send()
                .map_err(|e| ApiError::RequestFailed(e.to_string()))?;

            let bytes = response.as_bytes();
            let img = image::load_from_memory(bytes)?;
            frames.push(img);
        }

        Ok(frames)
    }

    fn image_to_base64(&self, img: &DynamicImage) -> Result<String> {
        let mut buf = Vec::new();
        img.write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Png)?;
        Ok(STANDARD.encode(&buf))
    }

    fn image_to_data_uri(&self, img: &DynamicImage) -> Result<String> {
        let b64 = self.image_to_base64(img)?;
        Ok(format!("data:image/png;base64,{b64}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_to_base64() {
        let config = ApiConfig {
            backend: "local".to_string(),
            endpoint: "http://localhost:8000".to_string(),
            api_key: None,
            replicate_model: None,
            style_strength: 0.8,
            timeout_secs: 60,
        };

        let client = ApiClient::new(&config).unwrap();
        let img = DynamicImage::new_rgba8(10, 10);
        let b64 = client.image_to_base64(&img).unwrap();
        assert!(!b64.is_empty());
    }
}
