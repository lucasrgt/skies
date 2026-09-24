// Builds the per-platform npm package that carries one prebuilt `skies` binary. Called by the publish workflow
// once per target: node cli/npm/pack-platform.mjs <npm-platform> <npm-arch> <binary-path> <out-dir>
import { chmodSync, copyFileSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { basename, join } from "node:path";

const [platform, arch, binary, out] = process.argv.slice(2);
if (!platform || !arch || !binary || !out) {
  console.error("usage: pack-platform.mjs <platform> <arch> <binary> <out-dir>");
  process.exit(2);
}

const { version } = JSON.parse(readFileSync(new URL("./package.json", import.meta.url), "utf8"));
mkdirSync(out, { recursive: true });
const target = join(out, basename(binary));
copyFileSync(binary, target);
chmodSync(target, 0o755);
writeFileSync(
  join(out, "package.json"),
  JSON.stringify(
    {
      name: `@skiesjs/cli-${platform}-${arch}`,
      version,
      description: `The skies binary for ${platform}-${arch}.`,
      license: "MIT",
      repository: { type: "git", url: "git+https://github.com/lucasrgt/skies.git" },
      os: [platform],
      cpu: [arch],
      files: [basename(binary)],
    },
    null,
    2,
  ) + "\n",
);
