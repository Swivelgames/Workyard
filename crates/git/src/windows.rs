//! Launch suspended, contain before execution, then observe the whole Job.
use std::{
	io,
	mem::{size_of, zeroed},
	os::windows::{io::AsRawHandle, process::CommandExt},
	process::{Child, Command},
};
use windows_sys::Win32::{
	Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE},
	System::{
		Console::{CTRL_BREAK_EVENT, GenerateConsoleCtrlEvent, GetConsoleProcessList},
		Diagnostics::ToolHelp::{
			CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
		},
		JobObjects::{
			AssignProcessToJobObject, CreateJobObjectW, JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
			JobObjectBasicAccountingInformation, QueryInformationJobObject, TerminateJobObject,
		},
		Threading::{
			CREATE_NEW_PROCESS_GROUP, CREATE_SUSPENDED, OpenThread, ResumeThread,
			THREAD_SUSPEND_RESUME,
		},
	},
};
use workyard_core::Problem;

struct Handle(HANDLE);
impl Drop for Handle {
	fn drop(&mut self) {
		// SAFETY: this object uniquely owns a valid handle.
		unsafe {
			CloseHandle(self.0);
		}
	}
}

pub struct Managed {
	pub child: Child,
	job: Handle,
}

impl Managed {
	pub fn spawn(mut command: Command) -> Result<Self, Problem> {
		// SAFETY: null name/security select an unnamed, non-inheritable job.
		let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
		if job.is_null() {
			return Err(last("Create Git job"));
		}

		let job = Handle(job);
		command.creation_flags(CREATE_SUSPENDED | CREATE_NEW_PROCESS_GROUP);
		let mut child = command
			.spawn()
			.map_err(|e| Problem::io("Launch suspended Git", e))?;

		// SAFETY: both handles remain owned and valid during assignment.
		if unsafe { AssignProcessToJobObject(job.0, child.as_raw_handle()) } == 0 {
			let error = last("Contain Git before execution");
			let _ = child.kill();
			let _ = child.wait();
			return Err(error);
		}

		if let Err(error) = resume_initial_thread(child.id()) {
			// SAFETY: the job is ours; the suspended child has not executed.
			unsafe {
				TerminateJobObject(job.0, 1);
			}

			let _ = child.wait();
			return Err(error);
		}

		Ok(Self { child, job })
	}

	pub fn interrupt(&mut self) -> Result<(), Problem> {
		let mut processes = [0u32; 1];
		// SAFETY: the array is valid for the supplied count. A positive result
		// establishes a console shared by this launcher and its normal child.
		let has_console = unsafe { GetConsoleProcessList(processes.as_mut_ptr(), 1) } > 0;
		if has_console {
			// SAFETY: only the child group created above is targeted.
			if unsafe { GenerateConsoleCtrlEvent(CTRL_BREAK_EVENT, self.child.id()) } != 0 {
				return Ok(());
			}
			return Err(last("Send CTRL_BREAK to Git"));
		}
		// Headless stdio cannot receive a console event. This is the explicit
		// containment fallback, not a timeout-based escalation.
		if unsafe { TerminateJobObject(self.job.0, 1) } == 0 {
			return Err(last("Terminate headless Git job"));
		}

		Ok(())
	}

	pub fn alive(&self) -> Result<bool, Problem> {
		// SAFETY: the output buffer has the correct layout/size for this class.
		let mut accounting: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION = unsafe { zeroed() };
		let result = unsafe {
			QueryInformationJobObject(
				self.job.0,
				JobObjectBasicAccountingInformation,
				(&mut accounting as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
				size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
				std::ptr::null_mut(),
			)
		};
		if result == 0 {
			return Err(last("Observe Git job completion"));
		}

		Ok(accounting.ActiveProcesses != 0)
	}
}

fn resume_initial_thread(pid: u32) -> Result<(), Problem> {
	// SAFETY: snapshot owns its handle and THREADENTRY32 advertises its size.
	let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
	if snapshot == INVALID_HANDLE_VALUE {
		return Err(last("Find suspended Git thread"));
	}

	let snapshot = Handle(snapshot);
	let mut entry: THREADENTRY32 = unsafe { zeroed() };
	entry.dwSize = size_of::<THREADENTRY32>() as u32;
	let mut found = unsafe { Thread32First(snapshot.0, &mut entry) } != 0;
	while found {
		if entry.th32OwnerProcessID == pid {
			let thread = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID) };
			if thread.is_null() {
				return Err(last("Open suspended Git thread"));
			}

			let thread = Handle(thread);
			if unsafe { ResumeThread(thread.0) } == u32::MAX {
				return Err(last("Resume contained Git"));
			}
			return Ok(());
		}
		found = unsafe { Thread32Next(snapshot.0, &mut entry) } != 0;
	}
	Err(Problem::new(
		workyard_core::ProblemCode::LaunchFailed,
		"Suspended Git initial thread was not found.",
	))
}

fn last(context: &str) -> Problem {
	Problem::io(context, io::Error::last_os_error())
}
