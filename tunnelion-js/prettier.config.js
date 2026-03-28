/** @type {import('prettier').Config} */
export default {
  tabWidth: 2,
  useTabs: false,
  semi: true,
  singleQuote: true,
  trailingComma: 'all',
  bracketSpacing: true,
  arrowParens: 'avoid',
  printWidth: 120,
  endOfLine: 'auto',
  bracketSameLine: true,
  plugins: ['@ianvs/prettier-plugin-sort-imports'],
  // Value imports first; `import type` last (see plugin README §4 inverted).
  importOrder: ['<BUILTIN_MODULES>', '', '<THIRD_PARTY_MODULES>', '', '^[.]', '', '<TYPES>'],
  importOrderParserPlugins: ['typescript'],
  importOrderTypeScriptVersion: '5.0.0',
};
