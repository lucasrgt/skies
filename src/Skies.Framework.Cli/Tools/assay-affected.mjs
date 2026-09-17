import { readFileSync, readdirSync } from 'node:fs';
import { createRequire } from 'node:module';
import { spawnSync } from 'node:child_process';
import path from 'node:path';

// The AVP leg hands Assay its selection through a file instead of a command line. A package with a few
// hundred co-located verifications expands past cmd.exe's 8191-character argument limit, which kills the
// verifier before it starts — and a verifier that never ran is not a proof that failed. Invocation
// failures therefore exit 2 (incomplete verification), never 1 (findings).
const EXCLUDED = new Set(['node_modules', 'dist', 'build', 'coverage', 'out', 'storybook-static']);

function assaySurface(directory) {
  const found = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    if (entry.name.startsWith('.') || EXCLUDED.has(entry.name)) continue;
    const child = path.join(directory, entry.name);
    if (entry.isDirectory()) found.push(...assaySurface(child));
    else if (entry.name.includes('.assay.test.')) found.push(child);
  }
  return found;
}

/** Resolve the verifier the project pins, never a package script that could stub acceptance with exit zero. */
function resolveAssay() {
  const require = createRequire(path.join(process.cwd(), 'package.json'));
  const manifestPath = require.resolve('avp-assay/package.json');
  const declared = JSON.parse(readFileSync(manifestPath, 'utf8')).bin;
  const entry = typeof declared === 'string' ? declared : declared?.assay;
  if (!entry) throw new Error('the installed avp-assay declares no `assay` executable');
  return path.join(path.dirname(manifestPath), entry);
}

try {
  const argument = process.argv[2];
  const full = argument === '--full';
  const selected = full ? [] : JSON.parse(readFileSync(argument, 'utf8'));
  if (!full && (!Array.isArray(selected) || selected.length === 0
      || selected.some((file) => typeof file !== 'string' || file.length === 0 || file.startsWith('-'))))
    throw new Error('an affected AVP run requires a nonempty file selection; use --full for the whole surface');

  const bin = resolveAssay();
  const discovered = assaySurface(process.cwd()).map((file) => path.relative(process.cwd(), file));
  const normalize = (file) => path.normalize(file).toLowerCase();
  const chosen = new Set(selected.map(normalize));
  // Naming every file of the whole surface is the same run as Assay's own discovery — and the only
  // selection large enough to threaten an argument limit. Collapse it instead of enumerating it.
  const whole = full || (discovered.length > 0 && discovered.every((file) => chosen.has(normalize(file))));
  const filters = whole ? [] : selected;

  process.stdout.write(`skies gate — frontend AVP: ${whole
    ? `the complete Assay surface (${discovered.length} file(s))`
    : `${filters.length} selected verification(s)`}, two workers\n`);
  const result = spawnSync(process.execPath, [bin, 'verify', ...filters, '--', '--maxWorkers=2'], { stdio: 'inherit' });
  // A missing status is never a verdict: spawnSync reports a launch failure through `error` and a killed child
  // through `signal`, and both mean no verification ran. Route them to the incomplete exit rather than to 1.
  if (result.error) throw result.error;
  if (result.status === null) throw new Error(`the verifier was terminated by ${result.signal ?? 'an unknown signal'}`);
  process.exitCode = result.status;
} catch (error) {
  process.stderr.write(`skies gate — frontend AVP: the verifier could not be invoked — ${error.message}. `
    + 'Incomplete verification, not a failed proof.\n');
  process.exitCode = 2;
}
