//! Consent follows the selected incumbents and task incarnation across calls.
use super::*;

#[test]
fn discard_approval_requires_a_token_on_both_tools() {
	let workspace = Workspace::new(&["api", "web"]);
	let state = task_documents(&workspace.state());
	let mut client = Client::launch(&workspace.root);

	for (tool, request) in [
		("remove", json!({"task": TASK})),
		("amend", json!({"task": TASK, "remove": ["api"]})),
	] {
		for key in ["approve_discard", "force", "yes"] {
			let mut args = request.clone();
			args[key] = json!(true);
			let invalid = client.call(tool, args);
			assert_eq!(invalid["isError"], true, "{invalid}");
			assert!(invalid["structuredContent"].is_null(), "{invalid}");
		}
	}
	assert_eq!(task_documents(&workspace.state()), state);
	assert!(workspace.tree("api/.git").is_file());
	assert!(workspace.tree("web/.git").is_file());
	client.close();
}

#[test]
fn approval_is_scoped_to_the_workspace_action_and_selected_participants() {
	let first = Workspace::new(&["api", "web", "infra"]);
	let second = Workspace::new(&["api", "web", "infra"]);
	for workspace in [&first, &second] {
		fs::write(workspace.tree("api/draft"), "same bytes").unwrap();
	}
	let mut client = Client::launch(&first.root);
	let selection = json!({"task":TASK, "remove":["api"]});
	let initial = client.call("amend", selection.clone());
	let token = decision(&initial);
	let first_state = task_documents(&first.state());
	let second_state = task_documents(&second.state());

	for (tool, args) in [
		("remove", json!({"task":TASK, "approve_discard":token})),
		(
			"amend",
			json!({"task":TASK, "remove":["api","web"], "approve_discard":token}),
		),
		(
			"amend",
			json!({"workspace":second.root, "task":TASK, "remove":["api"], "approve_discard":token}),
		),
	] {
		let stale = client.call(tool, args);
		assert_ne!(decision(&stale), token);
		assert_eq!(task_documents(&first.state()), first_state);
		assert_eq!(task_documents(&second.state()), second_state);
	}

	fs::remove_file(first.tree("api/draft")).unwrap();
	let mut approved = selection.clone();
	approved["approve_discard"] = json!(token);
	let stale = client.call("amend", approved);
	assert_eq!(
		stale["structuredContent"]["problem"]["code"], "decision_required",
		"{stale}"
	);
	assert_eq!(task_documents(&first.state()), first_state);
	assert!(first.tree("api/.git").is_file());

	let clean = client.call("amend", selection);
	assert_ne!(clean["isError"], true, "{clean}");
	assert!(!first.tree("api").exists());
	assert!(first.tree("web/.git").is_file());
	client.close();
}

#[test]
fn a_recreated_task_cannot_use_its_predecessors_approval() {
	let workspace = Workspace::new(&["api"]);
	let draft = workspace.tree("api/draft");
	fs::write(&draft, "same content").unwrap();
	let mut client = Client::launch(&workspace.root);
	let initial = client.call("remove", json!({"task":TASK}));
	let token = decision(&initial);
	let approved = json!({"task":TASK, "approve_discard":token});
	let removed = client.call("remove", approved.clone());
	assert_ne!(removed["isError"], true, "{removed}");

	let create = client.call(
		"create",
		json!({
			"task": TASK,
			"checkout": true,
			"repositories": [{"path": workspace.source("api")}],
		}),
	);
	assert_ne!(create["isError"], true, "{create}");
	fs::write(&draft, "same content").unwrap();
	let stale = client.call("remove", approved);
	assert_ne!(decision(&stale), token);
	assert_eq!(fs::read_to_string(draft).unwrap(), "same content");
	client.close();
}

