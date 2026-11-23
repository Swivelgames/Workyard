pub(super) use crate::guidance::text::path as display;
use std::path::{Component, Path, PathBuf};

pub(crate) struct Paths {
	cwd: Option<PathBuf>,
	home: Option<PathBuf>,
}

impl Paths {
	pub fn discover() -> Self {
		#[cfg(windows)]
		let home = std::env::var_os("USERPROFILE");
		#[cfg(not(windows))]
		let home = std::env::var_os("HOME");
		Self {
			cwd: std::env::current_dir().ok(),
			home: home.map(PathBuf::from),
		}
	}

	pub fn root(&self, path: &Path) -> String {
		let absolute = display(path);
		let mut choices = vec![absolute];
		if let Some(cwd) = &self.cwd {
			let from_cwd = relative(cwd, path);
			#[cfg(windows)]
			let from_cwd = from_cwd.or_else(|| {
				let canonical = cwd.canonicalize().ok()?;
				relative(&canonical, path)
			});
			if let Some(relative) = from_cwd {
				choices.push(display(&relative));
			}
		}
		if let Some(home) = &self.home
			&& let Ok(suffix) = path.strip_prefix(home)
		{
			choices.push(if suffix.as_os_str().is_empty() {
				"~".into()
			} else {
				format!("~/{}", display(suffix))
			});
		}
		choices
			.into_iter()
			.min_by_key(String::len)
			.expect("absolute fallback")
	}

	pub fn within(&self, root: &Path, path: &Path) -> String {
		relative(root, path).map_or_else(|| display(path), |path| display(&path))
	}
}

fn relative(base: &Path, path: &Path) -> Option<PathBuf> {
	let base: Vec<_> = base.components().collect();
	let target: Vec<_> = path.components().collect();
	if base.first() != target.first() {
		return None;
	}
	let common = base
		.iter()
		.zip(&target)
		.take_while(|(left, right)| left == right)
		.count();
	let mut result = PathBuf::new();
	for component in &base[common..] {
		if !matches!(component, Component::Normal(_)) {
			return None;
		}
		result.push("..");
	}
	for component in &target[common..] {
		result.push(component.as_os_str());
	}
	if result.as_os_str().is_empty() {
		result.push(".");
	}
	Some(result)
}

#[cfg(test)]
mod tests {
	#[test]
	fn relative_paths_have_an_explicit_root_basis() {
		let root = std::env::current_dir().unwrap();
		let paths = super::Paths {
			cwd: Some(root.clone()),
			home: None,
		};
		assert_eq!(paths.root(&root), ".");
		assert_eq!(paths.within(&root, &root.join(".workyard")), ".workyard");
		assert_eq!(paths.within(&root, &root.join("a\nb")), "a\\nb");
	}
}

#[cfg(test)]
mod context_tests {
	use super::Paths;

	#[test]
	fn child_cwd_external_state_and_home_context_stay_concise() {
		let base = std::env::current_dir().unwrap();
		let root = base.join("workspace");
		let child = Paths {
			cwd: Some(root.join("child")),
			home: None,
		};
		assert_eq!(child.root(&root), "..");
		assert_eq!(
			child.within(&root, &base.join("state")),
			std::path::Path::new("..").join("state").to_str().unwrap()
		);
		let home = Paths {
			cwd: None,
			home: Some(base),
		};
		assert_eq!(home.root(&root), "~/workspace");
	}

	#[cfg(unix)]
	#[test]
	fn non_unicode_paths_use_unambiguous_escaped_native_display() {
		use std::os::unix::ffi::OsStringExt;
		let path = std::path::PathBuf::from(std::ffi::OsString::from_vec(b"repo-\xff\n".to_vec()));
		let shown = super::display(&path);
		assert!(!shown.contains('\n'));
		assert!(shown.contains("\\xFF"));
		assert!(shown.contains("\\n"));
	}

	#[cfg(windows)]
	#[test]
	fn different_drives_keep_absolute_location() {
		let base = std::path::Path::new("C:\\workspace");
		let path = std::path::Path::new("D:\\state");
		let paths = Paths {
			cwd: Some(base.into()),
			home: None,
		};
		assert_eq!(paths.within(base, path), "D:\\state");
	}
}
