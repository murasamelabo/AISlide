use crate::{limits::{CapacityLimits, STANDARD}, model::{Deck, Element}, Error, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, io::{self, Write}};

struct BudgetWriter { remaining: usize, written: usize }

impl Write for BudgetWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.remaining { return Err(io::Error::other("serialized byte budget")); }
        self.remaining -= bytes.len();
        self.written += bytes.len();
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

pub fn serialized_bytes(value: &impl Serialize, limit: usize, label: &str) -> Result<usize> {
    let mut writer = BudgetWriter { remaining: limit, written: 0 };
    serde_json::to_writer(&mut writer, value).map_err(|error| {
        if error.is_io() { Error::Limit(format!("{label} exceeds {limit} bytes")) } else { Error::Json(error) }
    })?;
    Ok(writer.written)
}

pub fn deck(deck: &Deck, limits: &CapacityLimits) -> Result<()> {
    if deck.slides.is_empty() || deck.slides.len() > limits.slides {
        return Err(Error::Limit(format!("expected 1-{} slides", limits.slides)));
    }
    let mut total = 0;
    let mut images = 0;
    let mut resources = BTreeMap::new();
    let mut charts = 0;
    for slide in &deck.slides {
        let mut per_slide = 0;
        elements(&slide.elements, 0, &mut per_slide, &mut total, &mut images, &mut resources, limits)?;
        charts += crate::model::element_list(&slide.elements).iter().filter(|element| matches!(element, Element::Chart { .. })).count();
    }
    if let Some(design) = &deck.design {
        for template in design.masters.iter().map(|master| &master.elements).chain(design.layouts.iter().map(|layout| &layout.elements)) {
            let mut per_slide = 0;
            elements(template, 0, &mut per_slide, &mut total, &mut images, &mut resources, limits)?;
            charts += crate::model::element_list(template).iter().filter(|element| matches!(element, Element::Chart { .. })).count();
        }
    }
    let design_parts = deck.design.as_ref().map_or(5, |design| design.masters.len() * 3 + design.layouts.len() * 2);
    if 16 + deck.slides.len() * 4 + design_parts + charts * 3 + resources.len() * 2 > crate::package::MAX_PARTS {
        return Err(Error::Limit("generated resource part budget".into()));
    }
    let mut raster_work = 0u64;
    for (encoded, mime) in resources.values() {
        let bytes = decode_archive(encoded, 1024 * 1024)?;
        let format = match *mime { "image/png" => image::ImageFormat::Png, "image/jpeg" => image::ImageFormat::Jpeg, _ => return Err(Error::Unsupported("only PNG and JPEG raster images are accepted".into())) };
        let (width, height) = image::ImageReader::with_format(std::io::Cursor::new(bytes), format).into_dimensions()
            .map_err(|_| Error::Invalid("invalid raster dimensions".into()))?;
        if width == 0 || height == 0 || width > 4096 || height > 4096 { return Err(Error::Limit("image dimensions exceed 4096px".into())); }
        raster_work += u64::from(width) * u64::from(height) * 4;
        if raster_work > limits.raster_work_bytes as u64 { return Err(Error::Limit("unique raster work exceeds 64 MiB RGBA estimate".into())); }
    }
    Ok(())
}

fn elements<'a>(items: &'a [Element], depth: usize, per_slide: &mut usize, total: &mut usize, images: &mut usize, resources: &mut BTreeMap<[u8; 32], (&'a str, &'a str)>, limits: &CapacityLimits) -> Result<()> {
    if depth > limits.group_depth { return Err(Error::Limit("group nesting > 8".into())); }
    *per_slide += items.len();
    *total += items.len();
    if *per_slide > limits.elements_per_slide || *total > limits.elements_total { return Err(Error::Limit("too many scene elements".into())); }
    for element in items {
        match element {
            Element::Group { children, .. } => elements(children, depth + 1, per_slide, total, images, resources, limits)?,
            Element::Picture { base64, mime_type, svg, .. } => {
                *images = images.checked_add(base64.len()).and_then(|count| count.checked_add(svg.as_ref().map_or(0, String::len)))
                    .ok_or_else(|| Error::Limit("image size overflow".into()))?;
                if *images > limits.image_encoded_bytes { return Err(Error::Limit("scene image payload > 3 MiB encoded".into())); }
                let mut hash = Sha256::new();
                for field in [mime_type.as_bytes(), base64.as_bytes(), svg.as_deref().unwrap_or("").as_bytes()] {
                    hash.update(field.len().to_le_bytes()); hash.update(field);
                }
                resources.insert(hash.finalize().into(), (base64, mime_type));
                if resources.len() > limits.unique_images { return Err(Error::Limit("too many unique scene images".into())); }
            }
            _ => {}
        }
    }
    Ok(())
}

pub fn standard_deck(deck_value: &Deck) -> Result<()> { deck(deck_value, &STANDARD) }

