use crate::{Error, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub const MAX_FONT_BYTES: usize = 12 * 1024 * 1024;
pub const MAX_TOTAL_FONT_BYTES: usize = 24 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EmbeddedFont {
    pub family: String,
    pub style: FontStyle,
    pub base64: String,
    #[serde(default)]
    pub license_acknowledged: bool,
}

pub fn validate(resources: &[EmbeddedFont]) -> Result<()> {
    if resources.len() > 8 { return Err(Error::Limit("at most 8 embedded font faces".into())); }
    let mut total = 0;
    let mut keys = std::collections::BTreeSet::new();
    for resource in resources {
        let bytes = decode(&resource.base64)?;
        total += bytes.len();
        if total > MAX_TOTAL_FONT_BYTES { return Err(Error::Limit("combined fonts > 24 MiB; document including immutable origin must also fit the selected profile".into())); }
        let info = inspect(&bytes)?;
        eligible(&info)?;
        if info.family != resource.family || info.style != resource.style { return Err(invalid("metadata does not match bytes")); }
        if !keys.insert((resource.family.to_lowercase(), style_tag(resource.style))) { return Err(invalid("duplicate family/style")); }
    }
    Ok(())
}

fn eligible(info: &FontInfo) -> Result<()> {
    if !matches!(info.permission, FontPermission::Installable | FontPermission::Editable) {
        return Err(Error::Unsupported("font does not permit editable embedding (restricted, preview/print, bitmap-only or unknown rights)".into()));
    }
    if !info.usable { return Err(Error::Unsupported("font must be a complete static outline TTF/OTF; variable/color/metadata-only fonts are unsupported".into())); }
    Ok(())
}

pub fn embed(document: &crate::document::Document, expected_revision: u64, base64: String, license_acknowledged: bool) -> Result<crate::document::TransactionResult> {
    if !license_acknowledged { return Err(invalid("confirm your license permits document embedding and editing")); }
    let info = inspect_base64(&base64)?;
    eligible(&info)?;
    let mut resources = document.deck.embedded_fonts.clone();
    if let Some(existing) = resources.iter_mut().find(|font| font.family.eq_ignore_ascii_case(&info.family) && font.style == info.style) {
        if existing.base64 != base64 { return Err(Error::Conflict("a different font already occupies this family/style".into())); }
        existing.license_acknowledged = true;
    } else { resources.push(EmbeddedFont { family: info.family, style: info.style, base64, license_acknowledged }); }
    validate(&resources)?;
    crate::document::transact(document, crate::document::Transaction { expected_revision, expected_hash: document.hash.clone(),
        operations: serde_json::from_value(serde_json::json!([{"op":"add","path":"/deck/embedded_fonts","value":resources}]))? })
}

pub fn set_usage(document: &crate::document::Document, expected_revision: u64, sha256: &str, license_acknowledged: bool) -> Result<crate::document::TransactionResult> {
    let mut resources = document.deck.embedded_fonts.clone();
    let mut found = false;
    for font in &mut resources {
        if inspect_base64(&font.base64)?.sha256 == sha256 { font.license_acknowledged = license_acknowledged; found = true; }
    }
    if !found { return Err(invalid("unknown font hash")); }
    crate::document::transact(document, crate::document::Transaction { expected_revision, expected_hash: document.hash.clone(),
        operations: serde_json::from_value(serde_json::json!([{"op":"add","path":"/deck/embedded_fonts","value":resources}]))? })
}

pub fn infos(deck: &crate::model::Deck) -> Result<Vec<FontInfo>> {
    validate(&deck.embedded_fonts)?;
    deck.embedded_fonts.iter().map(|font| inspect_base64(&font.base64)).collect()
}

pub(crate) fn document_system(base: &cosmic_text::FontSystem, deck: &crate::model::Deck) -> Result<Option<cosmic_text::FontSystem>> {
    let active: Vec<_> = deck.embedded_fonts.iter().filter(|font| font.license_acknowledged).collect();
    if active.is_empty() { return Ok(None); }
    validate(&deck.embedded_fonts)?;
    let mut database = base.db().clone();
    let replaced: Vec<_> = database.faces().filter(|face| active.iter().any(|font| face.families.iter().any(|(name,_)| name.eq_ignore_ascii_case(&font.family)))).map(|face|face.id).collect();
    for id in replaced { database.remove_face(id); }
    for font in active {
        let ids = database.load_font_source(cosmic_text::fontdb::Source::Binary(std::sync::Arc::new(decode(&font.base64)?)));
        if ids.len()!=1 { return Err(Error::Unsupported("font could not be loaded into the document database".into())); }
        let id=ids[0];
        let mut face=database.face(id).cloned().ok_or_else(||invalid("loaded face missing"))?;
        let language=face.families.first().ok_or_else(||invalid("loaded family missing"))?.1;
        face.families.retain(|(name,_)|!name.eq_ignore_ascii_case(&font.family));
        face.families.insert(0,(font.family.clone(),language));
        database.remove_face(id); database.push_face_info(face);
    }
    Ok(Some(cosmic_text::FontSystem::new_with_locale_and_db(base.locale().into(), database)))
}

fn style_tag(style: FontStyle) -> &'static str {
    match style { FontStyle::Regular => "regular", FontStyle::Bold => "bold", FontStyle::Italic => "italic", FontStyle::BoldItalic => "boldItalic" }
}

