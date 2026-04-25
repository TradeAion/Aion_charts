import { execFileSync } from 'node:child_process';
import { writeFileSync } from 'node:fs';

function run(command, args) {
  execFileSync(command, args, { stdio: 'inherit' });
}

function output(command, args) {
  return execFileSync(command, args, { encoding: 'utf8' }).trim();
}

run('wasm-pack', ['build', '--target', 'web', 'wasm']);
writeFileSync(
  'wasm/pkg/.gitignore',
  '!axiuscharts_wasm_bg.wasm\n!axiuscharts_wasm_bg.wasm.d.ts\n!axiuscharts_wasm.d.ts\n!axiuscharts_wasm.js\n!package.json\n',
);

const artifactPaths = [
  'wasm/pkg/.gitignore',
  'wasm/pkg/axiuscharts_wasm.d.ts',
  'wasm/pkg/axiuscharts_wasm.js',
  'wasm/pkg/axiuscharts_wasm_bg.wasm',
  'wasm/pkg/axiuscharts_wasm_bg.wasm.d.ts',
  'wasm/pkg/package.json',
];

const diff = output('git', ['status', '--porcelain', '--', ...artifactPaths]);
if (diff) {
  console.error('\nWASM package artifacts are stale. Run `npm run build` and commit the generated files.');
  console.error(diff);
  process.exit(1);
}
