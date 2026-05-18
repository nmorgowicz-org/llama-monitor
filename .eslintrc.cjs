// ESLint config for llama-monitor frontend
// Purpose:
// - Catch ES module import/export issues (missing exports, wrong names)
// - Detect basic XSS via innerHTML/insertAdjacentHTML
// - Keep rules pragmatic; avoid noise.

const globals = require('globals');

module.exports = {
  root: true,
  env: {
    browser: true,
    es2021: true,
  },
  parserOptions: {
    ecmaVersion: 2021,
    sourceType: 'module',
  },
  plugins: [
    'import',
    'no-unsanitized',
  ],
  settings: {
    'import/resolver': {
      // Treat browser-style paths like /js/core/app-state.js as valid
      // since they are served by the Rust backend, not Node modules.
      node: {
        extensions: ['.js'],
      },
    },
  },
  rules: {
    // ES module import/export correctness
    'import/no-unresolved': [
      'error',
      {
        // Ignore browser-style absolute paths used by the app
        ignore: [
          '^/js/',
          '^/css/',
        ],
      },
    ],
    'import/named': 'error',
    'import/namespace': 'error',
    'import/default': 'error',

    // Prevent accidental overwrites of ES module bindings
    'import/no-import-assign': 'error',

    // Basic hygiene
    'no-undef': 'error',
    'no-unused-vars': [
      'warn',
      {
        argsIgnorePattern: '^_',
        varsIgnorePattern: '^_',
      },
    ],

    // XSS-related: require sanitization when using innerHTML/insertAdjacentHTML
    'no-unsanitized/method': 'warn',
    'no-unsanitized/property': 'warn',
  },

  // Ignore generated and third-party files
  ignorePatterns: [
    'node_modules/',
    'target/',
    'src/gen/',
  ],
};
