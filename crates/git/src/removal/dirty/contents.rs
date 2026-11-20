//! Enumerate and fingerprint the contents reachable through this worktree's directory handles.
use super::frame;
use crate::removal::{blocked, protected};
use cap_primitives::{
	ambient_authority,
	fs::{self, FollowSymlinks},
};
use sha2::{Digest, Sha256};
use std::{
	collections::BTreeSet,
	ffi::OsString,
	fs::File,
	io::{self, Read},
	path::{Component, Path, PathBuf},
};
use workyard_core::{Control, Problem};

enum Location<'a> {
	Entry { parent: &'a File, name: &'a Path },
	Absent,
	Displaced,
}

pub(super) struct Worktree(File);

impl Worktree {
	pub fn open(path: &Path, expected: (u64, u64)) -> Result<Self, Problem> {
		let root = fs::open_ambient_dir(path, ambient_authority())
			.map_err(|e| Problem::io("Open worktree for disclosure", e))?;
		if workyard_system::file_identity(&root)? != expected {
			return Err(blocked("The worktree changed before disclosure."));
		}
		Ok(Self(root))
	}

	pub fn inspect(
		&self,
		path: &Path,
		disclosed: &mut BTreeSet<PathBuf>,
		hash: &mut Sha256,
		control: &Control,
	) -> Result<(), Problem> {
		self.locate(path, control, |location| match location {
			Location::Entry { parent, name } => {
				inspect_entry(parent, name, path, disclosed, hash, control)
			}
			Location::Absent | Location::Displaced => {
				missing(path, disclosed, hash);
				Ok(())
			}
		})
	}

	pub fn include_index_entry(
		&self,
		path: &Path,
		gitlink: bool,
		control: &Control,
	) -> Result<bool, Problem> {
		self.locate(path, control, |location| {
			let (parent, name) = match location {
				Location::Entry { parent, name } => (parent, name),
				Location::Displaced => return Ok(true),
				Location::Absent => return Ok(false),
			};
			let Some(metadata) = metadata(parent, name)? else {
				// An intentionally absent sparse checkout entry has no files to discard.
				return Ok(false);
			};
			if !gitlink || !metadata.is_dir() {
				return Ok(true);
			}

			// Git can report an uninitialized submodule as clean even when
			// someone has saved files inside its checkout directory.
			let dir = open_directory(parent, name, path)?;
			let first = directory_names(&dir)
				.and_then(|mut names| names.next().transpose())
				.map_err(|e| Problem::io("Inspect submodule directory", e))?;
			Ok(first.is_some())
		})
	}

