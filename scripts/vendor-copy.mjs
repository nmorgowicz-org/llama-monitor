// scripts/vendor-copy.mjs
//
// Copy vendor distribution files from node_modules into static/vendor
// so we can self-host key third-party scripts and styles.
//
// Run after installing/updating dependencies to refresh vendor files:
//   node scripts/vendor-copy.mjs
//
// This keeps:
// - no CDN dependencies (for these libs)
// - stable, version-controlled files
// - Renovate-managed updates via package.json

import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const ROOT = path.resolve(import.meta.dirname, '..');

function src(...parts) {
  return path.join(ROOT, 'node_modules', ...parts);
}

function dest(...parts) {
  return path.join(ROOT, 'static', 'vendor', ...parts);
}

function ensureDir(filePath) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
}

function copy(srcPath, destPath) {
    ensureDir(destPath);
    const out = fs.readFileSync(srcPath);
    fs.writeFileSync(destPath, out);
    console.log(`copy: ${srcPath} -> ${destPath}`);
}

function copyCommonJsBrowserGlobal(srcPath, destPath, globalName) {
    ensureDir(destPath);
    const source = fs.readFileSync(srcPath, 'utf8');
    const out = `(() => {
    const module = { exports: {} };
    const exports = module.exports;
${source}
    globalThis.${globalName} = module.exports;
})();
`;
    fs.writeFileSync(destPath, out);
    console.log(`copy: ${srcPath} -> ${destPath}`);
}

// marked
copy(src('marked', 'marked.min.js'), dest('js', 'marked.min.js'));

// DOMPurify (dompurify package)
copy(
  src('dompurify', 'dist', 'purify.min.js'),
  dest('js', 'purify.min.js'),
);

// highlight.js core
copyCommonJsBrowserGlobal(
    src('highlight.js', 'lib', 'core.js'),
    dest('js', 'highlight.min.js'),
    'hljs',
);

// highlight.js theme (atom-one-dark)
copy(
  src('highlight.js', 'styles', 'atom-one-dark.min.css'),
  dest('css', 'atom-one-dark.min.css'),
);

console.log('vendor-copy complete');
