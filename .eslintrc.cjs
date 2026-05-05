const globals = require('globals');

module.exports = {
  env: {
    browser: true,
    es2022: true,
    node: true,
  },
  parserOptions: {
    ecmaVersion: 2022,
    sourceType: 'module',
  },
  plugins: ['no-unsanitized'],
  rules: {
    'no-unsanitized/property': 'error',
    'no-unsanitized/method': 'error',
  },
  settings: {
    'import/resolver': {
      node: {
        extensions: ['.js'],
      },
    },
  },
  overrides: [
    {
      files: ['static/js/**/*.js'],
      env: {
        browser: true,
        es2022: true,
      },
      globals: {
        ...globals.browser,
        window: 'readonly',
        document: 'readonly',
        DOMPurify: 'readonly',
        marked: 'readonly',
        hljs: 'readonly',
      },
    },
  ],
};
