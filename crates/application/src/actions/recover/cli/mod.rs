mod bindings;
mod view;
mod workspace;

pub(super) use bindings::bindings;
pub(super) use view::RecoverView;

const RESTORE_WORKTREE: &str = "Restore the worktree in place, or repair its Git link if it moved.";
