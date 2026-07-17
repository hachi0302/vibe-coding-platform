module.exports = {
  extends: ['@commitlint/config-conventional'],
  rules: {
    'type-enum': [
      2,
      'always',
      ['feat', 'fix', 'perf', 'doc', 'style', 'update', 'refactor', 'test', 'framework', 'revert', 'ci', 'release'],
    ],
    // lowercase conventional types — changelogithub's zero-config defaults
    // group feat/fix/perf into the release notes (the rest are allowed here but
    // won't appear in the changelog, matching the minimal vue3-toastify setup).
    'type-case': [2, 'always', 'lower-case'],
    'type-empty': [0],
    'scope-empty': [0],
    'scope-case': [0],
    'subject-full-stop': [0, 'never'],
    'subject-case': [0, 'never'],
    'header-max-length': [0, 'always', 72],
  },
};
