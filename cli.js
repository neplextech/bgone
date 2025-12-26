#!/usr/bin/env node

const { program } = require('commander');
const { readFileSync, writeFileSync, existsSync } = require('fs');
const { basename, extname, dirname, join } = require('path');
const { processImageSync, detectBackgroundColor } = require('./index.js');

const packageJson = require('./package.json');

program
  .name('bgone')
  .description('Remove solid background colors from images')
  .version(packageJson.version)
  .argument('<input>', 'Input image file')
  .argument('[output]', 'Output image file (defaults to input-bgone.png)')
  .option('-b, --bg <color>', 'Background color to remove (hex, e.g. #ffffff or fff)')
  .option('-f, --fg <colors...>', 'Foreground colors (hex or "auto" for deduction)')
  .option('-s, --strict', 'Strict mode - only use specified foreground colors', false)
  .option('-t, --threshold <value>', 'Color closeness threshold (0.0-1.0)', parseFloat)
  .option('--trim', 'Trim output to content bounding box', false)
  .option('--detect', 'Only detect and print background color, do not process')
  .action((input, output, options) => {
    if (!existsSync(input)) {
      console.error(`Error: Input file not found: ${input}`);
      process.exit(1);
    }

    const inputBuffer = readFileSync(input);

    if (options.detect) {
      const bgColor = detectBackgroundColor(inputBuffer);
      const hex = `#${bgColor.r.toString(16).padStart(2, '0')}${bgColor.g.toString(16).padStart(2, '0')}${bgColor.b.toString(16).padStart(2, '0')}`;
      console.log(`Detected background color: ${hex} (rgb(${bgColor.r}, ${bgColor.g}, ${bgColor.b}))`);
      return;
    }

    const outputPath = output || generateOutputPath(input);

    console.log(`Processing: ${input}`);

    if (options.bg) {
      console.log(`  Background: ${options.bg}`);
    } else {
      const bgColor = detectBackgroundColor(inputBuffer);
      const hex = `#${bgColor.r.toString(16).padStart(2, '0')}${bgColor.g.toString(16).padStart(2, '0')}${bgColor.b.toString(16).padStart(2, '0')}`;
      console.log(`  Background: ${hex} (auto-detected)`);
    }

    if (options.fg) {
      console.log(`  Foreground: ${options.fg.join(', ')}`);
    }

    if (options.strict) {
      console.log(`  Mode: strict`);
    }

    if (options.threshold !== undefined) {
      console.log(`  Threshold: ${options.threshold}`);
    }

    if (options.trim) {
      console.log(`  Trim: enabled`);
    }

    try {
      const result = processImageSync({
        input: inputBuffer,
        backgroundColor: options.bg,
        foregroundColors: options.fg,
        strictMode: options.strict,
        threshold: options.threshold,
        trim: options.trim,
      });

      writeFileSync(outputPath, result);
      console.log(`Output: ${outputPath}`);
    } catch (error) {
      console.error(`Error: ${error.message}`);
      process.exit(1);
    }
  });

function generateOutputPath(input) {
  const dir = dirname(input);
  const ext = extname(input);
  const name = basename(input, ext);
  let outputPath = join(dir, `${name}-bgone.png`);
  let counter = 1;

  while (existsSync(outputPath)) {
    outputPath = join(dir, `${name}-bgone-${counter}.png`);
    counter++;
  }

  return outputPath;
}

program.parse();
