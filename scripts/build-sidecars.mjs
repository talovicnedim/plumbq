#!/usr/bin/env node
// Builds the binaries bundled next to the desktop app (Tauri sidecars) and copies them to
// apps/desktop/src-tauri/binaries/<name>-<target-triple>[.exe], where `tauri build` expects them.
//
//   node scripts/build-sidecars.mjs [--target <triple>]
//
// The target comes from --target, then TAURI_ENV_TARGET_TRIPLE (set by `tauri build`), then the
// host. `universal-apple-darwin` builds both Mac architectures and merges them with `lipo`. It
// also keeps a copy per architecture: `tauri build` compiles the app once per architecture, and
// each of those builds looks for `<name>-<that architecture>`.
// Which sidecars to build is read from tauri.release.conf.json, so the two never disagree.
// With SKIP_SIDECAR_BUILD=1 (set by the release workflow, which builds them in its own step) it
// only checks that they exist.

import { execFileSync } from 'node:child_process';
import { copyFileSync, chmodSync, existsSync, mkdirSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const tauriDir = join(root, 'apps/desktop/src-tauri');
const outDir = join(tauriDir, 'binaries');

/** Sidecar name in the bundle → cargo package that builds it. */
const PACKAGES = {
	'plumbq-mcp': 'plumbq-mcp',
	'plumbq-cli': 'plumbq-cli'
};

function run(command, args) {
	console.error(`> ${command} ${args.join(' ')}`);
	try {
		execFileSync(command, args, { cwd: root, stdio: ['ignore', 'inherit', 'inherit'] });
	} catch {
		// The command already printed its own error.
		console.error(`build-sidecars: \`${command}\` failed`);
		process.exit(1);
	}
}

function output(command, args) {
	return execFileSync(command, args, { cwd: root, encoding: 'utf8' });
}

function targetTriple() {
	const flag = process.argv.indexOf('--target');
	if (flag !== -1 && process.argv[flag + 1]) return process.argv[flag + 1];
	if (process.env.TAURI_ENV_TARGET_TRIPLE) return process.env.TAURI_ENV_TARGET_TRIPLE;
	const host = output('rustc', ['-vV']).match(/^host: (\S+)$/m);
	if (!host) throw new Error('could not read the host target from `rustc -vV`');
	return host[1];
}

/** The bin target name of each cargo package, e.g. plumbq-cli → plumbq. */
function binNames() {
	const metadata = JSON.parse(output('cargo', ['metadata', '--no-deps', '--format-version', '1']));
	const names = {};
	for (const pkg of metadata.packages) {
		const bin = pkg.targets.find((t) => t.kind.includes('bin'));
		if (bin) names[pkg.name] = bin.name;
	}
	return names;
}

function sidecars() {
	const config = JSON.parse(readFileSync(join(tauriDir, 'tauri.release.conf.json'), 'utf8'));
	return config.bundle.externalBin.map((path) => path.replace(/^binaries\//, ''));
}

function build(packages, target) {
	run('cargo', ['build', '--release', '--target', target, ...packages.flatMap((p) => ['-p', p])]);
}

const target = targetTriple();
const windows = target.includes('windows');
const exe = windows ? '.exe' : '';
const bins = binNames();
const wanted = sidecars();
const packages = wanted.map((name) => {
	const pkg = PACKAGES[name];
	if (!pkg || !bins[pkg]) {
		throw new Error(`sidecar ${name}: no cargo package ${pkg ?? '(unknown)'} with a binary`);
	}
	return pkg;
});

if (process.env.SKIP_SIDECAR_BUILD) {
	const missing = wanted
		.map((name) => join(outDir, `${name}-${target}${exe}`))
		.filter((path) => !existsSync(path));
	if (missing.length > 0) {
		console.error(
			`build-sidecars: SKIP_SIDECAR_BUILD is set but missing:\n  ${missing.join('\n  ')}`
		);
		process.exit(1);
	}
	console.error('build-sidecars: already built, skipping');
	process.exit(0);
}

mkdirSync(outDir, { recursive: true });
const universal = target === 'universal-apple-darwin';
const archs = universal ? ['aarch64-apple-darwin', 'x86_64-apple-darwin'] : [target];
for (const arch of archs) build(packages, arch);

for (const [index, name] of wanted.entries()) {
	const bin = bins[packages[index]];
	const built = (arch) => join(root, 'target', arch, 'release', `${bin}${exe}`);
	const dests = [];
	for (const arch of archs) {
		const dest = join(outDir, `${name}-${arch}${exe}`);
		copyFileSync(built(arch), dest);
		dests.push(dest);
	}
	if (universal) {
		const dest = join(outDir, `${name}-${target}`);
		run('lipo', ['-create', '-output', dest, ...archs.map(built)]);
		dests.push(dest);
	}
	for (const dest of dests) {
		if (!windows) chmodSync(dest, 0o755);
		console.error(`sidecar ready: ${dest}`);
	}
}
