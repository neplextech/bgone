import { Bench } from 'tinybench';
import { processImageSync, processImage, ProcessImageOptions } from '../index.js';
import { join } from 'node:path';
import { readFile } from 'node:fs/promises';

const INPUT_PATH = join(import.meta.dirname, '..', '__test__', 'assets', 'image.png');
const b = new Bench();

const inputBuffer = await readFile(INPUT_PATH);

const options: ProcessImageOptions = {
  input: inputBuffer,
  strictMode: false,
  threshold: 0.05,
  trim: false,
};

b.add('Process image synchronously', () => {
  return processImageSync(options);
});

b.add('Process image asynchronously', () => {
  return processImage(options);
});

await b.run();

console.table(b.table());
