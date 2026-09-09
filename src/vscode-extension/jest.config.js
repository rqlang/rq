module.exports = {
  preset: 'ts-jest',
  testEnvironment: 'node',
  moduleNameMapper: {
    '^vscode$': '<rootDir>/test/vscode-mock.ts'
  },
  transform: {
    '^.+\\.tsx?$': ['ts-jest', { tsconfig: 'tsconfig.test.json' }],
    '^.+\\.md$': '<rootDir>/test/mdTransformer.js'
  },
  transformIgnorePatterns: ['/node_modules/(?!@modelcontextprotocol)']
};
