#![deny(clippy::all)]

pub mod background;
pub mod color;
pub mod deduce;
pub mod process;
pub mod unmix;

use crate::background::detect_background_color as detect_bg;
use crate::color::{
  denormalize_color, normalize_color, parse_foreground_spec, parse_hex_color, Color,
  ForegroundColorSpec, NormalizedColor,
};
use crate::deduce::deduce_unknown_colors;
use crate::process::{
  composite_pixel_over_background, process_pixel_non_strict_no_fg,
  process_pixel_non_strict_with_fg, trim_to_content,
};
use crate::unmix::{compute_result_color, unmix_colors, DEFAULT_COLOR_CLOSENESS_THRESHOLD};
use image::{ImageBuffer, Rgba};
use napi::bindgen_prelude::*;
use napi_derive::napi;
use rayon::prelude::*;
use std::io::Cursor;

#[napi(object)]
pub struct RgbColor {
  pub r: u8,
  pub g: u8,
  pub b: u8,
}

#[napi(object)]
pub struct RgbaColor {
  pub r: u8,
  pub g: u8,
  pub b: u8,
  pub a: u8,
}

#[napi(object)]
pub struct NormalizedRgbColor {
  pub r: f64,
  pub g: f64,
  pub b: f64,
}

#[napi(object)]
pub struct ProcessImageOptions {
  /// The input image buffer
  pub input: Buffer,
  /// The foreground colors to match, if any. Use "auto" to deduce unknown colors.
  pub foreground_colors: Option<Vec<String>>,
  /// The background color to remove. If not specified, it will be auto-detected.
  pub background_color: Option<String>,
  /// Whether to use strict mode. Restricts unmixing to only the specified foreground colors.
  pub strict_mode: bool,
  /// The threshold for color closeness (0.0-1.0, default: 0.05)
  pub threshold: Option<f64>,
  /// Whether to trim the output image to the bounding box of non-transparent pixels
  pub trim: bool,
}

#[napi(object)]
pub struct UnmixResultJs {
  /// The weights for each foreground color
  pub weights: Vec<f64>,
  /// The alpha value
  pub alpha: f64,
}

pub struct AsyncProcessImage {
  options: ProcessImageOptions,
}

#[napi]
impl Task for AsyncProcessImage {
  type Output = Vec<u8>;
  type JsValue = Buffer;

  fn compute(&mut self) -> Result<Self::Output> {
    process_image_internal(&self.options)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into())
  }
}

#[napi]
/// Process an image asynchronously to remove its background
///
/// Supports automatic background detection, foreground color deduction using "auto",
/// and both strict and non-strict processing modes.
///
/// # Arguments
/// * `options` - The options for the image processing
///
/// # Returns
/// A promise that resolves to the processed image buffer (PNG format)
pub fn process_image(options: ProcessImageOptions) -> AsyncTask<AsyncProcessImage> {
  AsyncTask::new(AsyncProcessImage { options })
}

#[napi]
/// Process an image synchronously to remove its background
///
/// Supports automatic background detection, foreground color deduction using "auto",
/// and both strict and non-strict processing modes.
///
/// # Arguments
/// * `options` - The options for the image processing
///
/// # Returns
/// The processed image buffer (PNG format)
pub fn process_image_sync(options: ProcessImageOptions) -> Result<Buffer> {
  let result = process_image_internal(&options)?;
  Ok(result.into())
}

#[napi]
/// Detect the background color of an image by sampling its edges
///
/// # Arguments
/// * `input` - The input image buffer
///
/// # Returns
/// The detected background color
pub fn detect_background_color(input: Buffer) -> Result<RgbColor> {
  let img = image::load_from_memory(&input)
    .map_err(|e| Error::new(Status::InvalidArg, format!("Failed to load image: {}", e)))?;
  let color = detect_bg(&img);
  Ok(RgbColor {
    r: color[0],
    g: color[1],
    b: color[2],
  })
}

#[napi]
/// Parse a hex color string into an RGB color
///
/// Supports formats: "#ff0000", "ff0000", "#f00", "f00"
///
/// # Arguments
/// * `hex` - The hex color string
///
/// # Returns
/// The parsed RGB color
pub fn parse_color(hex: String) -> Result<RgbColor> {
  let color = parse_hex_color(&hex)
    .map_err(|e| Error::new(Status::InvalidArg, format!("Invalid hex color: {}", e)))?;
  Ok(RgbColor {
    r: color[0],
    g: color[1],
    b: color[2],
  })
}

