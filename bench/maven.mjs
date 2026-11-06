// The Maven the benchmark uses (the Java task): `MVN`, else `mvn` on the PATH, else the copy the
// Maven wrapper downloaded into ~/.m2 (the project's own mvnw may be broken).
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

export function mavenBin() {
  if (process.env.MVN) return process.env.MVN;
  const onPath = spawnSync('sh', ['-c', 'command -v mvn'], { encoding: 'utf8' }).stdout.trim();
  if (onPath) return onPath;
  const dists = path.join(os.homedir(), '.m2', 'wrapper', 'dists');
  if (!fs.existsSync(dists)) return 'mvn';
  for (const dist of fs.readdirSync(dists).sort().reverse()) {
    const root = path.join(dists, dist);
    for (const hash of fs.readdirSync(root)) {
      const bin = path.join(root, hash, 'bin', 'mvn');
      if (fs.existsSync(bin)) return bin;
    }
  }
  return 'mvn';
}
