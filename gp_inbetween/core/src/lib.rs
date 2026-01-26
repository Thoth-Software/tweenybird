pub mod api;
pub mod config;
pub mod confidence;
pub mod feedback;
pub mod preprocessing;

pub use api::ApiClient;
pub use config::Config;
pub use confidence::{ConfidenceScorer, detect_motion_type};
pub use feedback::{FeedbackLogger, Statistics};
pub use preprocessing::{PaddingInfo, Preprocessor};

use anyhow::Result;
use image::{DynamicImage, GenericImageView};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Main generator struct that orchestrates the entire workflow
pub struct Generator {
    config: Config,
    api_client: ApiClient,
    preprocessor: Preprocessor,
    confidence_scorer: ConfidenceScorer,
    feedback_logger: FeedbackLogger,
}

impl Generator {
    pub fn new(config: Config) -> Result<Self> {
        let api_client = ApiClient::new(&config.api)?;
        let preprocessor = Preprocessor::new(&config.preprocessing);
        let confidence_scorer = ConfidenceScorer::new(config.auto_accept_threshold);
        let feedback_logger = FeedbackLogger::new()?;

        Ok(Self {
            config,
            api_client,
            preprocessor,
            confidence_scorer,
            feedback_logger,
        })
    }

    /// Generate inbetween frames from two keyframes
    pub fn generate_inbetweens(
        &self,
        frame_a_path: &Path,
        frame_b_path: &Path,
        num_frames: u32,
        character: Option<&str>,
        motion_type: Option<&str>,
    ) -> Result<GenerationResult> {
        log::info!(
            "Generating {} inbetweens between {:?} and {:?}",
            num_frames,
            frame_a_path,
            frame_b_path
        );

        // 1. Load images
        let img_a = image::open(frame_a_path)?;
        let img_b = image::open(frame_b_path)?;

        // Store original dimensions for potential restoration
        let (orig_width, orig_height) = img_a.dimensions();
        let padding_info = self.preprocessor.get_padding_info(orig_width, orig_height);

        // 2. Preprocess
        let cleaned_a = self.preprocessor.process(&img_a)?;
        let cleaned_b = self.preprocessor.process(&img_b)?;

        // 3. Auto-detect motion type if not provided
        let detected_motion = motion_type
            .map(String::from)
            .unwrap_or_else(|| detect_motion_type(&cleaned_a, &cleaned_b));

        log::info!("Motion type: {}", detected_motion);

        // 4. Call API
        let generated = self
            .api_client
            .generate_inbetweens(&cleaned_a, &cleaned_b, num_frames)?;

        log::info!("API returned {} frames", generated.len());

        // 5. Score confidence for each frame
        let mut scored_frames = Vec::new();
        for (i, frame) in generated.into_iter().enumerate() {
            let score = self.confidence_scorer.score_frame(
                &frame,
                &cleaned_a,
                &cleaned_b,
                &detected_motion,
                character,
            )?;

            log::debug!("Frame {} confidence: {:.2}", i, score);

            // Optionally restore original dimensions
            let final_frame = if self.config.preprocessing.normalize_resolution {
                self.preprocessor.restore_original_size(
                    &frame,
                    &padding_info,
                    orig_width,
                    orig_height,
                )
            } else {
                frame
            };

            scored_frames.push(ScoredFrame {
                frame: final_frame,
                score,
                auto_accept: self.confidence_scorer.should_auto_accept(score),
            });
        }

        // 6. Log generation
        self.feedback_logger.log_generation(
            character.unwrap_or("unknown"),
            &detected_motion,
            num_frames,
        )?;

        Ok(GenerationResult {
            frames: scored_frames,
            metadata: GenerationMetadata {
                character: character.map(String::from),
                motion_type: Some(detected_motion),
                auto_accept_threshold: self.config.auto_accept_threshold,
                original_width: orig_width,
                original_height: orig_height,
            },
        })
    }

    /// Log acceptance of a frame
    pub fn accept_frame(
        &self,
        frame_number: u32,
        character: &str,
        motion_type: &str,
        auto: bool,
        confidence: Option<f32>,
    ) -> Result<()> {
        self.feedback_logger
            .log_acceptance(frame_number, character, motion_type, auto, confidence)
    }

    /// Log rejection of a frame
    pub fn reject_frame(
        &self,
        frame_number: u32,
        character: &str,
        motion_type: &str,
        issues: &[String],
        confidence: Option<f32>,
    ) -> Result<()> {
        self.feedback_logger
            .log_rejection(frame_number, character, motion_type, issues, confidence)
    }

    /// Get statistics from the feedback log
    pub fn get_stats(
        &self,
        character: Option<&str>,
        motion_type: Option<&str>,
    ) -> Result<Statistics> {
        self.feedback_logger.get_stats(character, motion_type)
    }
}

/// A frame with its confidence score
#[derive(Debug)]
pub struct ScoredFrame {
    pub frame: DynamicImage,
    pub score: f32,
    pub auto_accept: bool,
}

/// Result of a generation operation
#[derive(Debug)]
pub struct GenerationResult {
    pub frames: Vec<ScoredFrame>,
    pub metadata: GenerationMetadata,
}

/// Metadata about a generation
#[derive(Debug, Serialize, Deserialize)]
pub struct GenerationMetadata {
    pub character: Option<String>,
    pub motion_type: Option<String>,
    pub auto_accept_threshold: f32,
    pub original_width: u32,
    pub original_height: u32,
}

/// Output metadata written to JSON file
#[derive(Debug, Serialize, Deserialize)]
pub struct OutputMetadata {
    pub character: Option<String>,
    pub motion_type: Option<String>,
    pub confidence_scores: Vec<f32>,
    pub auto_accept: Vec<bool>,
    pub auto_accept_threshold: f32,
}

impl From<&GenerationResult> for OutputMetadata {
    fn from(result: &GenerationResult) -> Self {
        Self {
            character: result.metadata.character.clone(),
            motion_type: result.metadata.motion_type.clone(),
            confidence_scores: result.frames.iter().map(|f| f.score).collect(),
            auto_accept: result.frames.iter().map(|f| f.auto_accept).collect(),
            auto_accept_threshold: result.metadata.auto_accept_threshold,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_metadata_conversion() {
        let result = GenerationResult {
            frames: vec![
                ScoredFrame {
                    frame: DynamicImage::new_rgba8(10, 10),
                    score: 0.9,
                    auto_accept: true,
                },
                ScoredFrame {
                    frame: DynamicImage::new_rgba8(10, 10),
                    score: 0.7,
                    auto_accept: false,
                },
            ],
            metadata: GenerationMetadata {
                character: Some("hero".to_string()),
                motion_type: Some("walk".to_string()),
                auto_accept_threshold: 0.85,
                original_width: 800,
                original_height: 600,
            },
        };

        let output: OutputMetadata = (&result).into();
        assert_eq!(output.confidence_scores.len(), 2);
        assert_eq!(output.auto_accept, vec![true, false]);
    }
}
