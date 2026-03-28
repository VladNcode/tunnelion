import eslintConfigPrettier from 'eslint-config-prettier/flat';
import neostandard, { resolveIgnoresFromGitignore } from 'neostandard';

export default [
  ...neostandard({
    ts: true,
    // Let Prettier own formatting; eslint-config-prettier turns off overlapping rules.
    noStyle: true,
    ignores: [...resolveIgnoresFromGitignore(), '**/*.d.ts'],
  }),
  {
    name: 'tunnelion-js/extra',
    files: ['**/*.{ts,mts,cts}'],
    rules: {
      // Catches `if (a === b === c)`-style mistakes (not covered by no-constant-condition alone).
      'no-constant-binary-expression': 'error',
      'prefer-object-has-own': 'error',
      '@typescript-eslint/consistent-type-imports': [
        'error',
        { prefer: 'type-imports', fixStyle: 'separate-type-imports' },
      ],
      '@typescript-eslint/no-import-type-side-effects': 'error',
      '@typescript-eslint/no-non-null-assertion': 'warn',
    },
  },
  eslintConfigPrettier,
];