pub fn to_eot(bytes: &[u8]) -> Result<Vec<u8>> {
    let info = inspect(bytes)?;
    eligible(&info)?;
    let tables = tables(bytes)?;
    let os2 = tables.get(b"OS/2").ok_or_else(|| invalid("OS/2 missing"))?;
    let head = tables.get(b"head").ok_or_else(|| invalid("head missing"))?;
    let names = tables.get(b"name").and_then(|data| ttf_parser::name::Table::parse(data)).ok_or_else(|| invalid("name table"))?;
    let mut output = vec![0u8; 80];
    output[4..8].copy_from_slice(&(bytes.len() as u32).to_le_bytes());
    output[8..12].copy_from_slice(&0x10000u32.to_le_bytes());
    output[16..26].copy_from_slice(&os2[32..42]);
    output[26] = 1;
    output[27] = u8::from(matches!(info.style, FontStyle::Italic | FontStyle::BoldItalic));
    output[28..32].copy_from_slice(&(word(os2,4)? as u32).to_le_bytes());
    output[32..34].copy_from_slice(&info.fs_type.ok_or_else(|| invalid("fsType missing"))?.to_le_bytes());
    output[34..36].copy_from_slice(&0x504cu16.to_le_bytes());
    for (target, source) in [(36,42),(40,46),(44,50),(48,54),(52,78),(56,82)] {
        let value = if source >= 78 && word(os2,0)? == 0 { 0 } else { dword(os2,source)? as u32 };
        output[target..target+4].copy_from_slice(&value.to_le_bytes());
    }
    output[60..64].copy_from_slice(&(dword(head,8)? as u32).to_le_bytes());
    for (id, fallback) in [(1,info.family.as_str()),(2,style_tag(info.style)),(5,"Version 1.0"),(4,info.family.as_str())] {
        let value = names.names.into_iter().filter(|name|name.name_id==id && name.language_id==0x409).find_map(|name|name.to_string()).unwrap_or_else(||fallback.into());
        let encoded: Vec<_> = value.encode_utf16().flat_map(u16::to_le_bytes).collect();
        if encoded.len() > 8192 { return Err(Error::Limit("EOT name length".into())); }
        output.extend([0,0]); output.extend((encoded.len() as u16).to_le_bytes()); output.extend(encoded);
    }
    output.extend(bytes);
    let length = output.len() as u32;
    output[..4].copy_from_slice(&length.to_le_bytes());
    Ok(output)
}