#[test]
fn same_name_replacement_discloses_the_incumbent_before_creating_anything() {
	let workspace = Workspace::new(&["api"]);
	let replacement = support::repo(workspace.fixture.path(), "replacement");
	let draft = workspace.tree("api/draft");
	fs::write(&draft, "incumbent edit").unwrap();

	let before = task_documents(&workspace.state());
	let registrations = support::git(&replacement, &["worktree", "list", "--porcelain"]);
	let mut client = Client::launch(&workspace.root);
	let mut args = json!({
		"task": TASK,
		"remove": ["api"],
		"repositories": [{"path": replacement, "name": "api"}],
		"from": "main",
	});
	let token = "review-replacement";
	let id = client.start_observed("amend", args.clone(), token);
	client.progress(token, 1, "Inspected worktree for api");
	let initial = client.reply(id)["result"].take();
	assert_eq!(
		initial["structuredContent"]["disclosures"][0]["dirty_paths"],
		json!(["draft"])
	);
	args["approve_discard"] = json!(decision(&initial));
	assert_eq!(fs::read_to_string(&draft).unwrap(), "incumbent edit");
	assert_eq!(
		registrations,
		support::git(&replacement, &["worktree", "list", "--porcelain"])
	);
	assert_eq!(task_documents(&workspace.state()), before);

	// The same name can select a different incoming backing repository.
	let other = support::repo(workspace.fixture.path(), "other");
	let mut changed = args.clone();
	changed["repositories"][0]["path"] = json!(other);
	let different = client.call("amend", changed);
	assert_ne!(decision(&different), decision(&initial));
	assert_eq!(task_documents(&workspace.state()), before);
	assert_eq!(fs::read_to_string(&draft).unwrap(), "incumbent edit");

	let token = "apply-replacement";
	let id = client.start_observed("amend", args.clone(), token);
	client.progress(token, 1, "Inspected worktree for api");
	client.progress(token, 2, "Creating worktree for api");
	client.progress(token, 3, "Added participant api");
	client.progress(token, 4, "Unlinked original worktree for api");
	let replaced = client.reply(id)["result"].take();
	assert_ne!(replaced["isError"], true, "{replaced}");
	let common_dir = ["rev-parse", "--path-format=absolute", "--git-common-dir"];
	assert_eq!(
		support::git(&workspace.tree("api"), &common_dir),
		support::git(&replacement, &common_dir)
	);

	fs::write(&draft, "replacement edit").unwrap();
	let settled = task_documents(&workspace.state());
	let replay = client.call("amend", args.clone());
	assert_eq!(
		replay["structuredContent"]["problem"]["code"], "decision_required",
		"{replay}"
	);
	assert_eq!(
		replay["structuredContent"]["decision"]["kind"],
		"remove_participants"
	);
	assert!(replay["structuredContent"]["decision"]["token"].is_null());
	args.as_object_mut().unwrap().remove("approve_discard");
	let clean_retry = client.call("amend", args);
	assert_ne!(clean_retry["isError"], true, "{clean_retry}");
	assert_eq!(task_documents(&workspace.state()), settled);
	assert_eq!(fs::read_to_string(draft).unwrap(), "replacement edit");
	client.close();
}

#[test]
fn an_approval_still_honors_an_active_git_writer() {
	let workspace = Workspace::new(&["api"]);
	let api = workspace.tree("api");
	let draft = api.join("draft");
	fs::write(&draft, "keep until the writer settles").unwrap();
	let mut client = Client::launch(&workspace.root);
	let reviewed = client.call("remove", json!({"task":TASK}));
	let token = decision(&reviewed);
	let before = task_documents(&workspace.state());

	let admin = support::git(&api, &["rev-parse", "--absolute-git-dir"]);
	let lock = Path::new(&admin).join("index.lock");
	fs::write(&lock, "").unwrap();
	let refused = client.call("remove", json!({"task":TASK, "approve_discard":token}));
	assert_eq!(refused["isError"], true, "{refused}");
	assert!(refused["structuredContent"]["decision"]["token"].is_null());
	assert!(lock.is_file());
	assert_eq!(
		fs::read_to_string(draft).unwrap(),
		"keep until the writer settles"
	);
	assert_eq!(task_documents(&workspace.state()), before);
	client.close();
}