#[napi]
/// Convert an RGB color (0-255) to a normalized RGB color (0.0-1.0)
///
/// # Arguments
/// * `color` - The RGB color
///
/// # Returns
/// The normalized RGB color
pub fn color_to_normalized(color: RgbColor) -> NormalizedRgbColor {
  let normalized = normalize_color([color.r, color.g, color.b]);
  NormalizedRgbColor {
    r: normalized[0],
    g: normalized[1],
    b: normalized[2],
  }
}

#[napi]
/// Convert a normalized RGB color (0.0-1.0) to an RGB color (0-255)
///
/// # Arguments
/// * `color` - The normalized RGB color
///
/// # Returns
/// The RGB color
pub fn normalized_to_color(color: NormalizedRgbColor) -> RgbColor {
  let denormalized = denormalize_color([color.r, color.g, color.b]);
  RgbColor {
    r: denormalized[0],
    g: denormalized[1],
    b: denormalized[2],
  }
}

#[napi]
/// Trim the image to the bounding box of non-transparent pixels
///
/// # Arguments
/// * `input` - The input image buffer
///
/// # Returns
/// The trimmed image buffer (PNG format)
pub fn trim_image(input: Buffer) -> Result<Buffer> {
  let img = image::load_from_memory(&input)
    .map_err(|e| Error::new(Status::InvalidArg, format!("Failed to load image: {}", e)))?;
  let rgba = img.to_rgba8();
  let trimmed = trim_to_content(&rgba);

  let mut buffer = Cursor::new(Vec::new());
  trimmed
    .write_to(&mut buffer, image::ImageFormat::Png)
    .map_err(|e| {
      Error::new(
        Status::GenericFailure,
        format!("Failed to write output image: {}", e),
      )
    })?;

  Ok(buffer.into_inner().into())
}

#[napi]
/// Unmix an observed color into foreground color components
///
/// Given an observed color and known foreground/background colors,
/// determines how much of each foreground color contributed to the observed color.
///
/// # Arguments
/// * `observed` - The observed color
/// * `foreground_colors` - The foreground colors to match
/// * `background` - The background color
///
/// # Returns
/// The unmix result containing weights for each foreground color and overall alpha
pub fn unmix_color(
  observed: RgbColor,
  foreground_colors: Vec<RgbColor>,
  background: RgbColor,
) -> UnmixResultJs {
  let fg_normalized: Vec<NormalizedColor> = foreground_colors
    .iter()
    .map(|c| normalize_color([c.r, c.g, c.b]))
    .collect();
  let bg_normalized = normalize_color([background.r, background.g, background.b]);

  let result = unmix_colors(
    [observed.r, observed.g, observed.b],
    &fg_normalized,
    bg_normalized,
  );

  UnmixResultJs {
    weights: result.weights,
    alpha: result.alpha,
  }
}

#[napi]
/// Compute the final color from unmix result
///
/// # Arguments
/// * `weights` - The weights for each foreground color
/// * `alpha` - The alpha value
/// * `foreground_colors` - The foreground colors
///
/// # Returns
/// The computed RGBA color
pub fn compute_unmix_result_color(
  weights: Vec<f64>,
  alpha: f64,
  foreground_colors: Vec<RgbColor>,
) -> RgbaColor {
  let fg_normalized: Vec<NormalizedColor> = foreground_colors
    .iter()
    .map(|c| normalize_color([c.r, c.g, c.b]))
    .collect();

  let unmix_result = crate::unmix::UnmixResult { weights, alpha };
  let (result_color, result_alpha) = compute_result_color(&unmix_result, &fg_normalized);
  let final_color = denormalize_color(result_color);

  RgbaColor {
    r: final_color[0],
    g: final_color[1],
    b: final_color[2],
    a: (result_alpha * 255.0).round() as u8,
  }
}

#[napi]
/// Composite an RGBA pixel over an RGB background color
///
/// If the input pixel is translucent (alpha < 255), this pre-composes it over
/// the background color to produce an opaque equivalent.
///
/// # Arguments
/// * `pixel` - The RGBA pixel color
/// * `background` - The background RGB color
///
/// # Returns
/// The composited RGB color
pub fn composite_over_background(pixel: RgbaColor, background: RgbColor) -> RgbColor {
  let rgba_pixel = Rgba([pixel.r, pixel.g, pixel.b, pixel.a]);
  let bg_color: Color = [background.r, background.g, background.b];
  let result = composite_pixel_over_background(&rgba_pixel, bg_color);
  RgbColor {
    r: result[0],
    g: result[1],
    b: result[2],
  }
}