pub fn from_eot(bytes: &[u8]) -> Result<&[u8]> {
    if bytes.len() > MAX_FONT_BYTES + 32784 || bytes.len() < 96 { return Err(invalid("EOT size")); }
    let long = |offset: usize| -> Result<usize> {
        let slice: [u8;4] = bytes.get(offset..offset+4).ok_or_else(||invalid("EOT field"))?.try_into().map_err(|_|invalid("EOT field"))?;
        Ok(u32::from_le_bytes(slice) as usize)
    };
    let short = |offset: usize| -> Result<usize> {
        let slice: [u8;2] = bytes.get(offset..offset+2).ok_or_else(||invalid("EOT field"))?.try_into().map_err(|_|invalid("EOT field"))?;
        Ok(u16::from_le_bytes(slice) as usize)
    };
    if long(0)? != bytes.len() || long(8)? != 0x10000 || long(12)? != 0 || short(34)? != 0x504c || bytes[64..80].iter().any(|byte|*byte!=0) {
        return Err(Error::Unsupported("only uncompressed, non-obfuscated EOT v1 full fonts are loaded; other encodings remain opaque".into()));
    }
    let mut offset = 80;
    for _ in 0..4 {
        if short(offset)? != 0 { return Err(invalid("EOT padding")); }
        let length = short(offset+2)?;
        if length % 2 != 0 || length > 8192 { return Err(invalid("EOT string length")); }
        offset = offset.checked_add(4+length).filter(|end|*end<=bytes.len()).ok_or_else(||invalid("EOT string range"))?;
    }
    let font = &bytes[offset..];
    if font.len() != long(4)? { return Err(invalid("EOT data length")); }
    let info = inspect(font)?;
    if info.fs_type.map(usize::from) != Some(short(32)?) { return Err(invalid("EOT permissions disagree with SFNT")); }
    Ok(font)
}

#[derive(Serialize)]
pub struct NativeFontInfo {
    pub family: String, pub style: FontStyle, pub sha256: Option<String>, pub byte_length: usize,
    pub info: Option<FontInfo>, pub status: String,
}

pub fn inspect_pptx(bytes: Vec<u8>) -> Result<Vec<NativeFontInfo>> {
    let package = crate::package::Package::open(bytes)?;
    let roots = crate::pptx::relationship_targets(&package,"","officeDocument")?;
    let main = roots.values().next().ok_or_else(||invalid("presentation missing"))?;
    Ok(read_native(&package,main)?.1)
}

