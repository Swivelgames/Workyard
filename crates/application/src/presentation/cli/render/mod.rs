//! Shared supplied-fact renderers; views choose content and completion policy.
mod amendment;
mod disclosure;
mod operation;
mod participants;
mod paths;
mod progress;
mod style;
mod table;
pub(crate) mod task;
mod terminal;
mod workspace;

pub(crate) use style::{Style, Tone, text};
pub(crate) use table::{LocatedParticipantRow, ParticipantRow};
pub(crate) use terminal::{MessageScope, Terminal};
pub(crate) use workspace::{guidance as workspace_guidance, separate};
