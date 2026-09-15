#![forbid(unsafe_code)]

pub mod package;
pub mod model;
pub mod report;
pub mod pptx;
pub mod protocol;
pub mod generation;
mod charts;
pub mod media;
pub mod graphics;
pub mod graphs;
pub mod sources;
pub mod data_report;
pub mod document;
pub mod editing;
mod canonical;
pub mod layout;
pub mod import;
pub mod extraction;
pub mod publication;
pub mod design;
pub mod objects;
pub mod parts;
mod native;
mod native_save;
mod native_slides;
mod provenance;
pub use protocol::execute_request;

#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("Invalid input: {0}")]
	Invalid(String),
	#[error("Unsupported: {0}")]
	Unsupported(String),
	#[error("Resource limit exceeded: {0}")]
	Limit(String),
	#[error("Conflict: {0}")]
	Conflict(String),
	#[error("Generation: {0}")]
	Generation(String),
	#[error("Generation: {0}")]
	ModelOutput(String),
	#[error("Archive: {0}")]
	Zip(#[from] zip::result::ZipError),
	#[error("I/O: {0}")]
	Io(#[from] std::io::Error),
	#[error("XML: {0}")]
	Xml(#[from] roxmltree::Error),
	#[error("JSON: {0}")]
	Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;