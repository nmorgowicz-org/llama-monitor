import noUnsanitized from 'eslint-plugin-no-unsanitized';
import globals from 'globals';

export default [
    {
        files: ['static/js/**/*.js'],
        plugins: {
            'no-unsanitized': noUnsanitized,
        },
        languageOptions: {
            ecmaVersion: 2022,
            sourceType: 'module',
            globals: {
                ...globals.browser,
                // Set by inline <script> in index.html via Rust template substitution
                APP_VERSION: 'readonly',
                APP_PLATFORM: 'readonly',
                // CDN libraries loaded via <script src="..."> in index.html
                marked: 'readonly',
                hljs: 'readonly',
            },
        },
        rules: {
            // Catches: assignment to imported module bindings (the TypeError loop we hit)
            'no-import-assign': 'error',

            // Catches: bare references to functions that are no longer on window
            'no-undef': 'error',

            // Catches: innerHTML/outerHTML with unescaped user data
            // Allow escapeHtml() as the approved sanitizer
            'no-unsanitized/property': ['error', {
                escape: { methods: ['escapeHtml'] },
            }],
            'no-unsanitized/method': ['error', {
                escape: { methods: ['escapeHtml'] },
            }],
        },
    },
    {
        // compat/globals.js deliberately assigns to window — that's its only job
        files: ['static/js/compat/globals.js'],
        rules: {
            'no-undef': 'off',
        },
    },
];
