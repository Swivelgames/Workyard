// From the Workyard checkout:
// cargo build --release --locked
// npm install --prefix target/mcp-interop --no-save --package-lock=false @modelcontextprotocol/sdk@1.32.2
// node tests/smoke_mcp.mjs
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import {
	existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, realpathSync, rmSync, writeFileSync,
} from "node:fs";
import { createRequire } from "node:module";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { CallbackTransport, ProgressTransport } from "./mcp_progress.mjs";

const requireSdk = createRequire(new URL("../target/mcp-interop/package.json", import.meta.url));
const { Client } = requireSdk("@modelcontextprotocol/sdk/client");
const target = fileURLToPath(new URL("../target/", import.meta.url));
const binary = join(target, "release", process.platform === "win32" ? "workyard.exe" : "workyard");

function git(cwd, ...args) {
	return execFileSync("git", args, { cwd, encoding: "utf8" });
}

async function createFixture(root) {
	const sources = join(root, "sources");
	mkdirSync(sources);
	for (const name of ["api", "web"]) {
		git(sources, "init", "--quiet", "-b", "main", name);
		git(join(sources, name), "-c", "user.name=MCP Fixture", "-c", "user.email=mcp@example.invalid",
			"commit", "--quiet", "--allow-empty", "-m", "Initial");
	}

	const workspace = join(root, "workspace");
	mkdirSync(workspace);
	await initializeTasks(workspace, sources);

	const draft = join(workspace, "tasks", "feat", "inventory", "api", "draft");
	writeFileSync(draft, "In progress\n");
	return { repo: join(sources, "api"), sources, workspace, draft };
}

async function callForResult(client, name, args, options) {
	const result = await client.callTool({ name, arguments: args }, undefined, options);
	assert.ok(!result.isError, `${name}: ${JSON.stringify(result.content)}`);
	assert.equal(result.content.length, 1);
	const [content] = result.content;
	assert.equal(content.type, "text");
	assert.deepEqual(JSON.parse(content.text), result.structuredContent);
	return result.structuredContent;
}

async function callWithProgress(client, name, args, expectedMessages) {
	client.transport.expectProgress(expectedMessages);
	// Supplying a callback asks the SDK to attach its progress token.
	return callForResult(client, name, args, { onprogress() {} });
}

async function initializeTasks(workspace, sources) {
	const client = new Client({ name: "workyard-sdk-setup", version: "1" });
	try {
		await client.connect(new CallbackTransport({ command: binary, args: ["mcp"], cwd: workspace }));
		const initialized = await callForResult(client, "init", {});
		assert.equal(realpathSync.native(initialized.workspace), realpathSync.native(workspace));
		assert.equal(realpathSync.native(initialized.tasks_root), realpathSync.native(join(workspace, "tasks")));

		const tracked = await callForResult(client, "projects.track", { paths: [sources] });
		assert.deepEqual(tracked.changed.map(path => realpathSync.native(path)), [realpathSync.native(sources)]);
		const catalog = await callForResult(client, "projects.list", {});
		const references = catalog.projects.map(project => project.reference);
		assert.deepEqual(references, ["api", "web"]);

		for (const task of ["feat/inventory", "fix/other"]) {
			const args = { task, projects: [references[0]], no_fetch: true, from: "main" };
			const updates = [];
			const created = await callForResult(client, "create", args, {
				onprogress: update => updates.push(update.message),
			});
			assert.equal(created.lifecycle, "active");
			assert.deepEqual(updates, [
				"Creating worktree for api", "Created worktree for api", "Finished worktree setup for api",
			]);
		}
	} finally {
		await client.close();
	}
}

function stateFiles(directory) {
	const files = {};
	for (const entry of readdirSync(directory, { withFileTypes: true })) {
		const path = join(directory, entry.name);
		files[entry.name] = entry.isDirectory() ? stateFiles(path) : readFileSync(path);
	}
	return files;
}

function snapshot({ repo, workspace, draft }) {
	return {
		state: stateFiles(join(workspace, ".workyard")),
		registrations: git(repo, "worktree", "list", "--porcelain"),
		draft: readFileSync(draft),
	};
}

