use crate::config::ApiConfig;
use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use image::DynamicImage;
use serde::{Deserialize, Serialize};
use std::io::Cursor;
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

    #[error("Missing API key for Replicate backend")]
    MissingApiKey,

    #[error("Missing model version for Replicate backend")]
    MissingModel,
}

pub struct ApiClient {
    config: ApiConfig,
}

// Replicate API types
#[derive(Debug, Serialize)]
struct ReplicateCreatePrediction {
    version: String,
    input: ReplicateInput,
}

#[derive(Debug, Serialize)]
struct ReplicateInput {
    image_1: String, // data URI: "data:image/png;base64,..."
    image_2: String,
    interpolation_steps: u32,
}

#[derive(Debug, Deserialize)]
struct ReplicatePrediction {
    id: String,
    status: String,
    output: Option<Vec<String>>, // URLs to generated images
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
        let api_key = self
            .config
            .api_key
            .as_ref()
            .ok_or(ApiError::MissingApiKey)?;

        let model_version = self
            .config
            .replicate_model
            .as_ref()
            .ok_or(ApiError::MissingModel)?;

        // Encode images as data URIs
        let data_uri_a = self.image_to_data_uri(frame_a)?;
        let data_uri_b = self.image_to_data_uri(frame_b)?;

        log::info!("Creating Replicate prediction with {} frames", num_frames);

        // Create prediction
        let create_request = ReplicateCreatePrediction {
            version: model_version.clone(),
            input: ReplicateInput {
                image_1: data_uri_a,
                image_2: data_uri_b,
                interpolation_steps: num_frames,
            },
        };

        let body = serde_json::to_string(&create_request)?;

        let response = minreq::post("https://api.replicate.com/v1/predictions")
            .with_header("Authorization", format!("Bearer {api_key}"))
            .with_header("Content-Type", "application/json")
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
                    let output_urls = prediction.output.unwrap_or_default();
                    log::info!("Prediction succeeded with {} output frames", output_urls.len());
                    return self.download_frames(&output_urls);
                }
                "failed" | "canceled" => {
                    let error = prediction.error.unwrap_or_else(|| "Unknown error".to_string());
                    return Err(ApiError::PredictionFailed(error).into());
                }
                _ => continue, // "starting" or "processing"
            }
        }
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