#[napi]
/// Get the default threshold for color closeness
///
/// # Returns
/// The default threshold (0.05 = 5% of max RGB distance)
pub fn get_default_threshold() -> f64 {
  DEFAULT_COLOR_CLOSENESS_THRESHOLD
}

fn process_image_internal(options: &ProcessImageOptions) -> Result<Vec<u8>> {
  // Load image from buffer first (needed for auto-detection)
  let img = image::load_from_memory(&options.input)
    .map_err(|e| Error::new(Status::InvalidArg, format!("Failed to load image: {}", e)))?;

  // Determine background color (auto-detect if not specified)
  let background_color = if let Some(bg_hex) = &options.background_color {
    parse_hex_color(bg_hex).map_err(|e| {
      Error::new(
        Status::InvalidArg,
        format!("Invalid background color: {}", e),
      )
    })?
  } else {
    detect_bg(&img)
  };

  // Parse foreground color specs (supports "auto" for deduction)
  let foreground_specs = options
    .foreground_colors
    .as_ref()
    .unwrap_or(&Vec::new())
    .iter()
    .map(|c| parse_foreground_spec(c))
    .collect::<anyhow::Result<Vec<ForegroundColorSpec>>>()
    .map_err(|e| {
      Error::new(
        Status::InvalidArg,
        format!("Invalid foreground color: {}", e),
      )
    })?;

  let color_threshold = options
    .threshold
    .unwrap_or(DEFAULT_COLOR_CLOSENESS_THRESHOLD);

  // Deduce unknown colors if any "auto" specs were provided
  let foreground_colors =
    deduce_unknown_colors(&img, &foreground_specs, background_color, color_threshold).map_err(
      |e| {
        Error::new(
          Status::GenericFailure,
          format!("Failed to deduce foreground colors: {}", e),
        )
      },
    )?;

  let rgba = img.to_rgba8();
  let (width, height) = rgba.dimensions();

  let fg_normalized: Vec<NormalizedColor> = foreground_colors
    .iter()
    .map(|&color| normalize_color(color))
    .collect();

  let bg_normalized = normalize_color(background_color);

  let pixels: Vec<_> = rgba.pixels().collect();
  let processed_pixels: Vec<[u8; 4]> = if !options.strict_mode && foreground_colors.is_empty() {
    pixels
      .par_iter()
      .map(|pixel| {
        let observed = composite_pixel_over_background(pixel, background_color);
        process_pixel_non_strict_no_fg(observed, bg_normalized)
      })
      .collect()
  } else if !options.strict_mode {
    pixels
      .par_iter()
      .map(|pixel| {
        let observed = composite_pixel_over_background(pixel, background_color);
        process_pixel_non_strict_with_fg(observed, &fg_normalized, bg_normalized, color_threshold)
      })
      .collect()
  } else {
    pixels
      .par_iter()
      .map(|pixel| {
        let observed = composite_pixel_over_background(pixel, background_color);
        let unmix_result = unmix_colors(observed, &fg_normalized, bg_normalized);
        let (result_color, alpha) = compute_result_color(&unmix_result, &fg_normalized);

        let final_color = denormalize_color(result_color);
        [
          final_color[0],
          final_color[1],
          final_color[2],
          (alpha * 255.0).round() as u8,
        ]
      })
      .collect()
  };

  let mut output_img = ImageBuffer::<Rgba<u8>, Vec<u8>>::new(width, height);
  for (i, pixel) in output_img.pixels_mut().enumerate() {
    *pixel = Rgba(processed_pixels[i]);
  }

  let final_img = if options.trim {
    trim_to_content(&output_img)
  } else {
    output_img
  };

  let mut buffer = Cursor::new(Vec::new());
  final_img
    .write_to(&mut buffer, image::ImageFormat::Png)
    .map_err(|e| {
      Error::new(
        Status::GenericFailure,
        format!("Failed to write output image: {}", e),
      )
    })?;

  Ok(buffer.into_inner())
}
