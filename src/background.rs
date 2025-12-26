// based on https://github.com/benface/bgone/blob/b362931f37252301f0f8dec183b2072f415b9b5f/src/background.rs

use crate::color::Color;
use image::DynamicImage;
use std::collections::HashMap;

/// Configuration for background detection
pub struct BackgroundDetectionConfig {
  /// Sample every N pixels on edges
  pub edge_sample_interval: u32,
}

impl Default for BackgroundDetectionConfig {
  fn default() -> Self {
    Self {
      edge_sample_interval: 10,
    }
  }
}

/// Detect the background color by sampling image edges and corners
///
/// # Arguments
/// * `img` - The image to analyze
///
/// # Returns
/// The most common RGB color found at image edges and corners
pub fn detect_background_color(img: &DynamicImage) -> Color {
  detect_background_color_with_config(img, &BackgroundDetectionConfig::default())
}

/// Detect background color with custom configuration
///
/// # Arguments
/// * `img` - The image to analyze
/// * `config` - Configuration for background detection
///
/// # Returns
/// The most common RGB color found at image edges and corners
pub fn detect_background_color_with_config(
  img: &DynamicImage,
  config: &BackgroundDetectionConfig,
) -> Color {
  let rgba = img.to_rgba8();
  let (width, height) = rgba.dimensions();

  let mut color_counts: HashMap<Color, u32> = HashMap::new();
  let mut sample_points = Vec::new();

  // Add corners
  sample_points.extend(&[
    (0, 0),
    (width - 1, 0),
    (0, height - 1),
    (width - 1, height - 1),
  ]);

  // Add edge samples
  for x in (0..width).step_by(config.edge_sample_interval as usize) {
    sample_points.push((x, 0));
    sample_points.push((x, height - 1));
  }

  for y in (0..height).step_by(config.edge_sample_interval as usize) {
    sample_points.push((0, y));
    sample_points.push((width - 1, y));
  }

  // Count color occurrences
  // For translucent pixels, composite over black to get the effective color
  for &(x, y) in &sample_points {
    let pixel = rgba.get_pixel(x, y);
    let alpha = pixel[3] as f64 / 255.0;

    // Composite over black background for translucent pixels
    let color = if alpha < 1.0 {
      [
        (pixel[0] as f64 * alpha).round() as u8,
        (pixel[1] as f64 * alpha).round() as u8,
        (pixel[2] as f64 * alpha).round() as u8,
      ]
    } else {
      [pixel[0], pixel[1], pixel[2]]
    };

    *color_counts.entry(color).or_insert(0) += 1;
  }

  // Find most common color
  color_counts
    .into_iter()
    .max_by_key(|(_, count)| *count)
    .map(|(color, _)| color)
    .unwrap_or([0, 0, 0])
}
