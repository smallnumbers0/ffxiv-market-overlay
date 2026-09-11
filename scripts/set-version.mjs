#!/usr/bin/env node
/**
 * Set the app version in every file that records it.
 *
 * The version lives in four places, and the release workflow refuses to build
 * when they disagree - a guard that only fires *after* a tag is pushed, which
 * makes hand-editing them the wrong number of steps to get right. `npm version`
 * knows about exactly one of the four.
 *
 * Deliberately does not commit or tag: that stays an explicit act, so running
 * this never leaves git in a state you didn't ask for. It prints the commands.
 *
 *   npm run set-version 0.1.2
 */
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

/** Each file, how to read its version, and how to write a new one. */
const TARGETS = [
  {
    path: "package.json",
    read: (text) => JSON.parse(text).version,
    write: (text, version) =>
      // Textual rather than JSON.stringify so npm's own formatting, key order,
      // and trailing newline survive untouched.
      replaceOnce(text, /("version":\s*")[^"]+(")/, version),
  },
  {
    path: "src-tauri/tauri.conf.json",
    read: (text) => JSON.parse(text).version,
    write: (text, version) =>
      replaceOnce(text, /("version":\s*")[^"]+(")/, version),
  },
  {
    path: "src-tauri/Cargo.toml",
    // Anchored to the first bare `version =` line, which is the [package] one.
    // Dependency versions are all inside `{ ... }` tables or come later.
    read: (text) => text.match(/^version = "([^"]+)"$/m)?.[1],
    write: (text, version) =>
      replaceOnce(text, /^(version = ")[^"]+(")$/m, version),
  },
  {
    path: "src-tauri/Cargo.lock",
    // Cargo rewrites this itself on the next build, but leaving it stale means
    // the first `cargo` run after a bump produces a dirty tree - which is a
    // surprise mid-release.
    read: (text) =>
      text.match(/name = "ffxiv-market-overlay"\nversion = "([^"]+)"/)?.[1],
    write: (text, version) =>
      replaceOnce(
        text,
        /(name = "ffxiv-market-overlay"\nversion = ")[^"]+(")/,
        version,
      ),
  },
];

function replaceOnce(text, pattern, version) {
  const matches = text.match(new RegExp(pattern, pattern.flags + "g"));
  if (!matches || matches.length !== 1) {
    throw new Error(
      `expected exactly one match for ${pattern}, found ${matches?.length ?? 0}`,
    );
  }
  return text.replace(pattern, `$1${version}$2`);
}

const version = process.argv[2];
if (!version) {
  console.error("usage: npm run set-version <x.y.z>");
  process.exit(1);
}
// Tauri and Cargo both reject anything that isn't a plain semver triple, and a
// leading "v" is the mistake worth catching here rather than in a build log.
if (!/^\d+\.\d+\.\d+$/.test(version)) {
  console.error(`not a version: '${version}' (expected x.y.z, with no 'v')`);
  process.exit(1);
}

for (const target of TARGETS) {
  const file = join(root, target.path);
  const before = readFileSync(file, "utf8");
  const previous = target.read(before);
  if (previous === undefined) {
    console.error(`could not find a version in ${target.path}`);
    process.exit(1);
  }
  writeFileSync(file, target.write(before, version));
  console.log(`  ${target.path.padEnd(26)} ${previous} -> ${version}`);
}

// Read back from disk rather than trusting the writes: a silent mismatch here
// is exactly the failure this script exists to prevent.
const written = TARGETS.map((target) => ({
  path: target.path,
  found: target.read(readFileSync(join(root, target.path), "utf8")),
}));
const wrong = written.filter((entry) => entry.found !== version);
if (wrong.length > 0) {
  console.error("\nversions still disagree after writing:");
  for (const entry of wrong) console.error(`  ${entry.path}: ${entry.found}`);
  process.exit(1);
}

// `git tag -a`, not `git tag`: `--follow-tags` pushes annotated tags only, so
// a lightweight one leaves the commit pushed and the tag behind - and the
// release never builds, with nothing on either end saying why.
console.log(`
All four agree on ${version}. To cut the release:

  git commit -am "Release ${version}"
  git tag -a v${version} -m "Release ${version}"
  git push --follow-tags

That tag builds the Windows installer and attaches it to a draft release.`);
