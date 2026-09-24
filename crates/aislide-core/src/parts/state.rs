use super::PartSpec;
use crate::{document::{Document, ImportedOrigin, Transaction, TransactionResult}, model::{Deck, Element}, native::{NativeDeck, properties, is_shape}, package::Package, Error, Result};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Serialize, Deserialize};
use serde_json::json;
use sha2::{Digest,Sha256};
use std::collections::BTreeSet;

#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartInstance {
    pub slide_id:String, pub element_id:String, pub spec:PartSpec,
    pub render_sha256:String,
    #[serde(default,skip_serializing_if="Option::is_none")] pub native_sha256:Option<String>,
    #[serde(default)] pub stale:bool,
}

pub(crate) struct NativeRegenerationGuard<'a> {
    origin: Option<&'a ImportedOrigin>,
    original: Option<(Package, NativeDeck)>,
    checked: BTreeSet<(String, String)>,
}

impl<'a> NativeRegenerationGuard<'a> {
    pub(crate) fn new(document: &'a Document) -> Self {
        Self { origin: document.origin.as_ref().filter(|origin| origin.native), original: None, checked: BTreeSet::new() }
    }

    fn check(&mut self, slide: &crate::model::Slide, id: &str) -> Result<()> {
        let Some(origin) = self.origin else { return Ok(()); };
        let source = slide.native_source_id.as_deref().unwrap_or(&slide.id);
        let identity = (source.to_owned(), id.to_owned());
        if self.checked.contains(&identity) { return Ok(()); }
        if self.original.is_none() {
            let package = Package::open(STANDARD.decode(&origin.base64).map_err(|_| Error::Invalid("part origin".into()))?)?;
            let native = crate::native::read(&package)?;
            self.original = Some((package, native));
        }
        if let Some((package, native)) = &self.original {
            if let Some(binding) = native.slides.iter().find(|binding| binding.id == source) {
                let original = native.deck.slides.iter().find(|slide| slide.id == source).and_then(|slide| slide.elements.iter().find(|element| element.bounds().0 == id));
                if let Some(original) = original {
                    for element in crate::model::element_list(std::slice::from_ref(original)) {
                        if let Element::Chart { id, kind, categories, series, options, .. } = element {
                            if !crate::native_save::chart_is_representable(package, binding, id, *kind, categories, series, options)? {
                                return Err(Error::Unsupported("custom native chart dependencies, settings or workbook content cannot be regenerated as a managed part".into()));
                            }
                        }
                    }
                }
            }
        }
        self.checked.insert(identity);
        Ok(())
    }
}

pub(crate) fn render_hash(element:&Element)->Result<String> {
    let mut value=serde_json::to_value(element)?;
    if let Some(object)=value.as_object_mut() {for field in ["x","y","width","height"] {object.remove(field);}}
    Ok(format!("{:x}",Sha256::digest(crate::canonical::bytes(&value)?)))
}

pub(crate) fn native_hash(package:&Package,native:&NativeDeck,slide_id:&str,element_id:&str)->Result<String> {
    let part=native.slides.iter().find(|part|part.id==slide_id).ok_or_else(||Error::Conflict("part slide missing".into()))?;
    let numeric=part.nodes.get(element_id).ok_or_else(||Error::Conflict("part native identity missing".into()))?;
    let xml=package.text(&part.path)?;let parsed=crate::pptx::parse(xml)?;
    let node=parsed.descendants().find(|node|is_shape(*node) && properties(*node).and_then(|node|node.attribute("id"))==Some(numeric)).ok_or_else(||Error::Conflict("part native shape missing".into()))?;
    let mut digest=Sha256::new();digest.update(&xml.as_bytes()[node.range()]);
    for (kind,namespace,tag,attribute) in [("chart",crate::native::C,"chart","id"),("image",crate::native::A,"blip","embed")] {
        let targets=crate::pptx::relationship_targets(package,&part.path,kind)?;
        for reference in node.descendants().filter(|node|node.has_tag_name((namespace,tag))).filter_map(|node|node.attribute((crate::native::R,attribute))) {
            let path=targets.get(reference).ok_or_else(||Error::Conflict("part resource missing".into()))?;digest.update(path.as_bytes());digest.update(package.part(path)?);
            if kind=="chart" {for workbook in crate::pptx::relationship_targets(package,path,"package")?.values() {digest.update(workbook.as_bytes());digest.update(package.part(workbook)?);}}
        }
    }
    Ok(format!("{:x}",digest.finalize()))
}

