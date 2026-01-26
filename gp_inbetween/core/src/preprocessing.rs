use crate::config::PreprocessingConfig;
use anyhow::Result;
use image::{DynamicImage, GenericImageView, ImageBuffer, Rgba, imageops::FilterType};

pub struct Preprocessor {
    config: PreprocessingConfig,
}

impl Preprocessor {
    pub fn new(config: &PreprocessingConfig) -> Self {
        Self {
            config: config.clone(),
        }
    }

    /// Process an image: normalize resolution and optionally clean up
    pub fn process(&self, img: &DynamicImage) -> Result<DynamicImage> {
        let mut processed = img.clone();

        // Normalize resolution if enabled
        if self.config.normalize_resolution {
            processed = self.normalize_resolution(&processed);
        }

        // Clean up image if enabled
        if self.config.cleanup_enabled {
            processed = self.cleanup(&processed);
        }

        Ok(processed)
    }

    /// Resize and pad image to target square resolution
    fn normalize_resolution(&self, img: &DynamicImage) -> DynamicImage {
        let target = self.config.target_resolution;
        let (width, height) = img.dimensions();

        // Already at target size
        if width == target && height == target {
            return img.clone();
        }

        // Calculate scale to fit within target while preserving aspect ratio
        let scale = (target as f32) / (width.max(height) as f32);
        let new_width = ((width as f32) * scale).round() as u32;
        let new_height = ((height as f32) * scale).round() as u32;

        log::debug!(
            "Resizing {}x{} -> {}x{} (target {})",
            width,
            height,
            new_width,
            new_height,
            target
        );

        // Resize with high-quality interpolation
        let resized = img.resize(new_width, new_height, FilterType::Lanczos3);

        // Create transparent canvas at target size
        let mut canvas: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(target, target, Rgba([0, 0, 0, 0]));

        // Center the resized image on the canvas
        let x_offset = (target - new_width) / 2;
        let y_offset = (target - new_height) / 2;

        // Copy resized image onto canvas
        let resized_rgba = resized.to_rgba8();
        for (x, y, pixel) in resized_rgba.enumerate_pixels() {
            let canvas_x = x + x_offset;
            let canvas_y = y + y_offset;
            if canvas_x < target && canvas_y < target {
                canvas.put_pixel(canvas_x, canvas_y, *pixel);
            }
        }

        DynamicImage::ImageRgba8(canvas)
    }

    /// Clean up the image by removing noise and artifacts
    fn cleanup(&self, img: &DynamicImage) -> DynamicImage {
        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();

        // Create output buffer
        let mut output: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(width, height);

        // Simple cleanup: remove isolated pixels (noise)
        // A pixel is considered isolated if it has fewer than 2 non-transparent neighbors
        for y in 0..height {
            for x in 0..width {
                let pixel = rgba.get_pixel(x, y);

                // Skip transparent pixels
                if pixel[3] < 128 {
                    output.put_pixel(x, y, Rgba([0, 0, 0, 0]));
                    continue;
                }

                // Count non-transparent neighbors
                let mut neighbor_count = 0;
                for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        if dx == 0 && dy == 0 {
                            continue;
                        }

                        let nx = x as i32 + dx;
                        let ny = y as i32 + dy;

                        if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                            let neighbor = rgba.get_pixel(nx as u32, ny as u32);
                            if neighbor[3] >= 128 {
                                neighbor_count += 1;
                            }
                        }
                    }
                }

                // Keep pixel if it has enough neighbors (not isolated noise)
                if neighbor_count >= 2 {
                    output.put_pixel(x, y, *pixel);
                } else {
                    output.put_pixel(x, y, Rgba([0, 0, 0, 0]));
                }
            }
        }

        // Clean alpha channel: make pixels either fully transparent or fully opaque
        for pixel in output.pixels_mut() {
            if pixel[3] < 128 {
                *pixel = Rgba([0, 0, 0, 0]);
            } else {
                pixel[3] = 255;
            }
        }

        DynamicImage::ImageRgba8(output)
    }

    /// Get the original dimensions before normalization (for reverse mapping)
    pub fn get_padding_info(
        &self,
        original_width: u32,
        original_height: u32,
    ) -> PaddingInfo {
        let target = self.config.target_resolution;
        let scale = (target as f32) / (original_width.max(original_height) as f32);
        let new_width = ((original_width as f32) * scale).round() as u32;
        let new_height = ((original_height as f32) * scale).round() as u32;

        PaddingInfo {
            x_offset: (target - new_width) / 2,
            y_offset: (target - new_height) / 2,
            scaled_width: new_width,
            scaled_height: new_height,
            scale,
        }
    }

    /// Remove padding and restore original aspect ratio
    pub fn restore_original_size(
        &self,
        processed: &DynamicImage,
        padding_info: &PaddingInfo,
        original_width: u32,
        original_height: u32,
    ) -> DynamicImage {
        // Crop to remove padding
        let cropped = processed.crop_imm(
            padding_info.x_offset,
            padding_info.y_offset,
            padding_info.scaled_width,
            padding_info.scaled_height,
        );

        // Resize back to original dimensions
        cropped.resize_exact(original_width, original_height, FilterType::Lanczos3)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PaddingInfo {
    pub x_offset: u32,
    pub y_offset: u32,
    pub scaled_width: u32,
    pub scaled_height: u32,
    pub scale: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> PreprocessingConfig {
        PreprocessingConfig {
            cleanup_enabled: true,
            target_resolution: 512,
            normalize_resolution: true,
            min_stroke_length: 5.0,
        }
    }

    #[test]
    fn test_normalize_square_image() {
        let config = test_config();
        let preprocessor = Preprocessor::new(&config);

        let img = DynamicImage::new_rgba8(256, 256);
        let processed = preprocessor.normalize_resolution(&img);

        assert_eq!(processed.width(), 512);
        assert_eq!(processed.height(), 512);
    }

    #[test]
    fn test_normalize_landscape_image() {
        let config = test_config();
        let preprocessor = Preprocessor::new(&config);

        let img = DynamicImage::new_rgba8(800, 400);
        let processed = preprocessor.normalize_resolution(&img);

        // Should be padded to 512x512
        assert_eq!(processed.width(), 512);
        assert_eq!(processed.height(), 512);
    }

    #[test]
    fn test_padding_info_roundtrip() {
        let config = test_config();
        let preprocessor = Preprocessor::new(&config);

        let original_width = 800u32;
        let original_height = 400u32;

        let img = DynamicImage::new_rgba8(original_width, original_height);
        let padding_info = preprocessor.get_padding_info(original_width, original_height);
        let processed = preprocessor.normalize_resolution(&img);
        let restored =
            preprocessor.restore_original_size(&processed, &padding_info, original_width, original_height);

        assert_eq!(restored.width(), original_width);
        assert_eq!(restored.height(), original_height);
    }
}
