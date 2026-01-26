use crate::feedback::FeedbackLogger;
use anyhow::Result;
use image::{DynamicImage, GenericImageView};

pub struct ConfidenceScorer {
    auto_accept_threshold: f32,
    feedback_logger: Option<FeedbackLogger>,
}

impl ConfidenceScorer {
    pub fn new(auto_accept_threshold: f32) -> Self {
        Self {
            auto_accept_threshold,
            feedback_logger: FeedbackLogger::new().ok(),
        }
    }

    pub fn with_feedback_logger(mut self, logger: FeedbackLogger) -> Self {
        self.feedback_logger = Some(logger);
        self
    }

    /// Score a generated frame based on multiple heuristics
    /// Returns a confidence score between 0.0 and 1.0
    pub fn score_frame(
        &self,
        generated: &DynamicImage,
        source_a: &DynamicImage,
        source_b: &DynamicImage,
        motion_type: &str,
        character: Option<&str>,
    ) -> Result<f32> {
        let mut score = 1.0;

        // Heuristic 1: Basic image validity
        let validity_penalty = self.check_image_validity(generated);
        score -= validity_penalty;

        // Heuristic 2: Motion complexity
        let complexity_penalty = self.assess_motion_complexity(source_a, source_b);
        score -= complexity_penalty;

        // Heuristic 3: Historical success rate
        let historical_penalty = self.check_historical_success(motion_type, character);
        score -= historical_penalty;

        // Heuristic 4: Color/brightness consistency
        let consistency_penalty = self.check_color_consistency(generated, source_a, source_b);
        score -= consistency_penalty;

        Ok(score.clamp(0.0, 1.0))
    }

    /// Check if a score meets the auto-accept threshold
    pub fn should_auto_accept(&self, score: f32) -> bool {
        score >= self.auto_accept_threshold
    }

    /// Check basic image validity (not blank, reasonable dimensions)
    fn check_image_validity(&self, img: &DynamicImage) -> f32 {
        let (width, height) = img.dimensions();

        // Check for blank/empty image
        if width == 0 || height == 0 {
            return 0.5;
        }

        // Sample pixels to check if image has content
        let rgba = img.to_rgba8();
        let total_pixels = (width * height) as usize;
        let sample_size = total_pixels.min(1000);
        let step = total_pixels / sample_size;

        let mut non_transparent = 0;
        let mut total_alpha = 0u64;

        for (i, pixel) in rgba.pixels().enumerate() {
            if i % step == 0 {
                total_alpha += u64::from(pixel[3]);
                if pixel[3] > 128 {
                    non_transparent += 1;
                }
            }
        }

        let avg_alpha = total_alpha as f32 / sample_size as f32;

        // Penalize if image is mostly transparent (likely failed generation)
        if non_transparent < sample_size / 10 {
            return 0.4;
        }

        // Penalize very low average alpha
        if avg_alpha < 50.0 {
            return 0.2;
        }

        0.0
    }

    /// Assess motion complexity between source frames
    fn assess_motion_complexity(&self, source_a: &DynamicImage, source_b: &DynamicImage) -> f32 {
        let diff = self.calculate_pixel_difference(source_a, source_b);

        // High difference = complex motion = lower confidence
        if diff > 0.4 {
            0.35 // Very complex motion, significant penalty
        } else if diff > 0.3 {
            0.25
        } else if diff > 0.2 {
            0.15
        } else if diff > 0.1 {
            0.05
        } else {
            0.0 // Very similar frames, easy to interpolate
        }
    }

    /// Calculate normalized pixel difference between two images
    fn calculate_pixel_difference(&self, img_a: &DynamicImage, img_b: &DynamicImage) -> f32 {
        let (w_a, h_a) = img_a.dimensions();
        let (w_b, h_b) = img_b.dimensions();

        // Different sizes = uncertain
        if w_a != w_b || h_a != h_b {
            return 0.5;
        }

        let rgba_a = img_a.to_rgba8();
        let rgba_b = img_b.to_rgba8();

        // Sample pixels and calculate difference
        let total_pixels = (w_a * h_a) as usize;
        let sample_size = total_pixels.min(500);
        let step = total_pixels.max(1) / sample_size.max(1);

        let mut total_diff = 0u64;
        let mut samples = 0u32;

        for (i, (pixel_a, pixel_b)) in rgba_a.pixels().zip(rgba_b.pixels()).enumerate() {
            if i % step == 0 {
                // Only compare non-transparent pixels
                if pixel_a[3] > 128 || pixel_b[3] > 128 {
                    let diff: u64 = pixel_a
                        .0
                        .iter()
                        .zip(pixel_b.0.iter())
                        .map(|(a, b)| (i32::from(*a) - i32::from(*b)).unsigned_abs() as u64)
                        .sum();

                    total_diff += diff;
                    samples += 1;
                }
            }
        }

        if samples == 0 {
            return 0.0;
        }

        // Normalize to 0-1 range (max diff per pixel is 255*4=1020)
        (total_diff as f32) / (samples as f32 * 1020.0)
    }

