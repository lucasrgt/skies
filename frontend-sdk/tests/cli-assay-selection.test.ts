import { expect, it } from 'vitest';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import path from 'node:path';

const helper = path.resolve(process.cwd(), '../src/Skies.Framework.Cli/Tools/assay-affected.mjs');

/** A package whose pinned verifier records the argv it was handed instead of running Vitest. */
function project(assayFiles: string[], options: { verifier?: boolean } = {}) {
  const root = mkdtempSync(path.join(tmpdir(), 'skies-assay-selection-'));
  writeFileSync(path.join(root, 'package.json'), JSON.stringify({ name: 'client' }));
  for (const file of assayFiles) {
    mkdirSync(path.join(root, path.dirname(file)), { recursive: true });
    writeFileSync(path.join(root, file), 'export const verification = true;\n');
  }
  if (options.verifier !== false) {
    const pkg = path.join(root, 'node_modules', 'avp-assay');
    mkdirSync(path.join(pkg, 'bin'), { recursive: true });
    writeFileSync(path.join(pkg, 'package.json'), JSON.stringify({
      name: 'avp-assay', version: '0.4.0', type: 'module',
      exports: { './package.json': './package.json' },
      bin: { assay: 'bin/assay.mjs' },
    }));
    writeFileSync(path.join(pkg, 'bin', 'assay.mjs'),
      "import { writeFileSync } from 'node:fs';\n"
      + "writeFileSync(process.env.ASSAY_ARGV_RECEIPT, JSON.stringify(process.argv.slice(2)));\n");
  }
  return root;
}

function run(root: string, selection: string[] | '--full') {
  const receipt = path.join(root, 'argv.json');
  let argument = '--full';
  if (selection !== '--full') {
    argument = path.join(root, 'selection.json');
    writeFileSync(argument, JSON.stringify(selection));
  }
  const result = spawnSync(process.execPath, [helper, argument], {
    cwd: root, encoding: 'utf8', timeout: 20_000,
    env: { ...process.env, ASSAY_ARGV_RECEIPT: receipt },
  });
  return { ...result, receipt };
}

it('collapses a whole-surface selection into the verifier own discovery instead of naming every file', () => {
  // The regression: a package with hundreds of co-located verifications expanded past the Windows shell's
  // 8191-character argument limit, so the verifier died before running and the gate reported a failed proof.
  const files = Array.from({ length: 200 }, (_, index) =>
    `src/features/a-rather-long-feature-directory-${index}/detail/LongComponentName${index}.assay.test.tsx`);
  const root = project(files);
  try {
    expect(files.join(' ').length).toBeGreaterThan(8191);
    const result = run(root, files);
    expect(result.status).toBe(0);
    expect(JSON.parse(readFileSync(result.receipt, 'utf8'))).toEqual(['verify', '--', '--maxWorkers=2']);
    expect(result.stdout).toContain('the complete Assay surface (200 file(s))');
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

it('hands an affected subset to the verifier verbatim without widening it to the surface', () => {
  const root = project(['src/a/A.assay.test.tsx', 'src/b/B.assay.test.tsx']);
  try {
    const result = run(root, ['src/a/A.assay.test.tsx']);
    expect(result.status).toBe(0);
    expect(JSON.parse(readFileSync(result.receipt, 'utf8')))
      .toEqual(['verify', 'src/a/A.assay.test.tsx', '--', '--maxWorkers=2']);
    expect(result.stdout).toContain('1 selected verification(s)');
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

it('runs the pinned verifier discovery for an explicit full surface', () => {
  const root = project(['src/a/A.assay.test.tsx']);
  try {
    const result = run(root, '--full');
    expect(result.status).toBe(0);
    expect(JSON.parse(readFileSync(result.receipt, 'utf8'))).toEqual(['verify', '--', '--maxWorkers=2']);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

it('reports a verifier it cannot invoke as incomplete verification, never as a failed proof', () => {
  const root = project(['src/a/A.assay.test.tsx'], { verifier: false });
  try {
    const result = run(root, ['src/a/A.assay.test.tsx']);
    expect(result.status).toBe(2);
    expect(result.stderr).toContain('could not be invoked');
    expect(result.stderr).toContain('Incomplete verification, not a failed proof');
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

it.each(['null', '[]', '["--reporter=json"]'])('refuses the malformed affected selection %s', (selection) => {
  const root = project(['src/a/A.assay.test.tsx']);
  try {
    writeFileSync(path.join(root, 'selection.json'), selection);
    const result = spawnSync(process.execPath, [helper, path.join(root, 'selection.json')], {
      cwd: root, encoding: 'utf8', timeout: 20_000,
      env: { ...process.env, ASSAY_ARGV_RECEIPT: path.join(root, 'argv.json') },
    });
    expect(result.status).toBe(2);
    expect(result.stdout).not.toContain('selected verification(s)');
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