function rpcResult(cwd, name, input) {
	const action = name.split(".");
	const request = { jsonrpc: "2.0", id: 1, method: "invoke", params: { action, input } };
	const output = execFileSync(binary, ["rpc"], {
		cwd,
		input: JSON.stringify(request) + "\n",
		encoding: "utf8",
	});
	return JSON.parse(output).result;
}

async function observe(client, cwd, name, args) {
	const outcome = await callForResult(client, name, args);
	assert.deepEqual(outcome, rpcResult(cwd, name, args), name);
	return outcome;
}

async function verifyProjects(client, cwd, sources, workspace) {
	const catalog = await observe(client, cwd, "projects.list", {});
	assert.deepEqual(catalog.projects.map(project => project.reference), ["api", "web"]);
	for (const project of catalog.projects) {
		assert.equal(realpathSync.native(project.path), realpathSync.native(join(sources, project.reference)));
	}

	const available = catalog.roots.find(root => root.status === "available");
	assert.equal(realpathSync.native(available.path), realpathSync.native(sources));
	const missing = catalog.roots.find(root => root.status === "missing");
	assert.equal(missing.path, join(workspace, "projects"));
	assert.equal(existsSync(missing.path), false);
}

async function verifyCreate(client, workspace) {
	const task = "feat/sdk-create";
	const args = { task, projects: ["api", "web"], from: "main", no_fetch: true };
	const messages = args.projects.flatMap(name => [
		`Creating worktree for ${name}`, `Created worktree for ${name}`, `Finished worktree setup for ${name}`,
	]);
	const created = await callWithProgress(client, "create", args, messages);

	assert.equal(created.operation.outcome, "succeeded");
	assert.equal(created.lifecycle, "active");
	assert.deepEqual(created.participants.map(p => p.name), args.projects);
	for (const name of args.projects) {
		const path = join(workspace, "tasks", task, name);
		assert.equal(git(path, "branch", "--show-current").trim(), task);
	}

	const duplicate = await client.callTool({ name: "create", arguments: args });
	assert.equal(duplicate.isError, true);
	assert.equal(duplicate.structuredContent.problem.code, "task_exists");
}

async function verifyNamedRecovery(client, fixture) {
	const before = snapshot(fixture);
	const args = { task: "feat/inventory", workspace: fixture.workspace };
	const outcome = await callWithProgress(client, "recover", args, []);
	assert.equal(outcome.nothing_to_recover, true);
	assert.equal(outcome.operation.kind, "recover");
	assert.deepEqual(snapshot(fixture), before);
}

async function verifyAmend(client, { workspace, sources }) {
	const task = "feat/inventory";
	const args = {
		task,
		projects: ["web"],
		repositories: [{ path: join(sources, "api"), name: "backend" }],
		from: "main",
		no_fetch: true,
	};
	const outcome = await callWithProgress(client, "amend", args, [
		"Renamed participant to backend", "Creating worktree for web", "Added participant web",
	]);
	assert.equal(outcome.operation.outcome, "succeeded");
	assert.deepEqual(outcome.changes, [
		{ participant: "backend", kind: "renamed" },
		{ participant: "web", kind: "added" },
	]);

	const taskDirectory = join(workspace, "tasks", task);
	assert.equal(readFileSync(join(taskDirectory, "backend", "draft"), "utf8"), "In progress\n");
	assert.equal(git(join(taskDirectory, "web"), "branch", "--show-current").trim(), task);
	const recovered = await callWithProgress(client, "recover", { task }, [
		"Checked participant backend (unchanged)", "Checked participant web (unchanged)",
	]);
	assert.equal(recovered.nothing_to_recover, true);
	assert.equal(recovered.operation.continues, "amend");
}

