import { execFileSync } from 'node:child_process';

function run(command, args) {
  execFileSync(command, args, { stdio: 'inherit' });
}

function output(command, args) {
  return execFileSync(command, args, { encoding: 'utf8' }).trim();
}

const artifactPaths = [
  'wasm/pkg/.gitignore',
  'wasm/pkg/aion_charts_wasm.d.ts',
  'wasm/pkg/aion_charts_wasm.js',
  'wasm/pkg/aion_charts_wasm_bg.wasm',
  'wasm/pkg/aion_charts_wasm_bg.wasm.d.ts',
  'wasm/pkg/package.json',
];

const beforeDiff = output('git', ['diff', '--binary', '--', ...artifactPaths]);
run('node', ['scripts/build-wasm-artifacts.mjs']);
const afterDiff = output('git', ['diff', '--binary', '--', ...artifactPaths]);

if (beforeDiff !== afterDiff) {
  const diff = output('git', ['status', '--porcelain', '--', ...artifactPaths]);
  console.error('\nWASM package artifacts are stale. Run `npm run build` and commit the generated files.');
  console.error(diff);
  process.exit(1);
}