pub(crate) fn read_native(package: &crate::package::Package, main: &str) -> Result<(Vec<EmbeddedFont>, Vec<NativeFontInfo>)> {
    use crate::native::{P,R};
    let xml = crate::pptx::parse(package.text(main)?)?;
    let Some(list) = xml.root_element().children().find(|node|node.has_tag_name((P,"embeddedFontLst"))) else { return Ok((Vec::new(),Vec::new())); };
    let relation_xml=crate::pptx::parse(package.text(&crate::native::relations_path(main))?)?;
    let mut targets=BTreeMap::new();
    for relation in relation_xml.root_element().children().filter(|node|node.has_tag_name(("http://schemas.openxmlformats.org/package/2006/relationships","Relationship")) && node.attribute("Type")==Some(format!("{R}/font").as_str())) {
        if relation.attribute("TargetMode").is_some_and(|mode|mode!="Internal") { continue; }
        if let (Some(id),Some(path))=(relation.attribute("Id"),relation.attribute("Target").and_then(|target|crate::pptx::resolve(main,target).ok())) {
            if package.part(&path).is_ok() { targets.insert(id,path); }
        }
    }
    let mut resources = Vec::new(); let mut inventory = Vec::new(); let mut total = 0;
    for entry in list.children().filter(|node|node.has_tag_name((P,"embeddedFont"))) {
        let family = entry.children().find(|node|node.has_tag_name((P,"font"))).and_then(|node|node.attribute("typeface")).unwrap_or("");
        if family.len()>128 { return Err(Error::Limit("embedded font family".into())); }
        for style in [FontStyle::Regular,FontStyle::Bold,FontStyle::Italic,FontStyle::BoldItalic] {
            let Some(node) = entry.children().find(|node|node.has_tag_name((P,style_tag(style)))) else { continue; };
            if inventory.len()>=64 { return Err(Error::Limit("embedded font inventory > 64 faces".into())); }
            let bytes = node.attribute((R,"id")).and_then(|id|targets.get(id)).and_then(|path|package.part(path).ok());
            let parsed = bytes.and_then(|data|from_eot(data).ok()).and_then(|font|inspect(font).ok().map(|info|(font,info)));
            let supported = parsed.as_ref().is_some_and(|(_,info)|eligible(info).is_ok() && info.family==family && info.style==style);
            let mut status = if supported { "consent_required" } else { "opaque_preserved" };
            if supported {
                let (font,_) = parsed.as_ref().ok_or_else(||invalid("font metadata"))?;
                if resources.len()<8 && total+font.len()<=MAX_TOTAL_FONT_BYTES && !resources.iter().any(|existing: &EmbeddedFont|existing.family.eq_ignore_ascii_case(family) && existing.style==style) {
                    total += font.len();
                    resources.push(EmbeddedFont {family:family.into(),style,base64:STANDARD.encode(font),license_acknowledged:false});
                } else { status = "opaque_preserved_capacity"; }
            }
            inventory.push(NativeFontInfo {family:family.into(),style,sha256:bytes.map(|bytes|format!("{:x}",Sha256::digest(bytes))),byte_length:bytes.map_or(0,<[u8]>::len),info:parsed.map(|(_,info)|info),status:status.into()});
        }
    }
    Ok((resources,inventory))
}

fn used(deck: &crate::model::Deck, family: &str) -> Result<bool> {
    fn has_family(value: &serde_json::Value, family: &str) -> bool {
        match value {
            serde_json::Value::Object(object) => object.get("font_family").and_then(serde_json::Value::as_str).is_some_and(|value|value.eq_ignore_ascii_case(family)) || object.values().any(|value|has_family(value,family)),
            serde_json::Value::Array(array) => array.iter().any(|value|has_family(value,family)), _ => false,
        }
    }
    if has_family(&serde_json::to_value(&deck.slides)?,family) { return Ok(true); }
    if let Some(design) = &deck.design {
        if has_family(&serde_json::to_value(design)?,family) { return Ok(true); }
        for theme in std::iter::once(&design.theme).chain(design.masters.iter().filter_map(|master| master.theme.as_ref())) {
            let fonts = &theme.fonts;
            if [&fonts.major,&fonts.minor,&fonts.east_asian,&fonts.complex_script].iter().any(|name|name.eq_ignore_ascii_case(family)) {
                return Ok(deck.slides.iter().map(|slide| &slide.elements).chain(design.masters.iter().map(|master| &master.elements)).chain(design.layouts.iter().map(|layout| &layout.elements)).flat_map(|elements|crate::model::element_list(elements)).any(|element|matches!(element,crate::model::Element::Text {text,..}|crate::model::Element::Shape {text,..} if !text.is_empty())));
            }
        }
    }
    Ok(false)
}

fn append_xml(xml: &str, node: roxmltree::Node<'_, '_>, fragment: &str) -> Result<Vec<u8>> {
    let close = xml[node.range()].rfind("</").ok_or_else(||invalid("XML container must have an end tag"))? + node.range().start;
    crate::native_save::apply(xml.into(),vec![(close..close,fragment.into())])
}