async function verifyRemoval(client, workspace) {
	const task = "feat/sdk-create";
	const args = { task, remove: ["web"] };
	const draft = join(workspace, "tasks", task, "web", "draft");
	writeFileSync(draft, "waiting for review");
	const decision = await client.callTool({ name: "amend", arguments: args });
	assert.equal(decision.isError, true);
	const { problem, disclosures, decision: required } = decision.structuredContent;
	assert.equal(problem.code, "decision_required", problem.message);
	assert.deepEqual(disclosures.find(d => d.participant === "web").dirty_paths, ["draft"]);
	assert.equal(typeof required.token, "string");
	assert.equal(readFileSync(draft, "utf8"), "waiting for review");

	const reconnected = new Client({ name: "workyard-sdk-reconnected", version: "1" });
	const transport = new ProgressTransport({ command: binary, args: ["mcp"], cwd: workspace });
	try {
		await reconnected.connect(transport);
		const approved = { ...args, approve_discard: required.token };
		const amended = await callForResult(reconnected, "amend", approved);
		assert.deepEqual(amended.changes, [{ participant: "web", kind: "removed" }]);
		assert.ok(!existsSync(join(workspace, "tasks", task, "web")));

		const retired = await callWithProgress(reconnected, "remove", { task }, [
			"Inspected worktree for api", "Unlinked worktree for api",
		]);
		assert.equal(retired.lifecycle, "retired");

		const inventory = await callForResult(reconnected, "list", {});
		assert.ok(inventory.tasks.some(item => item.name === task && item.lifecycle === "retired"));

		const repeated = await callWithProgress(reconnected, "remove", { task }, []);
		assert.equal(repeated.lifecycle, "retired");
	} finally {
		await reconnected.close();
	}
	transport.verify();
}

async function verifyUntrack(client, { workspace, repo, sources }) {
	const task = "feat/inventory";
	const args = { task };
	const status = await callForResult(client, "status", args);
	const registrations = git(repo, "worktree", "list", "--porcelain");
	const removed = await callForResult(client, "projects.untrack", { paths: [sources] });
	assert.deepEqual(removed.changed.map(path => realpathSync.native(path)), [realpathSync.native(sources)]);

	const catalog = await callForResult(client, "projects.list", {});
	assert.deepEqual(catalog.projects, []);
	assert.deepEqual(await callForResult(client, "status", args), status);
	assert.equal(git(repo, "worktree", "list", "--porcelain"), registrations);
	assert.equal(readFileSync(join(workspace, "tasks", task, "backend", "draft"), "utf8"), "In progress\n");
}

async function verifyTools(fixture) {
	const { workspace } = fixture;
	const cwd = join(workspace, "tasks");
	const client = new Client({ name: "workyard-sdk-smoke", version: "1" });
	const transport = new ProgressTransport({ command: binary, args: ["mcp"], cwd });
	try {
		await client.connect(transport);
		assert.equal(client.getServerVersion().name, "workyard");
		assert.deepEqual(client.getServerCapabilities(), { tools: {} });

		const { tools } = await client.listTools();
		assert.deepEqual(tools.map(tool => tool.name).sort(), [
			"amend", "create", "info", "init", "list", "projects.list", "projects.track",
			"projects.untrack", "recover", "remove", "status",
		]);

		const before = snapshot(fixture);
		const invalid = await client.callTool({ name: "status", arguments: { task: 42 } });
		assert.equal(invalid.isError, true);

		const inventory = await observe(client, cwd, "list", { workspace });
		assert.deepEqual(inventory.tasks.map(task => task.name), ["feat/inventory", "fix/other"]);

		const [selected] = inventory.tasks;
		const status = await observe(client, cwd, "status", { task: selected.name });
		assert.deepEqual(status.tasks.map(task => task.name), [selected.name]);
		assert.equal(status.tasks[0].current[0].dirty, true);

		const info = await observe(client, cwd, "info", {});
		assert.equal(info.discovery, "ancestor");
		assert.equal(info.active_tasks, 2);
		assert.equal(info.tasks_root, inventory.tasks_root);

		await verifyProjects(client, cwd, fixture.sources, info.workspace);
		assert.deepEqual(snapshot(fixture), before);

		await verifyCreate(client, workspace);
		await verifyNamedRecovery(client, fixture);
		await verifyAmend(client, fixture);
		await verifyRemoval(client, workspace);
		await verifyUntrack(client, fixture);
	} finally {
		await client.close();
	}
	transport.verify();
}

const root = mkdtempSync(join(target, "sdk-workspace-"));
try {
	const fixture = await createFixture(root);
	await verifyTools(fixture);
	console.log("SDK setup, discovery, observations, RPC parity, task creation, amendment and named recovery with live progress, reconnect-safe removal and untracking passed.");
} finally {
	rmSync(root, { recursive: true, force: true });
}
