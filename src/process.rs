// based on https://github.com/benface/bgone/blob/b362931f37252301f0f8dec183b2072f415b9b5f/src/lib.rs

use crate::color::{denormalize_color, normalize_color, Color, NormalizedColor};
use crate::unmix::{compute_result_color, is_color_close_to_foreground, unmix_colors};
use image::{ImageBuffer, Rgba};
use nalgebra::Vector3;

/// Composite a pixel over a background color to handle existing alpha channels
///
/// If the input pixel is translucent (alpha < 255), this pre-composes it over
/// the background color to produce an opaque equivalent. This allows bgone to
/// correctly process images that already have transparency.
///
/// Formula: result = foreground * alpha + background * (1 - alpha)
pub fn composite_pixel_over_background(pixel: &Rgba<u8>, background: Color) -> Color {
  let alpha = pixel[3] as f64 / 255.0;

  if alpha >= 1.0 {
    // Fully opaque - use as-is
    [pixel[0], pixel[1], pixel[2]]
  } else {
    // Translucent - composite over background
    let bg_norm = [
      background[0] as f64 / 255.0,
      background[1] as f64 / 255.0,
      background[2] as f64 / 255.0,
    ];
    let fg_norm = [
      pixel[0] as f64 / 255.0,
      pixel[1] as f64 / 255.0,
      pixel[2] as f64 / 255.0,
    ];

    [
      ((fg_norm[0] * alpha + bg_norm[0] * (1.0 - alpha)) * 255.0).round() as u8,
      ((fg_norm[1] * alpha + bg_norm[1] * (1.0 - alpha)) * 255.0).round() as u8,
      ((fg_norm[2] * alpha + bg_norm[2] * (1.0 - alpha)) * 255.0).round() as u8,
    ]
  }
}

/// Find the minimum alpha value that produces a valid foreground color
///
/// Given an observed color and background, this function finds the minimum alpha
/// value (between 0 and 1) such that there exists a valid foreground color
/// (all RGB components in [0, 1]) that satisfies:
/// observed = alpha * foreground + (1 - alpha) * background
///
/// Returns (foreground_color, alpha) or None if no valid solution exists
pub fn find_minimum_alpha_for_color(
  obs_norm: NormalizedColor,
  background: NormalizedColor,
) -> Option<(NormalizedColor, f64)> {
  let mut best_alpha = 1.0;
  let mut best_fg = obs_norm;

  // For truly minimal alpha, we need to consider different foreground colors.
  // The optimal foreground often has components at the extremes (0 or 1).
  // We'll try all 8 combinations of extreme values, plus the computed values.

  // First, let's compute the minimum alpha needed for each channel independently
  // For each channel i: observed[i] = alpha * fg[i] + (1 - alpha) * bg[i]
  // If fg[i] = 0: alpha = (bg[i] - observed[i]) / bg[i] (if bg[i] != 0)
  // If fg[i] = 1: alpha = (observed[i] - bg[i]) / (1 - bg[i]) (if bg[i] != 1)

  // Try all combinations of extreme foreground values (0 or 1 for each channel)
  for r_extreme in &[0.0, 1.0] {
    for g_extreme in &[0.0, 1.0] {
      for b_extreme in &[0.0, 1.0] {
        let fg_candidate = [*r_extreme, *g_extreme, *b_extreme];

        // Calculate required alpha for this foreground color
        // observed = alpha * foreground + (1 - alpha) * background
        // alpha = (observed - background) / (foreground - background)

        let mut alpha_needed = 0.0;
        let mut valid = true;

        let mut first_alpha_set = false;

        for i in 0..3 {
          let denom = fg_candidate[i] - background[i];
          if denom.abs() < 1e-10 {
            // fg[i] ≈ bg[i], check if observed[i] ≈ bg[i] too
            if (obs_norm[i] - background[i]).abs() > 1e-10 {
              valid = false;
              break;
            }
            // Any alpha works for this channel, continue
          } else {
            let alpha_i = (obs_norm[i] - background[i]) / denom;
            if !first_alpha_set {
              alpha_needed = alpha_i;
              first_alpha_set = true;
            } else if (alpha_i - alpha_needed).abs() > 1e-10 {
              // Different channels require different alphas - invalid
              valid = false;
              break;
            }
          }
        }

        if valid
          && first_alpha_set
          && alpha_needed > 0.0
          && alpha_needed <= 1.0
          && alpha_needed < best_alpha
        {
          // Verify the solution
          let mut reconstructed_valid = true;
          for i in 0..3 {
            let reconstructed =
              alpha_needed * fg_candidate[i] + (1.0 - alpha_needed) * background[i];
            if (reconstructed - obs_norm[i]).abs() > 1e-10 {
              reconstructed_valid = false;
              break;
            }
          }

          if reconstructed_valid {
            best_alpha = alpha_needed;
            best_fg = fg_candidate;
          }
        }
      }
    }
  }

  // Also try the direct computation approach with fine-grained alpha search
  for alpha_int in 1..=1000 {
    let alpha = alpha_int as f64 / 1000.0;

    if alpha >= best_alpha {
      break; // No point checking higher alphas
    }

    // Calculate the required foreground color for this alpha
    let fg_r = (obs_norm[0] - (1.0 - alpha) * background[0]) / alpha;
    let fg_g = (obs_norm[1] - (1.0 - alpha) * background[1]) / alpha;
    let fg_b = (obs_norm[2] - (1.0 - alpha) * background[2]) / alpha;

    // Check if this foreground color is valid (all components in [0, 1])
    if (0.0..=1.0).contains(&fg_r) && (0.0..=1.0).contains(&fg_g) && (0.0..=1.0).contains(&fg_b) {
      best_alpha = alpha;
      best_fg = [fg_r, fg_g, fg_b];
      break; // This is the minimum alpha with direct computation
    }
  }

  Some((best_fg, best_alpha))
}