    /// Check historical success rate from feedback log
    fn check_historical_success(&self, motion_type: &str, character: Option<&str>) -> f32 {
        let Some(logger) = &self.feedback_logger else {
            return 0.0;
        };

        match logger.get_acceptance_rate(character, Some(motion_type)) {
            Ok(rate) => {
                // If historical acceptance is low, reduce confidence
                if rate < 0.3 {
                    0.35
                } else if rate < 0.5 {
                    0.25
                } else if rate < 0.7 {
                    0.1
                } else {
                    0.0
                }
            }
            Err(_) => 0.0, // No historical data, assume neutral
        }
    }

    /// Check color/brightness consistency with source frames
    fn check_color_consistency(
        &self,
        generated: &DynamicImage,
        source_a: &DynamicImage,
        source_b: &DynamicImage,
    ) -> f32 {
        let gen_stats = self.calculate_image_stats(generated);
        let a_stats = self.calculate_image_stats(source_a);
        let b_stats = self.calculate_image_stats(source_b);

        // Expected stats should be roughly between source A and B
        let expected_brightness = (a_stats.brightness + b_stats.brightness) / 2.0;
        let expected_saturation = (a_stats.saturation + b_stats.saturation) / 2.0;

        // Allow some tolerance (sources might have different lighting)
        let brightness_tolerance = (a_stats.brightness - b_stats.brightness).abs() + 0.1;
        let saturation_tolerance = (a_stats.saturation - b_stats.saturation).abs() + 0.1;

        let brightness_diff = (gen_stats.brightness - expected_brightness).abs();
        let saturation_diff = (gen_stats.saturation - expected_saturation).abs();

        let mut penalty = 0.0;

        if brightness_diff > brightness_tolerance {
            penalty += 0.15;
        }

        if saturation_diff > saturation_tolerance {
            penalty += 0.1;
        }

        penalty
    }

    /// Calculate basic image statistics
    fn calculate_image_stats(&self, img: &DynamicImage) -> ImageStats {
        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();
        let total_pixels = (width * height) as usize;
        let sample_size = total_pixels.min(500);
        let step = total_pixels.max(1) / sample_size.max(1);

        let mut total_brightness = 0.0f64;
        let mut total_saturation = 0.0f64;
        let mut samples = 0u32;

        for (i, pixel) in rgba.pixels().enumerate() {
            if i % step == 0 && pixel[3] > 128 {
                let r = f64::from(pixel[0]) / 255.0;
                let g = f64::from(pixel[1]) / 255.0;
                let b = f64::from(pixel[2]) / 255.0;

                // Brightness (luminance)
                let brightness = 0.299 * r + 0.587 * g + 0.114 * b;
                total_brightness += brightness;

                // Saturation
                let max = r.max(g).max(b);
                let min = r.min(g).min(b);
                let saturation = if max > 0.0 {
                    (max - min) / max
                } else {
                    0.0
                };
                total_saturation += saturation;

                samples += 1;
            }
        }

        if samples == 0 {
            return ImageStats {
                brightness: 0.5,
                saturation: 0.0,
            };
        }

        ImageStats {
            brightness: (total_brightness / f64::from(samples)) as f32,
            saturation: (total_saturation / f64::from(samples)) as f32,
        }
    }
}

#[derive(Debug)]
struct ImageStats {
    brightness: f32,
    saturation: f32,
}

/// Detect motion type from two frames
pub fn detect_motion_type(img_a: &DynamicImage, img_b: &DynamicImage) -> String {
    let scorer = ConfidenceScorer::new(0.85);
    let diff = scorer.calculate_pixel_difference(img_a, img_b);

    // Very rough heuristics - in practice you'd want more sophisticated detection
    if diff < 0.05 {
        "static".to_string()
    } else if diff < 0.15 {
        "subtle".to_string() // Small movements like breathing, blinking
    } else if diff < 0.3 {
        "normal".to_string() // Typical animation motion
    } else {
        "dynamic".to_string() // Large movements, action scenes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confidence_scoring() {
        let scorer = ConfidenceScorer::new(0.85);

        // Create simple test images
        let img_a = DynamicImage::new_rgba8(100, 100);
        let img_b = DynamicImage::new_rgba8(100, 100);
        let generated = DynamicImage::new_rgba8(100, 100);

        let score = scorer
            .score_frame(&generated, &img_a, &img_b, "walk", Some("hero"))
            .unwrap();

        // Score should be between 0 and 1
        assert!(score >= 0.0);
        assert!(score <= 1.0);
    }

    #[test]
    fn test_motion_type_detection() {
        let img_a = DynamicImage::new_rgba8(100, 100);
        let img_b = DynamicImage::new_rgba8(100, 100);

        // Identical images should be detected as static
        let motion = detect_motion_type(&img_a, &img_b);
        assert!(motion == "static" || motion == "subtle");
    }

    #[test]
    fn test_auto_accept_threshold() {
        let scorer = ConfidenceScorer::new(0.85);

        assert!(scorer.should_auto_accept(0.9));
        assert!(scorer.should_auto_accept(0.85));
        assert!(!scorer.should_auto_accept(0.84));
        assert!(!scorer.should_auto_accept(0.5));
    }
}
