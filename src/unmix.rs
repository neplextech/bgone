// based on https://github.com/benface/bgone/blob/b362931f37252301f0f8dec183b2072f415b9b5f/src/unmix.rs

use crate::color::{Color, NormalizedColor};
use nalgebra::{DMatrix, DVector, Vector3};

/// Small epsilon value for numerical stability in floating point comparisons
const EPSILON: f64 = 1e-10;

/// Default threshold for color closeness in non-strict mode (0.05 = 5% of max RGB distance)
pub const DEFAULT_COLOR_CLOSENESS_THRESHOLD: f64 = 0.05;

/// Result of color unmixing: weights for each foreground color and overall alpha
pub struct UnmixResult {
  /// Weight for each foreground color (sums to 1.0 or less)
  pub weights: Vec<f64>,
  /// Overall alpha value (0.0 = fully transparent, 1.0 = fully opaque)
  pub alpha: f64,
}

/// Unmix an observed color into foreground components
///
/// Given an observed color and known foreground/background colors,
/// determines how much of each foreground color contributed to the observed color.
pub fn unmix_colors(
  observed: Color,
  foreground_colors: &[NormalizedColor],
  background: NormalizedColor,
) -> UnmixResult {
  unmix_colors_internal(observed, foreground_colors, background, true)
}

/// Internal unmix function with opacity optimization control
pub(crate) fn unmix_colors_internal(
  observed: Color,
  foreground_colors: &[NormalizedColor],
  background: NormalizedColor,
  optimize_opacity: bool,
) -> UnmixResult {
  let observed = Vector3::new(
    observed[0] as f64 / 255.0,
    observed[1] as f64 / 255.0,
    observed[2] as f64 / 255.0,
  );

  match foreground_colors.len() {
    0 => UnmixResult {
      weights: vec![],
      alpha: 0.0,
    },
    1 => unmix_single_color(observed, foreground_colors[0], background),
    _ => {
      if optimize_opacity {
        unmix_multiple_colors_optimized(observed, foreground_colors, background)
      } else {
        unmix_multiple_colors_simple(observed, foreground_colors, background)
      }
    }
  }
}

/// Unmix when there's only one foreground color
fn unmix_single_color(
  observed: Vector3<f64>,
  foreground: NormalizedColor,
  background: NormalizedColor,
) -> UnmixResult {
  let fg = Vector3::from_row_slice(&foreground);
  let bg = Vector3::from_row_slice(&background);

  // observed = weight * fg + (1 - weight) * bg
  // Solve for weight
  let obs_minus_bg = observed - bg;
  let fg_minus_bg = fg - bg;

  let weight = if fg_minus_bg.norm() > EPSILON {
    let dot = obs_minus_bg.dot(&fg_minus_bg);
    let norm_sq = fg_minus_bg.dot(&fg_minus_bg);
    (dot / norm_sq).clamp(0.0, 1.0)
  } else {
    0.0
  };

  UnmixResult {
    weights: vec![weight],
    alpha: weight,
  }
}

/// Simple unmix using least squares (for color deduction)
fn unmix_multiple_colors_simple(
  observed: Vector3<f64>,
  foreground_colors: &[NormalizedColor],
  background: NormalizedColor,
) -> UnmixResult {
  let n = foreground_colors.len();

  // Build matrix where columns are (fg_i - bg)
  let mut matrix_data = Vec::with_capacity(3 * n);
  for fg in foreground_colors {
    matrix_data.push(fg[0] - background[0]);
    matrix_data.push(fg[1] - background[1]);
    matrix_data.push(fg[2] - background[2]);
  }

  let a = DMatrix::from_column_slice(3, n, &matrix_data);
  let b = observed - Vector3::from_row_slice(&background);
  let b_vec = DVector::from_column_slice(&[b[0], b[1], b[2]]);

  // Solve using pseudo-inverse
  let weights = match a.pseudo_inverse(EPSILON) {
    Ok(a_inv) => {
      let solution = a_inv * b_vec;
      solution.iter().map(|&w| w.max(0.0)).collect()
    }
    Err(_) => {
      // Fallback: use only first color
      let mut weights = vec![0.0; n];
      weights[0] = 1.0;
      weights
    }
  };

  // Calculate alpha as sum of weights (clamped to 1.0)
  let sum: f64 = weights.iter().sum();
  let (final_weights, alpha) = if sum > 1.0 {
    // Normalize weights to sum to 1.0
    let normalized: Vec<f64> = weights.iter().map(|w| w / sum).collect();
    (normalized, 1.0)
  } else {
    (weights, sum)
  };

  UnmixResult {
    weights: final_weights,
    alpha,
  }
}

