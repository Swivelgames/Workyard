#[cfg(windows)]
use std::fs::OpenOptions;
use std::{fs::File, path::Path};
use workyard_core::Problem;

#[cfg(windows)]
pub fn file_identity(file: &File) -> Result<(u64, u64), Problem> {
	use std::os::windows::io::AsRawHandle;
	use windows_sys::Win32::Storage::FileSystem::{
		BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
	};

	let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };

	// SAFETY: File owns the valid handle and info is a correctly sized output.
	if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
		return Err(Problem::io(
			"Inspect lock identity",
			std::io::Error::last_os_error(),
		));
	}

	Ok((
		info.dwVolumeSerialNumber as u64,
		((info.nFileIndexHigh as u64) << 32) | info.nFileIndexLow as u64,
	))
}

#[cfg(unix)]
pub fn file_identity(file: &File) -> Result<(u64, u64), Problem> {
	use std::os::unix::fs::MetadataExt;

	let metadata = file
		.metadata()
		.map_err(|e| Problem::io("Inspect file identity", e))?;

	Ok((metadata.dev(), metadata.ino()))
}

pub fn directory_identity(path: &Path) -> Result<String, Problem> {
	#[cfg(unix)]
	let directory = File::open(path).map_err(|e| Problem::io("Open repository identity", e))?;

	#[cfg(windows)]
	let directory = {
		use std::os::windows::fs::OpenOptionsExt;
		OpenOptions::new()
			.read(true)
			.custom_flags(windows_sys::Win32::Storage::FileSystem::FILE_FLAG_BACKUP_SEMANTICS)
			.open(path)
			.map_err(|e| Problem::io("Open repository identity", e))?
	};

	let (volume, file) = file_identity(&directory)?;

	Ok(format!("{volume:x}:{file:x}"))
}
