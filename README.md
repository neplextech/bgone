# bgone

Node.js library to remove solid background colors from images without AI/ML based on [bgone](https://github.com/benface/bgone) by Benface.

## Features

- **Automatic background detection** - Detects solid background colors by sampling image edges
- **Color unmixing** - Separates foreground from background using advanced alpha blending algorithms
- **Foreground color deduction** - Automatically deduce unknown foreground colors using `"auto"`
- **Strict and non-strict modes** - Choose between exact color matching or flexible unmixing
- **Parallel processing** - Utilizes all CPU cores for maximum performance
- **Cross-platform** - Works on Windows, macOS, Linux, and more

## Example Result

| Input                                                                                      | Output                                                                                                |
| ------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------- |
| ![Input](https://github.com/neplextech/bgone/blob/main/__test__/assets/image.png?raw=true) | ![Output](https://github.com/neplextech/bgone/blob/main/__test__/assets/image-processed.png?raw=true) |

_Example image from [Unsplash](https://unsplash.com/photos/orange-fire-on-black-background-D9MDpAjlJ6k)_

## Installation

```bash
npm install @neplex/bgone
# or
yarn add @neplex/bgone
# or
pnpm add @neplex/bgone
# or
bun add @neplex/bgone
# or
deno add npm:@neplex/bgone
```

## Quick Start

```typescript
import { readFileSync, writeFileSync } from 'fs';
import { processImage, processImageSync, detectBackgroundColor } from '@neplex/bgone';

const input = readFileSync('input.png');

// Async processing (recommended for large images)
const output = await processImage({
  input,
  strictMode: false,
  trim: false,
});
writeFileSync('output.png', output);

// Sync processing
const outputSync = processImageSync({
  input,
  strictMode: false,
  trim: false,
});
```

## CLI Usage

You can use bgone directly from the command line:

```bash
# Using npx (no installation required)
npx @neplex/bgone input.png
```

### CLI Options

```
Usage: npx @neplex/bgone [options] <input> [output]

Remove solid background colors from images

Arguments:
  input                  Input image file
  output                 Output image file (defaults to input-bgone.png)

Options:
  -V, --version          output the version number
  -b, --bg <color>       Background color to remove (hex, e.g. #ffffff or fff)
  -f, --fg <colors...>   Foreground colors (hex or "auto" for deduction)
  -s, --strict           Strict mode - only use specified foreground colors
  -t, --threshold <value> Color closeness threshold (0.0-1.0)
  --trim                 Trim output to content bounding box
  --detect               Only detect and print background color, do not process
  -h, --help             display help for command
```

### CLI Examples

```bash
# Fully automatic - detects background and removes it
npx @neplex/bgone input.png

# Specify output path
npx @neplex/bgone input.png output.png

# With explicit background color
npx @neplex/bgone input.png --bg=#ffffff
npx @neplex/bgone input.png -b fff

# With foreground color for optimized opacity
npx @neplex/bgone input.png --fg=#ff0000

# Multiple foreground colors
npx @neplex/bgone input.png --fg ff0000 00ff00 0000ff

# Foreground color deduction
npx @neplex/bgone input.png --fg auto
npx @neplex/bgone input.png --fg auto auto --bg ffffff

# Mix known and unknown colors
npx @neplex/bgone input.png --fg ff0000 auto

# Strict mode with foreground colors
npx @neplex/bgone input.png --strict --fg=#ff0000
npx @neplex/bgone input.png -s --fg auto

# With threshold and trim
npx @neplex/bgone input.png -f f00 0f0 00f -b fff -t 0.1 --trim

# Only detect background color
npx @neplex/bgone input.png --detect
```

## API Reference

### Types

```typescript
interface RgbColor {
  r: number; // 0-255
  g: number; // 0-255
  b: number; // 0-255
}

interface RgbaColor {
  r: number; // 0-255
  g: number; // 0-255
  b: number; // 0-255
  a: number; // 0-255
}

interface NormalizedRgbColor {
  r: number; // 0.0-1.0
  g: number; // 0.0-1.0
  b: number; // 0.0-1.0
}

interface ProcessImageOptions {
  /** The input image buffer (PNG, JPEG, etc.) */
  input: Buffer;
  /** Foreground colors as hex strings. Use "auto" to deduce unknown colors. */
  foregroundColors?: string[];
  /** Background color as hex string. Auto-detected if not specified. */
  backgroundColor?: string;
  /** Restricts unmixing to only the specified foreground colors. */
  strictMode: boolean;
  /** Threshold for color closeness (0.0-1.0, default: 0.05) */
  threshold?: number;
  /** Trim output to bounding box of non-transparent pixels. */
  trim: boolean;
}

interface UnmixResult {
  /** Weight for each foreground color */
  weights: number[];
  /** Overall alpha value (0.0-1.0) */
  alpha: number;
}
```

### Image Processing

#### `processImage(options: ProcessImageOptions): Promise<Buffer>`

Process an image asynchronously to remove its background. Returns a Promise that resolves to the processed image buffer (PNG format).

```typescript
// Fully automatic - detects background and removes it
const output = await processImage({
  input: imageBuffer,
  strictMode: false,
  trim: false,
});

// With explicit background color
const output = await processImage({
  input: imageBuffer,
  backgroundColor: '#ffffff',
  strictMode: false,
  trim: true,
});

// With foreground colors for optimized opacity
const output = await processImage({
  input: imageBuffer,
  foregroundColors: ['#ff0000', '#00ff00'],
  backgroundColor: '#ffffff',
  strictMode: false,
  trim: false,
});

// Foreground color deduction using "auto"
const output = await processImage({
  input: imageBuffer,
  foregroundColors: ['auto'],
  backgroundColor: '#ffffff',
  strictMode: false,
  trim: false,
});

// Mix known and unknown colors
const output = await processImage({
  input: imageBuffer,
  foregroundColors: ['#ff0000', 'auto'],
  strictMode: true,
  trim: false,
});

// Strict mode - restricts to exact foreground colors
const output = await processImage({
  input: imageBuffer,
  foregroundColors: ['#ff0000'],
  backgroundColor: '#ffffff',
  strictMode: true,
  trim: false,
});
```

#### `processImageSync(options: ProcessImageOptions): Buffer`

Synchronous version of `processImage`. Use for smaller images or when async is not needed.

```typescript
const output = processImageSync({
  input: imageBuffer,
  strictMode: false,
  trim: false,
});
```

### Background Detection

#### `detectBackgroundColor(input: Buffer): RgbColor`

Detect the background color of an image by sampling its edges and corners.

```typescript
const bgColor = detectBackgroundColor(imageBuffer);
console.log(`Background: rgb(${bgColor.r}, ${bgColor.g}, ${bgColor.b})`);
```

### Image Utilities

#### `trimImage(input: Buffer): Buffer`

Trim an image to the bounding box of non-transparent pixels.

```typescript
const trimmed = trimImage(imageBuffer);
```

### Color Utilities

#### `parseColor(hex: string): RgbColor`

Parse a hex color string into an RGB color. Supports formats: `"#ff0000"`, `"ff0000"`, `"#f00"`, `"f00"`.

```typescript
const red = parseColor('#ff0000');
// { r: 255, g: 0, b: 0 }

const green = parseColor('0f0');
// { r: 0, g: 255, b: 0 }
```

#### `colorToNormalized(color: RgbColor): NormalizedRgbColor`

Convert an RGB color (0-255) to a normalized RGB color (0.0-1.0).

```typescript
const normalized = colorToNormalized({ r: 255, g: 128, b: 0 });
// { r: 1.0, g: 0.502, b: 0.0 }
```

#### `normalizedToColor(color: NormalizedRgbColor): RgbColor`

Convert a normalized RGB color (0.0-1.0) to an RGB color (0-255).

```typescript
const rgb = normalizedToColor({ r: 1.0, g: 0.5, b: 0.0 });
// { r: 255, g: 128, b: 0 }
```

### Color Unmixing

#### `unmixColor(observed: RgbColor, foregroundColors: RgbColor[], background: RgbColor): UnmixResult`

Unmix an observed color into foreground color components. Given an observed color and known foreground/background colors, determines how much of each foreground color contributed to the observed color.

```typescript
const result = unmixColor(
  { r: 128, g: 0, b: 0 }, // observed color
  [{ r: 255, g: 0, b: 0 }], // foreground colors
  { r: 0, g: 0, b: 0 }, // background
);
console.log(result.weights); // [0.502...]
console.log(result.alpha); // 0.502...
```

#### `computeUnmixResultColor(weights: number[], alpha: number, foregroundColors: RgbColor[]): RgbaColor`

Compute the final RGBA color from an unmix result.

```typescript
const rgba = computeUnmixResultColor([0.5, 0.5], 1.0, [
  { r: 255, g: 0, b: 0 },
  { r: 0, g: 255, b: 0 },
]);
// { r: 128, g: 128, b: 0, a: 255 }
```

#### `compositeOverBackground(pixel: RgbaColor, background: RgbColor): RgbColor`

Composite an RGBA pixel over an RGB background color. If the pixel is translucent (alpha < 255), pre-composes it over the background to produce an opaque equivalent.

```typescript
const result = compositeOverBackground({ r: 255, g: 0, b: 0, a: 128 }, { r: 0, g: 0, b: 0 });
// { r: 128, g: 0, b: 0 }
```

### Constants

#### `getDefaultThreshold(): number`

Get the default threshold for color closeness (0.05 = 5% of max RGB distance).

```typescript
const threshold = getDefaultThreshold();
// 0.05
```

## Processing Modes

### Non-Strict Mode (default)

In non-strict mode, the algorithm finds the optimal foreground color and alpha that produces the observed color when alpha-blended with the background. This mode:

- Works without specifying foreground colors
- Allows any color to be used as foreground
- Optimizes for minimum alpha (maximum transparency)
- Always produces perfect reconstruction of the original image

### Non-Strict Mode with Foreground Colors

When foreground colors are specified in non-strict mode:

- Pixels close to specified foreground colors use the optimized unmixing algorithm
- Pixels NOT close to any foreground color can use ANY color (preserves glows, gradients, etc.)
- Uses the `threshold` option to determine "closeness"

### Strict Mode

Strict mode restricts unmixing to only the specified foreground colors:

- Requires at least one foreground color (can be `"auto"` for deduction)
- Output pixels can only be a mix of the specified foreground colors
- Best for images with known, limited color palettes

## Foreground Color Deduction

Use `"auto"` in the `foregroundColors` array to automatically deduce unknown colors:

```typescript
// Deduce one unknown color
const output = await processImage({
  input,
  foregroundColors: ['auto'],
  strictMode: true,
  trim: false,
});

// Mix known and unknown colors
const output = await processImage({
  input,
  foregroundColors: ['#ff0000', 'auto', 'auto'],
  strictMode: true,
  trim: false,
});
```

## Performance

The library uses Rayon for parallel processing, utilizing all available CPU cores. For best performance:

- Use `processImage` (async) for large images to avoid blocking the event loop
- Use `processImageSync` for small images or batch processing
- Consider using worker threads for processing multiple images

## License

MIT

## Credits

Based on [bgone](https://github.com/benface/bgone) by Benface.