/// Process a pixel in non-strict mode without foreground colors
///
/// In this mode, we find the optimal foreground color and alpha that produces
/// the observed color when alpha-blended with the background.
///
/// The algorithm:
/// 1. Searches for the minimum alpha value that allows a valid foreground color
/// 2. A valid foreground color has all RGB components in [0, 1] range
/// 3. Always produces perfect reconstruction of the original image
pub fn process_pixel_non_strict_no_fg(observed: Color, background: NormalizedColor) -> [u8; 4] {
  let obs_norm = normalize_color(observed);

  // If the observed color is exactly the background, it's fully transparent
  if (obs_norm[0] - background[0]).abs() < 1e-6
    && (obs_norm[1] - background[1]).abs() < 1e-6
    && (obs_norm[2] - background[2]).abs() < 1e-6
  {
    return [0, 0, 0, 0];
  }

  // Find the optimal alpha and foreground color
  let (best_fg, best_alpha) = find_minimum_alpha_for_color(obs_norm, background).unwrap_or({
    // If we didn't find a valid solution with alpha <= 1.0, something is wrong
    // Fall back to using alpha = 1.0
    (obs_norm, 1.0)
  });

  let final_color = denormalize_color(best_fg);
  [
    final_color[0],
    final_color[1],
    final_color[2],
    (best_alpha * 255.0).round() as u8,
  ]
}

/// Process a pixel in non-strict mode with foreground colors
///
/// This mode combines two strategies:
/// 1. For pixels "close enough" to specified foreground colors (within threshold):
///    - Uses the standard unmixing algorithm optimized for high opacity
///    - Restricts to the specified foreground colors
/// 2. For pixels NOT close to any foreground color:
///    - Allows ANY color to be used
///    - Finds the minimum alpha that produces a valid foreground color
///    - Ensures perfect reconstruction
///
/// This allows the tool to preserve colors like glows and gradients that aren't
/// close to the specified foreground colors, while still optimizing for the
/// specified colors when appropriate.
pub fn process_pixel_non_strict_with_fg(
  observed: Color,
  foreground_colors: &[NormalizedColor],
  background: NormalizedColor,
  threshold: f64,
) -> [u8; 4] {
  let obs_norm = normalize_color(observed);
  let obs_vec = Vector3::new(obs_norm[0] as f64, obs_norm[1] as f64, obs_norm[2] as f64);

  // If the observed color is exactly the background, it's fully transparent
  if (obs_norm[0] - background[0]).abs() < 1e-6
    && (obs_norm[1] - background[1]).abs() < 1e-6
    && (obs_norm[2] - background[2]).abs() < 1e-6
  {
    return [0, 0, 0, 0];
  }

  // Check if this pixel is close to any foreground color
  let close_to_fg = is_color_close_to_foreground(obs_vec, foreground_colors, background, threshold);

  if close_to_fg {
    // Use the standard unmixing algorithm optimized for high opacity
    let unmix_result = unmix_colors(observed, foreground_colors, background);
    let (result_color, alpha) = compute_result_color(&unmix_result, foreground_colors);
    let final_color = denormalize_color(result_color);
    [
      final_color[0],
      final_color[1],
      final_color[2],
      (alpha * 255.0).round() as u8,
    ]
  } else {
    // Not close to any foreground color - find ANY color that works with minimal alpha
    let obs_norm = normalize_color(observed);

    // Find the optimal alpha and foreground color
    let (best_fg, best_alpha) = find_minimum_alpha_for_color(obs_norm, background).unwrap_or({
      // If we didn't find a valid solution with alpha <= 1.0, something is wrong
      // Fall back to using alpha = 1.0
      (obs_norm, 1.0)
    });

    let final_color = denormalize_color(best_fg);
    [
      final_color[0],
      final_color[1],
      final_color[2],
      (best_alpha * 255.0).round() as u8,
    ]
  }
}

/// Trim an image by cropping to the bounding box of non-transparent pixels.
///
/// Finds the bounding box of all pixels with alpha > 0 and crops the image
/// to that region. If all pixels are transparent, returns a 1x1 transparent image.
pub fn trim_to_content(img: &ImageBuffer<Rgba<u8>, Vec<u8>>) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
  let (width, height) = img.dimensions();

  if width == 0 || height == 0 {
    return ImageBuffer::new(1, 1);
  }

  // Find bounding box of non-transparent pixels
  let mut min_x = width;
  let mut min_y = height;
  let mut max_x = 0u32;
  let mut max_y = 0u32;

  for y in 0..height {
    for x in 0..width {
      let pixel = img.get_pixel(x, y);
      if pixel[3] > 0 {
        // Non-transparent pixel
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
      }
    }
  }

  // If no non-transparent pixels found, return a 1x1 transparent image
  if max_x < min_x || max_y < min_y {
    return ImageBuffer::from_pixel(1, 1, Rgba([0, 0, 0, 0]));
  }

  // Calculate new dimensions (inclusive bounds, so add 1)
  let new_width = max_x - min_x + 1;
  let new_height = max_y - min_y + 1;

  // If no trimming needed, return a clone
  if new_width == width && new_height == height {
    return img.clone();
  }

  // Create cropped image
  let mut trimmed = ImageBuffer::new(new_width, new_height);
  for y in 0..new_height {
    for x in 0..new_width {
      let src_pixel = img.get_pixel(min_x + x, min_y + y);
      trimmed.put_pixel(x, y, *src_pixel);
    }
  }

  trimmed
}
