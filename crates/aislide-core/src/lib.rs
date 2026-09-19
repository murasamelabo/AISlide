#![forbid(unsafe_code)]

pub mod package;
pub mod limits;
pub mod preflight;
pub mod model;
pub mod fonts;
pub mod text_ops;
pub mod proofing;
pub mod rich_text;
pub mod fields;
pub mod comments;
pub mod review;
pub mod table_format;
pub mod report;
pub mod pptx;
pub mod protocol;
pub mod generation;
pub mod text_assist;
mod charts;
mod chart_extended;
pub mod media;
pub mod visual;
pub mod vector;
pub mod geometry_ops;
pub mod image_edit;
pub mod segmentation;
pub mod graphics;
pub mod graphs;
pub mod sources;
pub mod data_report;
pub mod document;
pub mod recovery;
pub mod editing;
pub mod authoring_ops;
pub mod selection;
mod canonical;
pub mod layout;
pub mod render;
pub mod export_static;
pub mod import;
pub mod extraction;
pub mod publication;
pub mod design;
pub mod canvas;
pub mod templates;
pub mod design_presets;
pub mod objects;
pub mod parts;
pub mod guided;
mod native;
mod native_save;
mod native_slides;
mod native_design;
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