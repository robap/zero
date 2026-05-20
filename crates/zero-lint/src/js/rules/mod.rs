//! JS/TS lint rules. Each rule module defines a single `check` function
//! that takes a `FileCtx` and returns a `Vec<Diagnostic>`. Wired through
//! `js::lint_js_file`.

pub mod c01_no_class_component;
pub mod c02_custom_elements;
pub mod i01_unknown_specifier;
pub mod i02_dot_zero_import;
pub mod r01_template_val_read;
pub mod r02_val_assignment;
pub mod r03_module_reactive;
pub mod s01_function_size;
pub mod t01_event_listener;
pub mod t02_event_modifier;
pub mod t03_each_no_key;
pub mod t04_direct_dom;