pub(crate) fn refresh(parts:&mut [PartInstance],deck:&Deck,origin:Option<&ImportedOrigin>)->Result<()> {
    if parts.len()>128 {return Err(Error::Limit("more than 128 metadata parts".into()));}
    let original=if !parts.is_empty() {origin.filter(|origin|origin.native).map(|origin|->Result<_>{let package=Package::open(STANDARD.decode(&origin.base64).map_err(|_|Error::Invalid("part origin".into()))?)?;let native=crate::native::read(&package)?;Ok((package,native))}).transpose()?} else {None};
    let mut identities=BTreeSet::new();
    for part in parts {
        crate::model::valid_text(&part.slide_id,80)?;crate::model::valid_text(&part.element_id,40)?;
        if !identities.insert((&part.slide_id,&part.element_id)) || part.render_sha256.len()!=64 || !part.render_sha256.bytes().all(|byte|byte.is_ascii_hexdigit()) {return Err(Error::Invalid("part metadata identity/hash".into()));}
        super::validate_spec(&part.spec)?;
        let element=deck.slides.iter().find(|slide|slide.id==part.slide_id).and_then(|slide|slide.elements.iter().find(|element|element.bounds().0==part.element_id));
        let native_changed=match &part.native_sha256 {
            Some(expected)=>original.as_ref().is_none_or(|(package,native)|native_hash(package,native,&part.slide_id,&part.element_id).map_or(true,|hash|&hash!=expected)),
            None=>original.as_ref().is_some_and(|(_,native)|native.slides.iter().any(|slide|slide.id==part.slide_id && slide.nodes.contains_key(&part.element_id))),
        };
        part.stale=native_changed || element.map(render_hash).transpose()?.is_none_or(|hash|hash!=part.render_sha256);
    }
    Ok(())
}

pub(crate) fn resize_canvas(element: &mut Element, target_width: f64, target_height: f64) -> Result<()> {
    let Element::Group { view_width, view_height, .. } = element else { return Ok(()); };
    let horizontal = target_width / *view_width;
    let vertical = target_height / *view_height;
    fn scale(value: &mut serde_json::Value, horizontal: f64, vertical: f64) {
        for (field, factor) in [("x", horizontal), ("y", vertical), ("width", horizontal), ("height", vertical), ("view_width", horizontal), ("view_height", vertical)] {
            if let Some(number) = value.get(field).and_then(serde_json::Value::as_f64) { value[field] = json!(number * factor); }
        }
        let factor = horizontal.min(vertical);
        if let Some(number) = value.get("font_size").and_then(serde_json::Value::as_f64) { value["font_size"] = json!((number * factor).clamp(8.0, 120.0)); }
        if let Some(number) = value.get("stroke_width").and_then(serde_json::Value::as_f64).filter(|number| *number > 0.0) { value["stroke_width"] = json!((number * factor).max(0.5)); }
        if let Some(children) = value.get_mut("children").and_then(serde_json::Value::as_array_mut) { for child in children { scale(child, horizontal, vertical); } }
    }
    let mut value = serde_json::to_value(&*element)?;
    if let Some(children) = value.get_mut("children").and_then(serde_json::Value::as_array_mut) { for child in children { scale(child, horizontal, vertical); } }
    value["view_width"] = json!(target_width); value["view_height"] = json!(target_height);
    *element = serde_json::from_value(value)?;
    Ok(())
}

pub fn change(document:&Document,expected_revision:u64,slide_id:&str,id:&str,spec:&PartSpec,update:bool)->Result<TransactionResult> {
    crate::document::verify(document)?;
    if document.revision!=expected_revision {return Err(Error::Conflict("stale document revision".into()));}
    let mut parts=document.parts.clone();let mut deck=document.deck.clone();
    let mut native_guard=NativeRegenerationGuard::new(document);
    change_in_deck(&mut deck,&mut parts,slide_id,id,spec,update,&mut native_guard)?;
    crate::document::transact(document,Transaction {expected_revision,expected_hash:document.hash.clone(),operations:serde_json::from_value(json!([{"op":"replace","path":"/deck","value":deck},{"op":"add","path":"/parts","value":parts}]))?})
}