pub(crate) fn write_native(package: &mut crate::package::Package, main: &str, before: &[EmbeddedFont], deck: &crate::model::Deck) -> Result<()> {
    use crate::native::{P,R};
    validate(&deck.embedded_fonts)?;
    for old in before {
        if !deck.embedded_fonts.iter().any(|font|font.family==old.family && font.style==old.style && font.base64==old.base64) {
            return Err(Error::Unsupported("removing/replacing original embedded fonts is not supported; original binary parts are preserved".into()));
        }
    }
    let additions: Vec<_> = deck.embedded_fonts.iter().filter(|font|!before.iter().any(|old|old.family==font.family && old.style==font.style && old.base64==font.base64)).collect();
    if additions.is_empty() { return Ok(()); }
    let xml = package.text(main)?.to_owned(); let parsed = crate::pptx::parse(&xml)?;
    let list = parsed.root_element().children().find(|node|node.has_tag_name((P,"embeddedFontLst")));
    let existing: Vec<_> = list.into_iter().flat_map(|node|node.children()).filter(|node|node.has_tag_name((P,"embeddedFont")))
        .filter_map(|node|node.children().find(|node|node.has_tag_name((P,"font"))).and_then(|node|node.attribute("typeface"))).collect();
    let relation_path = crate::native::relations_path(main);
    let relation_xml = package.text(&relation_path)?.to_owned(); let relations = crate::pptx::parse(&relation_xml)?;
    let mut groups: BTreeMap<&str,Vec<_>> = BTreeMap::new();
    let mut relationship_fragment = String::new(); let mut types_fragment = String::new();
    for font in additions {
        if !font.license_acknowledged { return Err(invalid("license acknowledgement required before new embedding")); }
        if !used(deck,&font.family)? { continue; }
        if existing.iter().any(|name|name.eq_ignore_ascii_case(&font.family)) { return Err(Error::Unsupported("adding styles to an existing native font family is not yet supported".into())); }
        let bytes = decode(&font.base64)?; let info = inspect(&bytes)?;
        let id = format!("rIdAISlideFont{}",info.sha256);
        let path = format!("ppt/fonts/aislide-{}.fntdata",info.sha256);
        if relations.root_element().children().any(|node|node.attribute("Id")==Some(id.as_str())) { return Err(Error::Conflict("font relationship already exists".into())); }
        package.add_part(path.clone(),to_eot(&bytes)?)?;
        relationship_fragment.push_str(&format!("<Relationship xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\" Id=\"{id}\" Type=\"{R}/font\" Target=\"/{path}\"/>"));
        types_fragment.push_str(&format!("<Override xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\" PartName=\"/{path}\" ContentType=\"application/x-fontdata\"/>"));
        groups.entry(&font.family).or_default().push((font.style,id));
    }
    if groups.is_empty() { return Ok(()); }
    let mut fragment = String::new();
    for (family,mut styles) in groups {
        styles.sort_by_key(|(style,_)|match style {FontStyle::Regular=>0,FontStyle::Bold=>1,FontStyle::Italic=>2,FontStyle::BoldItalic=>3});
        fragment.push_str(&format!("<p:embeddedFont xmlns:p=\"{P}\" xmlns:r=\"{R}\"><p:font typeface=\"{}\"/>",quick_xml::escape::escape(family)));
        for (style,id) in styles { fragment.push_str(&format!("<p:{} r:id=\"{id}\"/>",style_tag(style))); }
        fragment.push_str("</p:embeddedFont>");
    }
    let output = if let Some(list) = list { append_xml(&xml,list,&fragment)? } else {
        let following = parsed.root_element().children().find(|node|node.is_element() && node.tag_name().namespace()==Some(P) && ["custShowLst","photoAlbum","custDataLst","kinsoku","defaultTextStyle","modifyVerifier","extLst"].contains(&node.tag_name().name()));
        let wrapped = format!("<p:embeddedFontLst xmlns:p=\"{P}\">{fragment}</p:embeddedFontLst>");
        if let Some(following)=following { crate::native_save::apply(xml.clone(),vec![(following.range().start..following.range().start,wrapped)])? }
        else { append_xml(&xml,parsed.root_element(),&wrapped)? }
    };
    package.replace_part(main,output)?;
    let xml = package.text(main)?.to_owned(); let parsed = crate::pptx::parse(&xml)?;
    let mut flags = Vec::new();
    crate::native_save::set_attribute(&xml,parsed.root_element(),"embedTrueTypeFonts","1",&mut flags)?;
    crate::native_save::set_attribute(&xml,parsed.root_element(),"saveSubsetFonts","0",&mut flags)?;
    package.replace_part(main,crate::native_save::apply(xml,flags)?)?;
    package.replace_part(&relation_path,append_xml(&relation_xml,relations.root_element(),&relationship_fragment)?)?;
    let types = package.text("[Content_Types].xml")?.to_owned(); let parsed = crate::pptx::parse(&types)?;
    package.replace_part("[Content_Types].xml",append_xml(&types,parsed.root_element(),&types_fragment)?)?;
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FontStyle { Regular, Bold, Italic, BoldItalic }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FontPermission { Installable, Editable, PreviewPrint, Restricted, BitmapOnly, Unknown }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FontInfo {
    pub family: String,
    pub style: FontStyle,
    pub fs_type: Option<u16>,
    pub permission: FontPermission,
    pub no_subsetting: bool,
    pub byte_length: usize,
    pub sha256: String,
    pub format: String,
    pub usable: bool,
    pub license_verified: bool,
    pub copyright: Option<String>,
    pub license_description: Option<String>,
    pub license_url: Option<String>,
}

fn invalid(message: &str) -> Error { Error::Invalid(format!("font: {message}")) }

fn word(bytes: &[u8], offset: usize) -> Result<u16> {
    let slice = bytes.get(offset..offset + 2).ok_or_else(|| invalid("truncated field"))?;
    Ok(u16::from_be_bytes([slice[0], slice[1]]))
}

fn dword(bytes: &[u8], offset: usize) -> Result<usize> {
    let slice = bytes.get(offset..offset + 4).ok_or_else(|| invalid("truncated field"))?;
    Ok(u32::from_be_bytes([slice[0], slice[1], slice[2], slice[3]]) as usize)
}

fn tables(bytes: &[u8]) -> Result<BTreeMap<[u8; 4], &[u8]>> {
    if bytes.len() > MAX_FONT_BYTES { return Err(Error::Limit("font > 12 MiB".into())); }
    if !matches!(bytes.get(..4), Some(b"\0\x01\0\0" | b"OTTO")) {
        return Err(Error::Unsupported("only standalone TTF/OTF fonts; WOFF, collections and obfuscated fonts are unsupported".into()));
    }
    let count = word(bytes, 4)? as usize;
    if count == 0 || count > 128 { return Err(Error::Limit("font table count must be 1..128".into())); }
    let directory_end = 12 + count * 16;
    let mut result = BTreeMap::new();
    let mut ranges = Vec::new();
    let mut previous = None;
    for index in 0..count {
        let record = 12 + index * 16;
        let tag: [u8; 4] = bytes.get(record..record + 4).ok_or_else(|| invalid("truncated table directory"))?.try_into().map_err(|_| invalid("table tag"))?;
        let start = dword(bytes, record + 8)?;
        let length = dword(bytes, record + 12)?;
        let end = start.checked_add(length).ok_or_else(|| invalid("table range overflow"))?;
        if previous.is_some_and(|prior| prior >= tag) || start < directory_end || start % 4 != 0 || end > bytes.len() {
            return Err(invalid("unordered, duplicate or out-of-range table"));
        }
        if length > 0 { ranges.push(start..end); }
        result.insert(tag, &bytes[start..end]);
        previous = Some(tag);
    }
    ranges.sort_by_key(|range| range.start);
    if ranges.windows(2).any(|pair| pair[0].end > pair[1].start) { return Err(invalid("overlapping tables")); }
    Ok(result)
}

pub fn decode(base64: &str) -> Result<Vec<u8>> {
    if base64.len() > MAX_FONT_BYTES.div_ceil(3) * 4 { return Err(Error::Limit("font > 12 MiB".into())); }
    let bytes = STANDARD.decode(base64).map_err(|_| invalid("invalid base64"))?;
    if bytes.len() > MAX_FONT_BYTES { return Err(Error::Limit("font > 12 MiB".into())); }
    Ok(bytes)
}

pub fn inspect_base64(base64: &str) -> Result<FontInfo> { inspect(&decode(base64)?) }

pub fn inspect(bytes: &[u8]) -> Result<FontInfo> {
    let tables = tables(bytes)?;
    let name_data = tables.get(b"name").ok_or_else(|| invalid("missing name table"))?;
    if name_data.len() > 65536 || word(name_data,2)? > 256 { return Err(Error::Limit("font names > 64 KiB or 256 records".into())); }
    let names = ttf_parser::name::Table::parse(name_data).ok_or_else(|| invalid("invalid name table"))?;
    if names.names.into_iter().any(|name|name.name.len()>8192) { return Err(Error::Limit("font name > 8192 encoded bytes".into())); }
    let name = |id| -> Option<String> {
        names.names.into_iter().filter(|name| name.name_id == id)
            .filter_map(|name| name.to_string().map(|text| (name.language_id != 0x409, text)))
            .filter(|(_, text)| !text.trim().is_empty() && text.len() <= 4096 && !text.chars().any(char::is_control))
            .min_by_key(|(non_english, _)| *non_english).map(|(_, text)| text)
    };
    let family = name(1).ok_or_else(|| invalid("missing Unicode family name"))?;
    if family.len() > 128 { return Err(Error::Limit("font family > 128 bytes".into())); }
    let os2 = tables.get(b"OS/2").copied();
    let version = os2.and_then(|data| word(data, 0).ok());
    let minimum = match version { Some(0) => 78, Some(1) => 86, Some(2..=4) => 96, Some(5) => 100, _ => usize::MAX };
    let os2 = os2.filter(|data| data.len() >= minimum);
    let fs_type = os2.and_then(|data| word(data, 8).ok());
    let permission = match fs_type {
        Some(flags) if flags & 2 != 0 => FontPermission::Restricted,
        Some(flags) if flags & 0x200 != 0 => FontPermission::BitmapOnly,
        Some(flags) if flags & !0x10e != 0 => FontPermission::Unknown,
        Some(flags) => match flags & 0xf { 0 => FontPermission::Installable, 4 => FontPermission::PreviewPrint, 8 => FontPermission::Editable, _ => FontPermission::Unknown },
        None => FontPermission::Unknown,
    };
    let selection = os2.and_then(|data| word(data, 62).ok()).unwrap_or(0);
    let style = match (selection & 0x20 != 0, selection & 1 != 0) {
        (false, false) => FontStyle::Regular, (true, false) => FontStyle::Bold,
        (false, true) => FontStyle::Italic, (true, true) => FontStyle::BoldItalic,
    };
    let usable = name(6).is_some() && ttf_parser::Face::parse(bytes, 0).is_ok()
        && [b"cmap", b"head", b"hhea", b"hmtx", b"maxp", b"post"].iter().all(|tag| tables.contains_key(*tag))
        && (tables.contains_key(b"glyf") && tables.contains_key(b"loca") || tables.contains_key(b"CFF "))
        && [b"fvar", b"SVG ", b"COLR", b"CBDT", b"sbix"].iter().all(|tag| !tables.contains_key(*tag));
    Ok(FontInfo { family, style, fs_type, permission, no_subsetting: fs_type.is_some_and(|flags| flags & 0x100 != 0),
        byte_length: bytes.len(), sha256: format!("{:x}", Sha256::digest(bytes)),
        format: if bytes.starts_with(b"OTTO") { "otf" } else { "ttf" }.into(), usable, license_verified: false,
        copyright: name(0), license_description: name(13), license_url: name(14) })
}