/// Unmix when there are multiple foreground colors using least squares
/// Optimizes for maximum opacity while maintaining color accuracy.
///
/// This function tries multiple approaches to find the solution with maximum
/// opacity that still accurately reconstructs the observed color:
/// 1. Standard least squares (all colors)
/// 2. Single colors (maximum possible opacity)
/// 3. Pairs of colors (compromise between opacity and flexibility)
///
/// All solutions are verified to ensure they reconstruct the original color
/// within a small error threshold.
fn unmix_multiple_colors_optimized(
  observed: Vector3<f64>,
  foreground_colors: &[NormalizedColor],
  background: NormalizedColor,
) -> UnmixResult {
  let n = foreground_colors.len();
  let bg = Vector3::from_row_slice(&background);
  let target = observed - bg;

  // Try different approaches to find the one with maximum opacity
  let mut best_weights = vec![0.0; n];
  let mut best_alpha = 0.0;

  // Approach 1: Standard least squares solution
  let mut matrix_data = Vec::with_capacity(3 * n);
  for fg in foreground_colors {
    matrix_data.push(fg[0] - background[0]);
    matrix_data.push(fg[1] - background[1]);
    matrix_data.push(fg[2] - background[2]);
  }

  let a = DMatrix::from_column_slice(3, n, &matrix_data);
  let b_vec = DVector::from_column_slice(&[target[0], target[1], target[2]]);

  if let Ok(a_inv) = a.pseudo_inverse(EPSILON) {
    let solution = a_inv * b_vec.clone();
    let weights: Vec<f64> = solution.iter().map(|&w| w.max(0.0)).collect();
    let sum: f64 = weights.iter().sum();

    if sum > 0.0 {
      let alpha = sum.min(1.0);
      if alpha > best_alpha {
        best_weights = if sum > 1.0 {
          weights.iter().map(|w| w / sum).collect()
        } else {
          weights
        };
        best_alpha = alpha;
      }
    }
  }

  // Approach 2: Try each color individually to see if any single color achieves higher opacity
  for (i, fg) in foreground_colors.iter().enumerate() {
    let fg_vec = Vector3::from_row_slice(fg);
    let fg_minus_bg = fg_vec - bg;

    if fg_minus_bg.norm() > EPSILON {
      let dot = target.dot(&fg_minus_bg);
      let norm_sq = fg_minus_bg.dot(&fg_minus_bg);
      let weight = (dot / norm_sq).clamp(0.0, 1.0);

      // Verify the reconstructed color is close to the observed color
      let reconstructed = weight * fg_vec + (1.0 - weight) * bg;
      let error = (reconstructed - observed).norm();

      // Only accept if the reconstruction error is small
      if weight > best_alpha && error < 0.01 {
        best_weights = vec![0.0; n];
        best_weights[i] = weight;
        best_alpha = weight;
      }
    }
  }

  // Approach 3: Try pairs of colors for better opacity
  if n >= 2 && best_alpha < 0.99 {
    for i in 0..n {
      for j in (i + 1)..n {
        // Build 3x2 matrix for this pair
        let fg_i = foreground_colors[i];
        let fg_j = foreground_colors[j];
        let pair_matrix = DMatrix::from_column_slice(
          3,
          2,
          &[
            fg_i[0] - background[0],
            fg_j[0] - background[0],
            fg_i[1] - background[1],
            fg_j[1] - background[1],
            fg_i[2] - background[2],
            fg_j[2] - background[2],
          ],
        );

        if let Ok(pair_inv) = pair_matrix.pseudo_inverse(EPSILON) {
          let pair_solution = pair_inv * b_vec.clone();
          let w_i = pair_solution[0].max(0.0);
          let w_j = pair_solution[1].max(0.0);
          let sum = w_i + w_j;

          if sum > 0.0 {
            let alpha = sum.min(1.0);

            // Verify the reconstruction is accurate
            let normalized_wi = if sum > 1.0 { w_i / sum } else { w_i };
            let normalized_wj = if sum > 1.0 { w_j / sum } else { w_j };
            let reconstructed = normalized_wi * Vector3::from_row_slice(&fg_i)
              + normalized_wj * Vector3::from_row_slice(&fg_j)
              + (1.0 - normalized_wi - normalized_wj) * bg;
            let error = (reconstructed - observed).norm();

            // Only accept if reconstruction is accurate AND alpha is better
            if alpha > best_alpha && error < 0.01 {
              best_weights = vec![0.0; n];
              if sum > 1.0 {
                best_weights[i] = w_i / sum;
                best_weights[j] = w_j / sum;
                best_alpha = 1.0;
              } else {
                best_weights[i] = w_i;
                best_weights[j] = w_j;
                best_alpha = alpha;
              }
            }
          }
        }
      }
    }
  }

  UnmixResult {
    weights: best_weights,
    alpha: best_alpha,
  }
}

