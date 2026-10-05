//! Workflows: definitions, validation and the engine that decides what runs next.

pub mod chat_runner;
pub mod engine;
pub mod graph;
pub mod integration;
pub mod orchestrator;
pub mod repair;
pub mod runner;
pub mod service;
pub mod templates;
pub mod validation;
pub mod worktree_lock;

#[cfg(test)]
mod code_tests;
#[cfg(test)]
mod engine_tests;
#[cfg(test)]
mod orchestrator_tests;
#[cfg(test)]
mod routing_tests;
// Stand-in CLIs are shell scripts started in a Unix terminal.
#[cfg(all(test, unix))]
mod real_stack_tests;
#[cfg(test)]
pub mod test_support;
