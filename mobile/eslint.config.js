const { defineConfig, globalIgnores } = require('eslint/config');
const expoConfig = require('eslint-config-expo/flat');

module.exports = defineConfig([
  globalIgnores(['dist/*']),
  expoConfig,
  {
    // Jest's `jest.mock(...)` calls must run before the mocked module is
    // imported, which `import/first` and `import/no-duplicates` don't
    // expect — and the AsyncStorage jest mock is required via `require()`
    // specifically so it isn't hoisted above that `jest.mock()` call. Both
    // are idiomatic for this test setup, not oversights.
    files: ['**/__tests__/**', '**/*.test.{ts,tsx}'],
    rules: {
      'import/first': 'off',
      'import/no-duplicates': 'off',
      '@typescript-eslint/no-require-imports': 'off',
    },
  },
]);
