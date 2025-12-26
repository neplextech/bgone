// based on https://github.com/benface/bgone/blob/b362931f37252301f0f8dec183b2072f415b9b5f/src/deduce.rs

use crate::color::{normalize_color, Color, ForegroundColorSpec, NormalizedColor};
use crate::unmix::{compute_result_color, unmix_colors_internal};
use anyhow::Result;
use image::DynamicImage;
use std::collections::HashMap;

const MAX_CANDIDATES_2_UNKNOWNS: usize = 30;
const MAX_CANDIDATES_3_UNKNOWNS_ALL: usize = 25;
const MAX_CANDIDATES_3_UNKNOWNS_SELECTED: usize = 20;

fn color_distance(c1: NormalizedColor, c2: NormalizedColor) -> f64 {
  (0..3).map(|i| (c1[i] - c2[i]).powi(2)).sum::<f64>().sqrt()
}

fn find_candidate_foreground_colors(
  observed_colors: &[(Color, usize)],
  background: Color,
  num_candidates: usize,
  threshold: f64,
) -> Vec<Color> {
  let bg_norm = normalize_color(background);
  let mut candidates = Vec::new();

  for &(observed, _) in observed_colors.iter().take(100) {
    let obs_norm = normalize_color(observed);

    if color_distance(obs_norm, bg_norm) < 0.01 {
      continue;
    }

    for alpha_percent in [25, 50, 75, 90, 100] {
      let alpha = alpha_percent as f64 / 100.0;

      let mut fg = [0.0; 3];
      let mut valid = true;

      for i in 0..3 {
        fg[i] = (obs_norm[i] - bg_norm[i] * (1.0 - alpha)) / alpha;

        if fg[i] < 0.0 || fg[i] > 1.0 {
          valid = false;
          break;
        }
      }

      if valid {
        let fg_u8 = [
          (fg[0] * 255.0).round() as u8,
          (fg[1] * 255.0).round() as u8,
          (fg[2] * 255.0).round() as u8,
        ];

        let reconstructed = [
          (fg[0] * alpha + bg_norm[0] * (1.0 - alpha)) * 255.0,
          (fg[1] * alpha + bg_norm[1] * (1.0 - alpha)) * 255.0,
          (fg[2] * alpha + bg_norm[2] * (1.0 - alpha)) * 255.0,
        ];

        let error = (0..3)
          .map(|i| (reconstructed[i] - observed[i] as f64).powi(2))
          .sum::<f64>()
          .sqrt();

        if error < 5.0 {
          candidates.push(fg_u8);
        }
      }
    }
  }

  let mut unique_candidates = Vec::new();
  for candidate in candidates {
    let mut is_duplicate = false;
    for existing in &unique_candidates {
      if color_distance(normalize_color(candidate), normalize_color(*existing)) < threshold {
        is_duplicate = true;
        break;
      }
    }
    if !is_duplicate {
      unique_candidates.push(candidate);
    }
  }

  if unique_candidates.len() > num_candidates {
    select_most_different_colors(&unique_candidates, num_candidates)
  } else {
    unique_candidates
  }
}

fn select_most_different_colors(colors: &[Color], n: usize) -> Vec<Color> {
  if colors.len() <= n {
    return colors.to_vec();
  }

  let mut selected: Vec<Color> = Vec::new();

  while selected.len() < n {
    let next = colors
      .iter()
      .filter(|&&c| !selected.contains(&c))
      .max_by_key(|&&color| {
        if selected.is_empty() {
          let [r, g, b] = color;
          let max = r.max(g).max(b) as i32;
          let min = r.min(g).min(b) as i32;
          max - min
        } else {
          selected
            .iter()
            .map(|s| {
              let dist = color_distance(normalize_color(color), normalize_color(*s));
              (dist * 1000.0) as i32
            })
            .min()
            .unwrap_or(i32::MAX)
        }
      });

    if let Some(&color) = next {
      selected.push(color);
    } else {
      break;
    }
  }

  selected
}

fn evaluate_color_set(
  foreground_colors: &[NormalizedColor],
  pixels: &[(Color, usize)],
  background: NormalizedColor,
) -> f64 {
  let mut total_error = 0.0;
  let mut total_weight = 0.0;

  for &(observed, count) in pixels {
    let weight = (count as f64).sqrt();

    let unmix_result = unmix_colors_internal(observed, foreground_colors, background, false);
    let (result_color, alpha) = compute_result_color(&unmix_result, foreground_colors);

    let reconstructed = [
      result_color[0] * alpha + background[0] * (1.0 - alpha),
      result_color[1] * alpha + background[1] * (1.0 - alpha),
      result_color[2] * alpha + background[2] * (1.0 - alpha),
    ];

    let observed_norm = normalize_color(observed);
    let error: f64 = (0..3)
      .map(|i| (reconstructed[i] - observed_norm[i]).powi(2))
      .sum::<f64>()
      .sqrt();

    total_error += error * weight;
    total_weight += weight;
  }

  let reconstruction_error = total_error / total_weight;

  let mut color_quality_penalty = 0.0;
  for fg_color in foreground_colors {
    let distance_to_bg = color_distance(*fg_color, background);
    const MAX_PENALTY_PER_COLOR: f64 = 0.00001;
    color_quality_penalty += (1.0 - (distance_to_bg / 1.732)) * MAX_PENALTY_PER_COLOR;
  }
  color_quality_penalty /= foreground_colors.len() as f64;

  reconstruction_error + color_quality_penalty
}