pub(crate) fn change_in_deck(deck:&mut Deck,parts:&mut Vec<PartInstance>,slide_id:&str,id:&str,spec:&PartSpec,update:bool,native_guard:&mut NativeRegenerationGuard<'_>)->Result<()> {
    if !update && parts.len()>=128 {return Err(Error::Limit("more than 128 metadata parts".into()));}
    let slide=deck.slides.iter_mut().find(|slide|slide.id==slide_id).ok_or_else(||Error::Invalid("unknown part slide".into()))?;
    let fallback=crate::design::Theme::default();
    let existing=parts.iter().position(|part|part.slide_id==slide_id && part.element_id==id);
    let mut effective_spec=spec.clone();
    if update {
        let index=existing.ok_or_else(||Error::Invalid("part metadata missing".into()))?;
        if parts[index].stale {return Err(Error::Conflict("part metadata is stale; retain manual edits or insert a new part".into()));}
        let current=slide.elements.iter().find(|element|element.bounds().0==id).ok_or_else(||Error::Conflict("part element missing".into()))?;
        if render_hash(current)?!=parts[index].render_sha256 {return Err(Error::Conflict("part metadata is stale after an earlier edit in this batch".into()));}
        if crate::model::element_list(std::slice::from_ref(current)).iter().any(|element|element.visual().is_some_and(|visual|visual.locked || visual.hidden)) {return Err(Error::Unsupported("unlock and show the managed part before updating".into()));}
        native_guard.check(slide,id)?;
        if let Some(layout)=&mut effective_spec.layout {
            if parts[index].spec.layout.as_ref().is_some_and(|previous| [previous.x,previous.y,previous.width,previous.height]==[layout.x,layout.y,layout.width,layout.height]) {
                let (_,x,y,width,height)=current.bounds();
                layout.x=x;layout.y=y;layout.width=width;layout.height=height;
            }
        }
    }
    let mut element=super::create_with_theme(id,&effective_spec,crate::design::slide_theme(slide,deck.design.as_ref()).unwrap_or(&fallback))?;
    if !update && effective_spec.layout.is_none() {
        let region = deck.design.as_ref().and_then(|design| design.layouts.iter().find(|layout| Some(&layout.id) == slide.layout_id.as_ref() && layout.id == "preset-visual-content"))
            .and_then(|layout| layout.elements.iter().find(|element| element.bounds().0 == "preset-visual-region"));
        if let (Some(region), Element::Group { x, y, width, height, .. }) = (region, &mut element) {
            let (_, left, top, region_width, region_height) = region.bounds();
            let scale = (region_width / *width).min(region_height / *height);
            *width *= scale; *height *= scale;
            *x = left + (region_width - *width) / 2.0; *y = top + (region_height - *height) / 2.0;
            let target = (*width, *height);
            resize_canvas(&mut element, target.0, target.1)?;
        }
    }
    let native_sha256=existing.and_then(|index|parts[index].native_sha256.clone());
    if update {
        let index=existing.ok_or_else(||Error::Invalid("part metadata missing".into()))?;
        if parts[index].stale {return Err(Error::Conflict("part metadata is stale; retain manual edits or insert a new part".into()));}
        let current=slide.elements.iter_mut().find(|element|element.bounds().0==id).ok_or_else(||Error::Conflict("part element missing".into()))?;
        if effective_spec.layout.is_none() {
            if let Element::Group { view_width, view_height, .. } = current { resize_canvas(&mut element, *view_width, *view_height)?; }
            if let Element::Group{x,y,width,height,..}=&mut element {let bounds=current.bounds();*x=bounds.1;*y=bounds.2;*width=bounds.3;*height=bounds.4;}
        }
        *current=element.clone();
    } else {if existing.is_some() || slide.elements.iter().any(|element|element.bounds().0==id) {return Err(Error::Conflict("part identity already exists".into()));}slide.elements.push(element.clone());}
    let instance=PartInstance {slide_id:slide_id.into(),element_id:id.into(),spec:effective_spec,render_sha256:render_hash(&element)?,native_sha256,stale:false};
    if let Some(index)=existing {parts[index]=instance;} else {parts.push(instance);}
    Ok(())
}