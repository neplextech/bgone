import test from 'ava';
import { join } from 'node:path';
import { readFile, writeFile } from 'node:fs/promises';
import {
  processImage,
  processImageSync,
  detectBackgroundColor,
  parseColor,
  colorToNormalized,
  normalizedToColor,
  trimImage,
  unmixColor,
  computeUnmixResultColor,
  compositeOverBackground,
  getDefaultThreshold,
} from '../index.js';
import { readFileSync } from 'node:fs';

const INPUT_PATH = join(import.meta.dirname, 'assets', 'image.png');

// ============================================================================
// processImage (async)
// ============================================================================

test('processImage - removes background with auto-detection', async (t) => {
  const inputBuffer = await readFile(INPUT_PATH);
  const output = await processImage({
    input: inputBuffer,
    strictMode: false,
    trim: false,
  });

  t.true(Buffer.isBuffer(output));
  t.true(output.length > 0);
  await writeFile(join(import.meta.dirname, 'assets', 'image-processed.png'), output);
});

test('processImage - removes background with explicit background color', async (t) => {
  const inputBuffer = await readFile(INPUT_PATH);
  const output = await processImage({
    input: inputBuffer,
    backgroundColor: '#ffffff',
    strictMode: false,
    trim: false,
  });

  t.true(Buffer.isBuffer(output));
  t.true(output.length > 0);
});

test('processImage - removes background with foreground colors', async (t) => {
  const inputBuffer = await readFile(INPUT_PATH);
  const output = await processImage({
    input: inputBuffer,
    foregroundColors: ['#000000'],
    backgroundColor: '#ffffff',
    strictMode: false,
    trim: false,
  });

  t.true(Buffer.isBuffer(output));
  t.true(output.length > 0);
});

test('processImage - strict mode with foreground colors', async (t) => {
  const inputBuffer = await readFile(INPUT_PATH);
  const output = await processImage({
    input: inputBuffer,
    foregroundColors: ['#000000'],
    backgroundColor: '#ffffff',
    strictMode: true,
    trim: false,
  });

  t.true(Buffer.isBuffer(output));
  t.true(output.length > 0);
});

test('processImage - with trim enabled', async (t) => {
  const inputBuffer = await readFile(INPUT_PATH);
  const output = await processImage({
    input: inputBuffer,
    strictMode: false,
    trim: true,
  });

  t.true(Buffer.isBuffer(output));
  t.true(output.length > 0);
  await writeFile(join(import.meta.dirname, 'assets', 'image-trimmed.png'), output);
});

test('processImage - with custom threshold', async (t) => {
  const inputBuffer = await readFile(INPUT_PATH);
  const output = await processImage({
    input: inputBuffer,
    foregroundColors: ['#000000'],
    backgroundColor: '#ffffff',
    strictMode: false,
    threshold: 0.1,
    trim: false,
  });

  t.true(Buffer.isBuffer(output));
  t.true(output.length > 0);
});

test('processImage - with auto foreground color deduction', async (t) => {
  const inputBuffer = await readFile(INPUT_PATH);
  const output = await processImage({
    input: inputBuffer,
    foregroundColors: ['auto'],
    backgroundColor: '#ffffff',
    strictMode: true,
    trim: false,
  });

  t.true(Buffer.isBuffer(output));
  t.true(output.length > 0);
});

// ============================================================================
// processImageSync
// ============================================================================

test('processImageSync - removes background synchronously', (t) => {
  const inputBuffer = readFileSync(INPUT_PATH);
  const output = processImageSync({
    input: inputBuffer,
    strictMode: false,
    trim: false,
  });

  t.true(Buffer.isBuffer(output));
  t.true(output.length > 0);
});

test('processImageSync - with explicit background color', (t) => {
  const inputBuffer = readFileSync(INPUT_PATH);
  const output = processImageSync({
    input: inputBuffer,
    backgroundColor: '#ffffff',
    strictMode: false,
    trim: false,
  });

  t.true(Buffer.isBuffer(output));
  t.true(output.length > 0);
});

// ============================================================================
// detectBackgroundColor
// ============================================================================

test('detectBackgroundColor - detects background from image', async (t) => {
  const inputBuffer = await readFile(INPUT_PATH);
  const bgColor = detectBackgroundColor(inputBuffer);

  t.is(typeof bgColor.r, 'number');
  t.is(typeof bgColor.g, 'number');
  t.is(typeof bgColor.b, 'number');
  t.true(bgColor.r >= 0 && bgColor.r <= 255);
  t.true(bgColor.g >= 0 && bgColor.g <= 255);
  t.true(bgColor.b >= 0 && bgColor.b <= 255);
});

