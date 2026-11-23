//! Shared supplied-fact renderers; views choose content and completion policy.
mod amendment;
mod command;
mod disclosure;
mod operation;
mod participants;
mod paths;
mod problem;
mod progress;
mod recorded_problem;
mod style;
mod table;
mod tag;
pub(crate) mod task;
mod terminal;
mod workspace;

pub(crate) use style::{Style, Tone, text};
pub(crate) use table::{ParticipantRow, StatusParticipantRow};
pub(crate) use tag::TaskTag;
pub(crate) use terminal::{MessageScope, Terminal};
pub(crate) use workspace::{guidance as workspace_guidance, separate};