	/// Resolve every parent without following links. An obstructing file or link
	/// displaces indexed descendants even when Git suppresses their status.
	fn locate<T>(
		&self,
		path: &Path,
		control: &Control,
		visit: impl FnOnce(Location<'_>) -> Result<T, Problem>,
	) -> Result<T, Problem> {
		let mut components = path.components().peekable();
		let mut parent = &self.0;
		let mut child;
		let mut prefix = PathBuf::new();
		while let Some(Component::Normal(part)) = components.next() {
			let name = Path::new(part);
			if components.peek().is_none() {
				return visit(Location::Entry { parent, name });
			}
			control.check()?;

			let Some(metadata) = metadata(parent, name)? else {
				return visit(Location::Absent);
			};
			if !metadata.is_dir() {
				return visit(Location::Displaced);
			}

			prefix.push(name);
			child = open_directory(parent, name, &prefix)?;
			parent = &child;
		}
		Err(blocked("Invalid relative disclosure path."))
	}
}

fn metadata(parent: &File, name: &Path) -> Result<Option<fs::Metadata>, Problem> {
	match fs::stat(parent, name, FollowSymlinks::No) {
		Ok(metadata) => Ok(Some(metadata)),
		Err(e)
			if matches!(
				e.kind(),
				io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
			) =>
		{
			Ok(None)
		}
		Err(e) => Err(Problem::io("Inspect disclosed file", e)),
	}
}

fn missing(relative: &Path, disclosed: &mut BTreeSet<PathBuf>, hash: &mut Sha256) {
	disclosed.insert(relative.to_owned());
	frame(hash, b"missing");
}

fn open_directory(parent: &File, name: &Path, relative: &Path) -> Result<File, Problem> {
	let dir = fs::open_dir_nofollow(parent, name)
		.map_err(|e| Problem::io("Open disclosed directory", e))?;
	protected::opened_directory(&dir, relative)?;
	Ok(dir)
}

fn inspect_entry(
	parent: &File,
	name: &Path,
	relative: &Path,
	disclosed: &mut BTreeSet<PathBuf>,
	hash: &mut Sha256,
	control: &Control,
) -> Result<(), Problem> {
	control.check()?;
	let Some(metadata) = metadata(parent, name)? else {
		missing(relative, disclosed, hash);
		return Ok(());
	};
	if metadata.is_dir() {
		let dir = open_directory(parent, name, relative)?;
		return inspect_directory(&dir, relative, disclosed, hash, control);
	}

	disclosed.insert(relative.to_owned());
	if metadata.file_type().is_symlink() {
		let target = fs::read_link_contents(parent, name)
			.map_err(|e| Problem::io("Inspect disclosed symlink", e))?;
		frame(hash, b"symlink");
		frame(hash, target.as_os_str().as_encoded_bytes());
		return Ok(());
	}
	if !metadata.is_file() {
		return Err(blocked(
			"Cannot safely inspect a special file before removal.",
		));
	}
	hash_file(parent, name, hash, control)
}

fn inspect_directory(
	dir: &File,
	relative: &Path,
	disclosed: &mut BTreeSet<PathBuf>,
	hash: &mut Sha256,
	control: &Control,
) -> Result<(), Problem> {
	frame(hash, b"directory");
	let mut names = directory_names(dir)
		.and_then(Iterator::collect::<io::Result<Vec<_>>>)
		.map_err(|e| Problem::io("Read disclosed directory", e))?;
	names.sort();
	if names.is_empty() {
		disclosed.insert(relative.to_owned());
	}
	for name in names {
		frame(hash, name.as_encoded_bytes());
		let child = relative.join(&name);
		let mut entry = Sha256::new();
		inspect_entry(
			dir,
			Path::new(&name),
			&child,
			disclosed,
			&mut entry,
			control,
		)?;
		frame(hash, &entry.finalize());
	}
	Ok(())
}

fn directory_names(dir: &File) -> io::Result<impl Iterator<Item = io::Result<OsString>>> {
	#[cfg(unix)]
	let entries = fs::read_base_dir(dir)?;

	// Windows directory handles exclude delete sharing. Retain the verbatim path,
	// including its UNC prefix, when enumerating the pinned directory.
	#[cfg(windows)]
	let entries = std::fs::read_dir(winx::file::get_file_path(dir)?)?;

	Ok(entries.map(|entry| entry.map(|e| e.file_name())))
}

fn hash_file(
	parent: &File,
	name: &Path,
	hash: &mut Sha256,
	control: &Control,
) -> Result<(), Problem> {
	let mut options = fs::OpenOptions::new();
	options.read(true);
	#[cfg(unix)]
	{
		use fs::OpenOptionsExt;
		options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
	}
	let mut file =
		fs::open(parent, name, &options).map_err(|e| Problem::io("Read disclosed file", e))?;
	let metadata = file
		.metadata()
		.map_err(|e| Problem::io("Inspect disclosed file", e))?;
	if !metadata.is_file() {
		return Err(blocked("The disclosed file changed during inspection."));
	}
	frame(hash, b"file");
	frame(hash, &metadata.len().to_le_bytes());
	#[cfg(unix)]
	{
		use std::os::unix::fs::PermissionsExt;
		frame(hash, &(metadata.permissions().mode() & 0o111).to_le_bytes());
	}
	let mut buffer = [0; 8192];
	loop {
		control.check()?;
		let size = file
			.read(&mut buffer)
			.map_err(|e| Problem::io("Read disclosed file", e))?;
		if size == 0 {
			return Ok(());
		}
		hash.update(&buffer[..size]);
	}
}

#[cfg(all(test, unix))]
mod tests {
	use super::*;

	#[test]
	fn enumerating_an_open_directory_after_a_symlink_swap_stays_in_the_original_directory() {
		let fixture = tempfile::tempdir().unwrap();
		let worktree = fixture.path().join("worktree");
		let cache = worktree.join("cache");
		let external = fixture.path().join("external");
		std::fs::create_dir_all(&cache).unwrap();
		std::fs::create_dir(&external).unwrap();
		std::fs::write(cache.join("draft"), "inside the worktree").unwrap();
		std::fs::write(external.join("private"), "outside the worktree").unwrap();

		let root = fs::open_ambient_dir(&worktree, ambient_authority()).unwrap();
		let relative = Path::new("cache");
		let opened = open_directory(&root, relative, relative).unwrap();
		std::fs::rename(&cache, fixture.path().join("previous-cache")).unwrap();
		std::os::unix::fs::symlink(external, cache).unwrap();

		let mut disclosed = BTreeSet::new();
		inspect_directory(
			&opened,
			relative,
			&mut disclosed,
			&mut Sha256::new(),
			&Control::default(),
		)
		.unwrap();
		assert_eq!(disclosed, BTreeSet::from([relative.join("draft")]));
	}
}