// ============================================================================
// parseColor
// ============================================================================

test('parseColor - parses 6-digit hex with hash', (t) => {
  const color = parseColor('#ff0000');
  t.deepEqual(color, { r: 255, g: 0, b: 0 });
});

test('parseColor - parses 6-digit hex without hash', (t) => {
  const color = parseColor('00ff00');
  t.deepEqual(color, { r: 0, g: 255, b: 0 });
});

test('parseColor - parses 3-digit hex with hash', (t) => {
  const color = parseColor('#f00');
  t.deepEqual(color, { r: 255, g: 0, b: 0 });
});

test('parseColor - parses 3-digit hex without hash', (t) => {
  const color = parseColor('0f0');
  t.deepEqual(color, { r: 0, g: 255, b: 0 });
});

test('parseColor - parses white', (t) => {
  const color = parseColor('#ffffff');
  t.deepEqual(color, { r: 255, g: 255, b: 255 });
});

test('parseColor - parses black', (t) => {
  const color = parseColor('#000000');
  t.deepEqual(color, { r: 0, g: 0, b: 0 });
});

test('parseColor - throws on invalid color', (t) => {
  t.throws(() => parseColor('invalid'));
});

test('parseColor - throws on wrong length', (t) => {
  t.throws(() => parseColor('#ff00'));
});

// ============================================================================
// colorToNormalized
// ============================================================================

test('colorToNormalized - normalizes white', (t) => {
  const normalized = colorToNormalized({ r: 255, g: 255, b: 255 });
  t.is(normalized.r, 1);
  t.is(normalized.g, 1);
  t.is(normalized.b, 1);
});

test('colorToNormalized - normalizes black', (t) => {
  const normalized = colorToNormalized({ r: 0, g: 0, b: 0 });
  t.is(normalized.r, 0);
  t.is(normalized.g, 0);
  t.is(normalized.b, 0);
});

test('colorToNormalized - normalizes mid-gray', (t) => {
  const normalized = colorToNormalized({ r: 128, g: 128, b: 128 });
  t.true(Math.abs(normalized.r - 0.502) < 0.01);
  t.true(Math.abs(normalized.g - 0.502) < 0.01);
  t.true(Math.abs(normalized.b - 0.502) < 0.01);
});

// ============================================================================
// normalizedToColor
// ============================================================================

test('normalizedToColor - denormalizes white', (t) => {
  const color = normalizedToColor({ r: 1, g: 1, b: 1 });
  t.deepEqual(color, { r: 255, g: 255, b: 255 });
});

test('normalizedToColor - denormalizes black', (t) => {
  const color = normalizedToColor({ r: 0, g: 0, b: 0 });
  t.deepEqual(color, { r: 0, g: 0, b: 0 });
});

test('normalizedToColor - denormalizes mid-gray', (t) => {
  const color = normalizedToColor({ r: 0.5, g: 0.5, b: 0.5 });
  t.is(color.r, 128);
  t.is(color.g, 128);
  t.is(color.b, 128);
});

test('colorToNormalized and normalizedToColor - roundtrip', (t) => {
  const original = { r: 100, g: 150, b: 200 };
  const normalized = colorToNormalized(original);
  const denormalized = normalizedToColor(normalized);
  t.deepEqual(denormalized, original);
});

// ============================================================================
// trimImage
// ============================================================================

test('trimImage - trims image to content', async (t) => {
  const inputBuffer = await readFile(INPUT_PATH);
  const processed = await processImage({
    input: inputBuffer,
    strictMode: false,
    trim: false,
  });
  const trimmed = trimImage(processed);

  t.true(Buffer.isBuffer(trimmed));
  t.true(trimmed.length > 0);
});

// ============================================================================
// unmixColor
// ============================================================================

test('unmixColor - unmixes pure foreground color', (t) => {
  const result = unmixColor({ r: 255, g: 0, b: 0 }, [{ r: 255, g: 0, b: 0 }], { r: 0, g: 0, b: 0 });

  t.true(result.weights.length === 1);
  t.true(Math.abs(result.weights[0] - 1) < 0.01);
  t.true(Math.abs(result.alpha - 1) < 0.01);
});

test('unmixColor - unmixes background color', (t) => {
  const result = unmixColor({ r: 0, g: 0, b: 0 }, [{ r: 255, g: 0, b: 0 }], { r: 0, g: 0, b: 0 });

  t.true(result.weights.length === 1);
  t.true(result.alpha < 0.01);
});

test('unmixColor - unmixes blended color', (t) => {
  const result = unmixColor({ r: 128, g: 0, b: 0 }, [{ r: 255, g: 0, b: 0 }], { r: 0, g: 0, b: 0 });

  t.true(result.weights.length === 1);
  t.true(Math.abs(result.alpha - 0.5) < 0.05);
});