/// Calculate the Euclidean distance between two colors in RGB space
fn color_distance(color1: Vector3<f64>, color2: Vector3<f64>) -> f64 {
  (color1 - color2).norm()
}

/// Check if an observed color is "close enough" to any foreground color when unmixed
/// Returns true if the color can be primarily represented by one of the foreground colors
pub fn is_color_close_to_foreground(
  observed: Vector3<f64>,
  foreground_colors: &[NormalizedColor],
  background: NormalizedColor,
  threshold: f64,
) -> bool {
  // Try unmixing with each individual foreground color
  for fg in foreground_colors {
    let fg_vec = Vector3::from_row_slice(fg);
    let bg_vec = Vector3::from_row_slice(&background);

    // Calculate the weight needed for this foreground color
    let fg_minus_bg = fg_vec - bg_vec;
    if fg_minus_bg.norm() > EPSILON {
      let obs_minus_bg = observed - bg_vec;
      let dot = obs_minus_bg.dot(&fg_minus_bg);
      let norm_sq = fg_minus_bg.dot(&fg_minus_bg);
      let weight = (dot / norm_sq).clamp(0.0, 1.0);

      // Reconstruct the color with this single foreground
      let reconstructed = weight * fg_vec + (1.0 - weight) * bg_vec;

      // Check if the reconstruction is close to the observed color
      if color_distance(reconstructed, observed) < threshold {
        return true;
      }
    }
  }

  false
}

/// Compute the final color from unmixing results
pub fn compute_result_color(
  unmix_result: &UnmixResult,
  foreground_colors: &[NormalizedColor],
) -> (NormalizedColor, f64) {
  if unmix_result.alpha == 0.0 {
    return ([0.0, 0.0, 0.0], 0.0);
  }

  let mut result = [0.0, 0.0, 0.0];
  let sum_weights: f64 = unmix_result.weights.iter().sum();

  if sum_weights > 0.0 {
    for (i, &weight) in unmix_result.weights.iter().enumerate() {
      if let Some(fg) = foreground_colors.get(i) {
        result[0] += weight * fg[0];
        result[1] += weight * fg[1];
        result[2] += weight * fg[2];
      }
    }

    // Normalize by sum of weights
    result[0] /= sum_weights;
    result[1] /= sum_weights;
    result[2] /= sum_weights;
  }

  (result, unmix_result.alpha)
}