pub fn value(value: &serde_json::Value, limits: &CapacityLimits, cancellation: Option<&crate::generation::CancellationToken>) -> Result<()> {
    fn visit(value: &serde_json::Value, depth: usize, nodes: &mut usize, limits: &CapacityLimits, cancellation: Option<&crate::generation::CancellationToken>) -> Result<()> {
        *nodes += 1;
        if depth > limits.json_depth || *nodes > limits.json_nodes { return Err(Error::Limit("JSON depth or node budget".into())); }
        if *nodes % 1024 == 1 && cancellation.is_some_and(|token| token.is_cancelled()) { return Err(Error::Generation("operation cancelled".into())); }
        match value {
            serde_json::Value::Object(object) => {
                for child in object.values() { visit(child, depth + 1, nodes, limits, cancellation)?; }
                if let Some(sections) = object.get("sections").and_then(serde_json::Value::as_array) {
                    if sections.len() > limits.slides { return Err(Error::Limit(format!("expected at most {} report sections", limits.slides))); }
                }
                if let Some(slides) = object.get("slides").and_then(serde_json::Value::as_array) {
                    if object.contains_key("version") {
                        if slides.is_empty() || slides.len() > limits.slides { return Err(Error::Limit(format!("expected 1-{} slides", limits.slides))); }
                        let mut total = 0;
                        for slide in slides {
                            let mut count = 0;
                            scene_elements(&slide["elements"], 0, &mut count, &mut total, limits)?;
                        }
                        if let Some(design) = object.get("design") {
                            for collection in ["masters", "layouts"] {
                                for template in design[collection].as_array().into_iter().flatten() {
                                    scene_elements(&template["elements"], 0, &mut 0, &mut total, limits)?;
                                }
                            }
                        }
                    }
                }
                if object.contains_key("deck") && object.contains_key("sources") && object.contains_key("bindings") {
                    serialized_bytes(object, limits.document_bytes, "complete document including immutable origin")?;
                    if let Some(encoded) = object.get("origin").and_then(|origin|origin.get("base64")).and_then(serde_json::Value::as_str) {
                        archive_admission(encoded, limits.archive_bytes)?;
                    }
                }
            }
            serde_json::Value::Array(array) => for child in array { visit(child, depth + 1, nodes, limits, cancellation)?; },
            _ => {}
        }
        Ok(())
    }
    visit(value, 0, &mut 0, limits, cancellation)
}

fn scene_elements(value: &serde_json::Value, depth: usize, count: &mut usize, total: &mut usize, limits: &CapacityLimits) -> Result<()> {
    if depth > limits.group_depth { return Err(Error::Limit("group nesting > 8".into())); }
    if let Some(items) = value.as_array() {
        *count += items.len(); *total += items.len();
        if *count > limits.elements_per_slide || *total > limits.elements_total { return Err(Error::Limit("too many scene elements".into())); }
        for item in items {
            if let Some(children) = item.get("children") { scene_elements(children, depth + 1, count, total, limits)?; }
        }
    }
    Ok(())
}

pub fn archive_admission(encoded: &str, limit: usize) -> Result<()> {
    if encoded.len() > limit.div_ceil(3) * 4 { return Err(Error::Limit("encoded archive budget".into())); }
    let padding = if encoded.ends_with("==") { 2 } else if encoded.ends_with('=') { 1 } else { 0 };
    if base64::decoded_len_estimate(encoded.len()).saturating_sub(padding) > limit { return Err(Error::Limit("decoded archive budget".into())); }
    Ok(())
}

pub fn decode_archive(encoded: &str, limit: usize) -> Result<Vec<u8>> {
    use base64::{Engine, engine::general_purpose::STANDARD};
    archive_admission(encoded, limit)?;
    let bytes = STANDARD.decode(encoded).map_err(|_| Error::Invalid("invalid base64".into()))?;
    if bytes.len() > limit { return Err(Error::Limit("decoded archive budget".into())); }
    Ok(bytes)
}

pub fn json(bytes: &[u8]) -> Result<serde_json::Value> {
    use serde::de::{DeserializeSeed, Deserializer, IgnoredAny, MapAccess, SeqAccess, Visitor};
    struct Seed<'a> { nodes: &'a mut usize, depth: usize }
    impl<'de> DeserializeSeed<'de> for Seed<'_> {
        type Value = ();
        fn deserialize<Decoder: Deserializer<'de>>(self, decoder: Decoder) -> std::result::Result<(), Decoder::Error> {
            *self.nodes += 1;
            if *self.nodes > STANDARD.json_nodes || self.depth > STANDARD.json_depth {
                return Err(serde::de::Error::custom("JSON depth or node budget"));
            }
            decoder.deserialize_any(self)
        }
    }
    impl<'de> Visitor<'de> for Seed<'_> {
        type Value = ();
        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result { formatter.write_str("bounded JSON") }
        fn visit_bool<Failure: serde::de::Error>(self, _: bool) -> std::result::Result<(), Failure> { Ok(()) }
        fn visit_i64<Failure: serde::de::Error>(self, _: i64) -> std::result::Result<(), Failure> { Ok(()) }
        fn visit_u64<Failure: serde::de::Error>(self, _: u64) -> std::result::Result<(), Failure> { Ok(()) }
        fn visit_f64<Failure: serde::de::Error>(self, _: f64) -> std::result::Result<(), Failure> { Ok(()) }
        fn visit_str<Failure: serde::de::Error>(self, _: &str) -> std::result::Result<(), Failure> { Ok(()) }
        fn visit_unit<Failure: serde::de::Error>(self) -> std::result::Result<(), Failure> { Ok(()) }
        fn visit_seq<Sequence: SeqAccess<'de>>(self, mut sequence: Sequence) -> std::result::Result<(), Sequence::Error> {
            while sequence.next_element_seed(Seed { nodes: self.nodes, depth: self.depth + 1 })?.is_some() {}
            Ok(())
        }
        fn visit_map<Mapping: MapAccess<'de>>(self, mut mapping: Mapping) -> std::result::Result<(), Mapping::Error> {
            while mapping.next_key::<IgnoredAny>()?.is_some() {
                mapping.next_value_seed(Seed { nodes: self.nodes, depth: self.depth + 1 })?;
            }
            Ok(())
        }
    }
    if bytes.len() > crate::limits::LARGE.request_bytes { return Err(Error::Limit("JSON request exceeds 96 MiB".into())); }
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    Seed { nodes: &mut 0, depth: 0 }.deserialize(&mut decoder)?;
    decoder.end()?;
    Ok(serde_json::from_slice(bytes)?)
}