test('unmixColor - unmixes with multiple foreground colors', (t) => {
  const result = unmixColor(
    { r: 128, g: 128, b: 0 },
    [
      { r: 255, g: 0, b: 0 },
      { r: 0, g: 255, b: 0 },
    ],
    { r: 0, g: 0, b: 0 },
  );

  t.true(result.weights.length === 2);
  t.true(result.alpha > 0);
});

// ============================================================================
// computeUnmixResultColor
// ============================================================================

test('computeUnmixResultColor - computes single color', (t) => {
  const rgba = computeUnmixResultColor([1], 1, [{ r: 255, g: 0, b: 0 }]);

  t.is(rgba.r, 255);
  t.is(rgba.g, 0);
  t.is(rgba.b, 0);
  t.is(rgba.a, 255);
});

test('computeUnmixResultColor - computes mixed colors', (t) => {
  const rgba = computeUnmixResultColor([0.5, 0.5], 1, [
    { r: 255, g: 0, b: 0 },
    { r: 0, g: 255, b: 0 },
  ]);

  t.is(rgba.r, 128);
  t.is(rgba.g, 128);
  t.is(rgba.b, 0);
  t.is(rgba.a, 255);
});

test('computeUnmixResultColor - respects alpha', (t) => {
  const rgba = computeUnmixResultColor([1], 0.5, [{ r: 255, g: 0, b: 0 }]);

  t.is(rgba.r, 255);
  t.is(rgba.g, 0);
  t.is(rgba.b, 0);
  t.is(rgba.a, 128);
});

// ============================================================================
// compositeOverBackground
// ============================================================================

test('compositeOverBackground - opaque pixel unchanged', (t) => {
  const result = compositeOverBackground({ r: 255, g: 0, b: 0, a: 255 }, { r: 0, g: 0, b: 0 });

  t.deepEqual(result, { r: 255, g: 0, b: 0 });
});

test('compositeOverBackground - transparent pixel returns background', (t) => {
  const result = compositeOverBackground({ r: 255, g: 0, b: 0, a: 0 }, { r: 0, g: 255, b: 0 });

  t.deepEqual(result, { r: 0, g: 255, b: 0 });
});

test('compositeOverBackground - semi-transparent pixel blends', (t) => {
  const result = compositeOverBackground({ r: 255, g: 0, b: 0, a: 128 }, { r: 0, g: 0, b: 0 });

  t.true(result.r > 100 && result.r < 150);
  t.is(result.g, 0);
  t.is(result.b, 0);
});

test('compositeOverBackground - blends with white background', (t) => {
  const result = compositeOverBackground({ r: 0, g: 0, b: 0, a: 128 }, { r: 255, g: 255, b: 255 });

  t.true(result.r > 100 && result.r < 150);
  t.true(result.g > 100 && result.g < 150);
  t.true(result.b > 100 && result.b < 150);
});

// ============================================================================
// getDefaultThreshold
// ============================================================================

test('getDefaultThreshold - returns default value', (t) => {
  const threshold = getDefaultThreshold();

  t.is(typeof threshold, 'number');
  t.is(threshold, 0.05);
});

// ============================================================================
// Error handling
// ============================================================================

test('processImage - throws on invalid image data', async (t) => {
  await t.throwsAsync(async () => {
    await processImage({
      input: Buffer.from('not an image'),
      strictMode: false,
      trim: false,
    });
  });
});

test('processImageSync - throws on invalid image data', (t) => {
  t.throws(() => {
    processImageSync({
      input: Buffer.from('not an image'),
      strictMode: false,
      trim: false,
    });
  });
});

test('processImage - throws on invalid background color', async (t) => {
  const inputBuffer = await readFile(INPUT_PATH);
  await t.throwsAsync(async () => {
    await processImage({
      input: inputBuffer,
      backgroundColor: 'invalid',
      strictMode: false,
      trim: false,
    });
  });
});

test('processImage - throws on invalid foreground color', async (t) => {
  const inputBuffer = await readFile(INPUT_PATH);
  await t.throwsAsync(async () => {
    await processImage({
      input: inputBuffer,
      foregroundColors: ['invalid'],
      strictMode: false,
      trim: false,
    });
  });
});

test('detectBackgroundColor - throws on invalid image data', (t) => {
  t.throws(() => {
    detectBackgroundColor(Buffer.from('not an image'));
  });
});

test('trimImage - throws on invalid image data', (t) => {
  t.throws(() => {
    trimImage(Buffer.from('not an image'));
  });
});
