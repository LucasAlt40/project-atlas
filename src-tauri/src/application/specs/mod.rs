//! Spec-Driven Development (ADR 0035): the use cases around a specification and the framework
//! that gives it a format.

pub mod cli;
pub mod cli_service;
pub mod edits;
pub mod framework;
pub mod implementation;
pub mod openspec;
pub mod ports;
pub mod progress;
pub mod prompts;
pub mod review;
pub mod runner;
pub mod service;
pub mod view;

#[cfg(test)]
mod real_cli_tests;
#[cfg(test)]
mod tests;
