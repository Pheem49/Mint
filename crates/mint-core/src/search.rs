//! Finding things: code search (symbol index, semantic embeddings), the
//! learned-knowledge store, web search, and workspace file/folder lookup.

pub mod files;
pub mod knowledge;
pub mod linked_folders;
pub mod semantic;
pub mod symbols;
pub mod repo_map;
pub mod local_embedding;
pub mod text_embedding;
pub mod web_search;
