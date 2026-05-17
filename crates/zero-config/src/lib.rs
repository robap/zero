//! `zero.toml` parsing/validation and rendering (config + toml_writer combined).

pub mod config;
pub mod toml_writer;

pub use config::{BuildConfig, Config, DevConfig, ProjectConfig};
pub use toml_writer::{TomlInput, render_toml};
