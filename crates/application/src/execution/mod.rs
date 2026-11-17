//! Adapters between core execution and Trine invocation.
mod confirmation;
mod events;
pub(crate) mod removal;
mod worker;

pub(crate) use events::Events;
pub(crate) use worker::blocking_action;
