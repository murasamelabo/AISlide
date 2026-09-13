import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { Presentation } from 'lucide-react';
import sharp from 'sharp';
import { mkdir } from 'node:fs/promises';

const destination = 'apps/studio/src-tauri/icons';
await mkdir(destination, { recursive: true });
const markup = renderToStaticMarkup(createElement(Presentation, { size: 1024, color: 'white', strokeWidth: 1.25 }));
await sharp(Buffer.from(markup)).resize(640, 640).extend({ top: 192, bottom: 192, left: 192, right: 192, background: '#087f73' }).flatten({ background: '#087f73' }).png().toFile(`${destination}/app.png`);