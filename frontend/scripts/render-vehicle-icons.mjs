// Render each vehicle type's 3D model to a transparent PNG icon in
// src/assets/3d/vehicles/. Run after changing a model or the scene lighting:
//
//   npm run render:vehicle-icons
//
// Starts a Vite dev server, opens scripts/render-vehicle-icons.html in
// headless Chromium once per type, then crops the canvas to the vehicle
// (square, with even padding) and scales it to ICON_SIZE.
import { fileURLToPath } from 'node:url';
import { writeFile } from 'node:fs/promises';
import path from 'node:path';
import { createServer } from 'vite';
import { chromium } from '@playwright/test';

const ICON_SIZE = 192;
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const outDir = path.join(root, 'src/assets/3d/vehicles');
const types = ['Truck', 'Tipper', 'Trailer', 'Tempo', 'Pickup', 'Tanker'];

const server = await createServer({ root, server: { port: 5199, strictPort: true }, logLevel: 'error' });
await server.listen();
const browser = await chromium.launch({ args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader'] });
try {
  const page = await browser.newPage({ viewport: { width: 640, height: 640 }, deviceScaleFactor: 1 });
  for (const type of types) {
    await page.goto(`http://localhost:5199/scripts/render-vehicle-icons.html?type=${type}`);
    await page.waitForFunction(() => window.__vehicleReady === true, null, { timeout: 30_000 });
    const dataUrl = await page.evaluate(size => {
      const src = document.querySelector('canvas');
      const { width: w, height: h } = src;
      const scratch = Object.assign(document.createElement('canvas'), { width: w, height: h });
      const sctx = scratch.getContext('2d');
      sctx.drawImage(src, 0, 0);
      const alpha = sctx.getImageData(0, 0, w, h).data;
      let minX = w, minY = h, maxX = -1, maxY = -1;
      for (let y = 0; y < h; y++) {
        for (let x = 0; x < w; x++) {
          if (alpha[(y * w + x) * 4 + 3] > 8) {
            if (x < minX) minX = x;
            if (x > maxX) maxX = x;
            if (y < minY) minY = y;
            if (y > maxY) maxY = y;
          }
        }
      }
      if (maxX < 0) throw new Error('canvas is empty');
      const side = Math.max(maxX - minX, maxY - minY) * 1.08;
      const cx = (minX + maxX) / 2, cy = (minY + maxY) / 2;
      const out = Object.assign(document.createElement('canvas'), { width: size, height: size });
      const octx = out.getContext('2d');
      octx.imageSmoothingQuality = 'high';
      octx.drawImage(scratch, cx - side / 2, cy - side / 2, side, side, 0, 0, size, size);
      return out.toDataURL('image/png');
    }, ICON_SIZE);
    const file = path.join(outDir, `${type.toLowerCase()}.png`);
    await writeFile(file, Buffer.from(dataUrl.split(',')[1], 'base64'));
    console.log(`rendered ${path.relative(root, file)}`);
  }
} finally {
  await browser.close();
  await server.close();
}
