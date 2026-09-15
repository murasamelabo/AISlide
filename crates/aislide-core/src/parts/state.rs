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

pub fn change(document:&Document,expected_revision:u64,slide_id:&str,id:&str,spec:&PartSpec,update:bool)->Result<TransactionResult> {
    crate::document::verify(document)?;
    if document.revision!=expected_revision {return Err(Error::Conflict("stale document revision".into()));}
    let mut parts=document.parts.clone();let mut deck=document.deck.clone();
    let slide=deck.slides.iter_mut().find(|slide|slide.id==slide_id).ok_or_else(||Error::Invalid("unknown part slide".into()))?;
    let fallback=crate::design::Theme::default();
    let mut element=super::create_with_theme(id,spec,document.deck.design.as_ref().map(|design|&design.theme).unwrap_or(&fallback))?;
    let existing=parts.iter().position(|part|part.slide_id==slide_id && part.element_id==id);
    let native_sha256=existing.and_then(|index|parts[index].native_sha256.clone());
    if update {
        let index=existing.ok_or_else(||Error::Invalid("part metadata missing".into()))?;
        if parts[index].stale {return Err(Error::Conflict("part metadata is stale; retain manual edits or insert a new part".into()));}
        let current=slide.elements.iter_mut().find(|element|element.bounds().0==id).ok_or_else(||Error::Conflict("part element missing".into()))?;
        if let Element::Group{x,y,width,height,..}=&mut element {let bounds=current.bounds();*x=bounds.1;*y=bounds.2;*width=bounds.3;*height=bounds.4;}
        *current=element.clone();parts.remove(index);
    } else {if existing.is_some() || slide.elements.iter().any(|element|element.bounds().0==id) {return Err(Error::Conflict("part identity already exists".into()));}slide.elements.push(element.clone());}
    parts.push(PartInstance {slide_id:slide_id.into(),element_id:id.into(),spec:spec.clone(),render_sha256:render_hash(&element)?,native_sha256,stale:false});
    crate::document::transact(document,Transaction {expected_revision,expected_hash:document.hash.clone(),operations:serde_json::from_value(json!([{"op":"replace","path":"/deck","value":deck},{"op":"add","path":"/parts","value":parts}]))?})
}