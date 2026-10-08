pub mod books;
pub mod index;
pub mod json;
pub mod layout;
pub mod search;
pub mod text;

#[cfg(target_arch = "wasm32")]
mod wasm;
