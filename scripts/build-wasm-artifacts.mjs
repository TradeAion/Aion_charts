import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';

const rootPackage = JSON.parse(readFileSync('package.json', 'utf8'));

execFileSync('wasm-pack', ['build', '--target', 'web', ...process.argv.slice(2), 'wasm'], {
  stdio: 'inherit',
});

writeFileSync(
  'wasm/pkg/.gitignore',
  '!aion_charts_wasm_bg.wasm\n!aion_charts_wasm_bg.wasm.d.ts\n!aion_charts_wasm.d.ts\n!aion_charts_wasm.js\n!package.json\n',
);

const generatedPackagePath = 'wasm/pkg/package.json';
const generatedPackage = JSON.parse(readFileSync(generatedPackagePath, 'utf8'));
generatedPackage.name = rootPackage.name;
generatedPackage.version = rootPackage.version;
generatedPackage.description = rootPackage.description;
generatedPackage.license = rootPackage.license;
generatedPackage.repository = rootPackage.repository;
generatedPackage.publishConfig = rootPackage.publishConfig;

writeFileSync(`${generatedPackagePath}`, `${JSON.stringify(generatedPackage, null, 2)}\n`);