/// Deduce unknown foreground colors from an image
pub fn deduce_unknown_colors(
  image: &DynamicImage,
  specs: &[ForegroundColorSpec],
  background_color: Color,
  threshold: f64,
) -> Result<Vec<Color>> {
  let mut known_colors = Vec::new();
  let mut unknown_indices = Vec::new();

  for (i, spec) in specs.iter().enumerate() {
    match spec {
      ForegroundColorSpec::Known(color) => {
        known_colors.push(*color);
      }
      ForegroundColorSpec::Unknown => {
        unknown_indices.push(i);
      }
    }
  }

  if unknown_indices.is_empty() {
    return Ok(
      specs
        .iter()
        .map(|spec| match spec {
          ForegroundColorSpec::Known(color) => *color,
          ForegroundColorSpec::Unknown => unreachable!(),
        })
        .collect(),
    );
  }

  let rgba = image.to_rgba8();
  let mut color_counts = HashMap::new();

  for pixel in rgba.pixels() {
    let color = [pixel[0], pixel[1], pixel[2]];
    *color_counts.entry(color).or_insert(0) += 1;
  }

  let mut pixels: Vec<(Color, usize)> = color_counts.into_iter().collect();
  pixels.sort_by_key(|&(_, count)| std::cmp::Reverse(count));

  let unknown_count = unknown_indices.len();
  let candidates =
    find_candidate_foreground_colors(&pixels, background_color, unknown_count * 10, threshold);

  let mut all_candidates = candidates;

  let standard_colors = vec![
    [255, 0, 0],
    [0, 255, 0],
    [0, 0, 255],
    [255, 255, 0],
    [255, 0, 255],
    [0, 255, 255],
    [255, 128, 0],
    [128, 0, 255],
  ];

  for color in standard_colors {
    if !known_colors.contains(&color)
      && color != background_color
      && !all_candidates
        .iter()
        .any(|&c| color_distance(normalize_color(c), normalize_color(color)) < 0.01)
    {
      all_candidates.push(color);
    }
  }

  let background_norm = normalize_color(background_color);
  let known_norm: Vec<NormalizedColor> = known_colors.iter().map(|&c| normalize_color(c)).collect();

  let mut best_colors = vec![];
  let mut best_error = f64::MAX;

  if unknown_count == 1 {
    for candidate in &all_candidates {
      let mut test_fg = vec![[0.0; 3]; specs.len()];
      let mut known_idx = 0;

      for (i, spec) in specs.iter().enumerate() {
        match spec {
          ForegroundColorSpec::Known(_) => {
            test_fg[i] = known_norm[known_idx];
            known_idx += 1;
          }
          ForegroundColorSpec::Unknown => {
            test_fg[i] = normalize_color(*candidate);
          }
        }
      }

      let error = evaluate_color_set(&test_fg, &pixels, background_norm);
      if error < best_error {
        best_error = error;
        best_colors = vec![*candidate];
      }
    }
  } else if unknown_count == 2 && all_candidates.len() <= MAX_CANDIDATES_2_UNKNOWNS {
    for (i, c1) in all_candidates.iter().enumerate() {
      for c2 in all_candidates.iter().skip(i + 1) {
        let mut test_fg = vec![[0.0; 3]; specs.len()];
        let mut known_idx = 0;
        let test_unknown = [*c1, *c2];
        let mut unknown_idx = 0;

        for (i, spec) in specs.iter().enumerate() {
          match spec {
            ForegroundColorSpec::Known(_) => {
              test_fg[i] = known_norm[known_idx];
              known_idx += 1;
            }
            ForegroundColorSpec::Unknown => {
              test_fg[i] = normalize_color(test_unknown[unknown_idx]);
              unknown_idx += 1;
            }
          }
        }

        let error = evaluate_color_set(&test_fg, &pixels, background_norm);
        if error < best_error {
          best_error = error;
          best_colors = test_unknown.to_vec();
        }
      }
    }
  } else if unknown_count == 3 {
    let candidates_to_try = if all_candidates.len() <= MAX_CANDIDATES_3_UNKNOWNS_ALL {
      all_candidates.clone()
    } else {
      select_most_different_colors(&all_candidates, MAX_CANDIDATES_3_UNKNOWNS_SELECTED)
    };

    for (i, c1) in candidates_to_try.iter().enumerate() {
      for (j, c2) in candidates_to_try.iter().enumerate().skip(i + 1) {
        for c3 in candidates_to_try.iter().skip(j + 1) {
          let mut test_fg = vec![[0.0; 3]; specs.len()];
          let mut known_idx = 0;
          let test_unknown = [*c1, *c2, *c3];
          let mut unknown_idx = 0;

          for (i, spec) in specs.iter().enumerate() {
            match spec {
              ForegroundColorSpec::Known(_) => {
                test_fg[i] = known_norm[known_idx];
                known_idx += 1;
              }
              ForegroundColorSpec::Unknown => {
                test_fg[i] = normalize_color(test_unknown[unknown_idx]);
                unknown_idx += 1;
              }
            }
          }

          let error = evaluate_color_set(&test_fg, &pixels, background_norm);
          if error < best_error {
            best_error = error;
            best_colors = test_unknown.to_vec();
          }
        }
      }
    }
  } else {
    best_colors = select_most_different_colors(&all_candidates, unknown_count);
  }

  let mut final_colors = Vec::new();
  let mut unknown_idx = 0;

  for spec in specs {
    match spec {
      ForegroundColorSpec::Known(color) => {
        final_colors.push(*color);
      }
      ForegroundColorSpec::Unknown => {
        if unknown_idx < best_colors.len() {
          final_colors.push(best_colors[unknown_idx]);
        } else {
          final_colors.push([128, 128, 128]);
        }
        unknown_idx += 1;
      }
    }
  }

  Ok(final_colors)
}
