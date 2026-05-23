//! `zero build` — CommonJS-style bundler for the zero framework.

pub mod bundler;
pub mod css;
pub mod index_html;
pub mod manifest;
pub mod minify;
pub mod resolver;

#[cfg(test)]
mod tests {
    #[test]
    fn swc_minifier_module_is_available() {
        use swc_core::ecma::minifier::option::MinifyOptions;
        let _ = MinifyOptions::default();
    }
}
