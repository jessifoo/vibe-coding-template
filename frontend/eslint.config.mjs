// @ts-check
/**
 * ESLint flat-config for the frontend.
 *
 * Replaces `.eslintrc.json` (deleted). ESLint 9 dropped support for the legacy
 * `.eslintrc.*` format; this is the migration.
 *
 * Structure (order matters — later configs override earlier ones):
 *   1. Global ignores (replaces .eslintignore).
 *   2. Base JavaScript recommendations from `@eslint/js`.
 *   3. TypeScript recommendations from the `typescript-eslint` meta package.
 *   4. `next/core-web-vitals` via FlatCompat — eslint-config-next 15.x still
 *      exports the legacy `.eslintrc` shape; the Next 16 line ships a native
 *      flat config, at which point this file drops FlatCompat entirely.
 *   5. Project-specific rule overrides that mirror the old .eslintrc.json.
 *
 * The `lint` script in package.json runs `eslint .` directly rather than
 * `next lint` (which is deprecated in Next 16). Doing the CLI swap here keeps
 * the future Next 15 -> 16 upgrade small.
 */

import { FlatCompat } from '@eslint/eslintrc';
import js from '@eslint/js';
import tseslint from 'typescript-eslint';
import { dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const compat = new FlatCompat({
  baseDirectory: __dirname,
  recommendedConfig: js.configs.recommended,
});

export default tseslint.config(
  // 1. Ignores. Must be a standalone object with only `ignores` so ESLint
  //    treats it as a global-ignore config rather than a per-file config.
  {
    ignores: [
      '.next/**',
      'node_modules/**',
      'next-env.d.ts',
      'eslint.config.mjs', // don't lint this file with the project's TS rules
    ],
  },

  // 2. ESLint recommended.
  js.configs.recommended,

  // 3. typescript-eslint recommended. Untyped (fast) preset — the project
  //    doesn't use type-aware rules like `no-floating-promises` yet. If we
  //    add those later, swap for `tseslint.configs.recommendedTypeChecked`
  //    and configure `parserOptions.project`.
  ...tseslint.configs.recommended,

  // 4. Next.js rules via legacy-config compat bridge.
  ...compat.extends('next/core-web-vitals'),

  // 5. Project rules (verbatim from the previous .eslintrc.json).
  {
    rules: {
      'no-console': ['warn', { allow: ['warn', 'error'] }],
      'no-debugger': 'error',
      '@typescript-eslint/no-explicit-any': 'error',
    },
  },
);
