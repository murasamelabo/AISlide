use crate::{Error, Result, package::Package, rich_text::RichParagraph};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const NS: &str = "http://schemas.microsoft.com/office/powerpoint/2018/8/main";
pub(crate) const REL: &str = "http://schemas.microsoft.com/office/2018/10/relationships";
pub(crate) const MIME: &str = "application/vnd.ms-powerpoint.comments+xml";
pub(crate) const AUTHORS_MIME: &str = "application/vnd.ms-powerpoint.authors+xml";
pub(crate) const EXT: &str = "{6950BFC3-D8DA-4A85-94F7-54DA5524770B}";
const PC: &str = "http://schemas.microsoft.com/office/powerpoint/2013/main/command";
const AC: &str = "http://schemas.microsoft.com/office/drawing/2013/main/command";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status { #[default] Active, Resolved, Closed }

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Author {
    pub id: String,
    pub name: String,
    pub user_id: String,
    pub provider_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub initials: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Anchor {
    Unknown,
    Slide { slide_id: u32, slide_creation_id: u32 },
    Shape { slide_id: u32, slide_creation_id: u32, shape_id: u32, shape_creation_id: String },
    TextRange { slide_id: u32, shape_id: u32, start: u32, end: u32 },
    Preserved,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModernReply {
    pub id: String,
    pub author: Author,
    pub created: String,
    #[serde(default)] pub status: Status,
    #[serde(default)] pub body: Vec<RichParagraph>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModernThread {
    pub id: String,
    pub author: Author,
    pub created: String,
    #[serde(default)] pub status: Status,
    pub anchor: Anchor,
    #[serde(default)] pub replies: Vec<ModernReply>,
    #[serde(default)] pub body: Vec<RichParagraph>,
}

fn invalid() -> Error { Error::Invalid("invalid modern comment structure or identity".into()) }

fn guid(value: &str) -> Result<String> {
    let raw = value.strip_prefix('{').and_then(|value| value.strip_suffix('}')).ok_or_else(invalid)?;
    if raw.len() != 36 || !raw.bytes().enumerate().all(|(index, byte)| if [8, 13, 18, 23].contains(&index) { byte == b'-' } else { byte.is_ascii_hexdigit() }) { return Err(invalid()); }
    Ok(raw.to_ascii_uppercase())
}

fn validate_author(author: &Author) -> Result<()> {
    guid(&author.id)?;
    for value in [&author.name, &author.user_id, &author.provider_id] {
        crate::model::valid_text(value, 256)?;
        if value.is_empty() || value.chars().any(char::is_control) { return Err(invalid()); }
    }
    if let Some(initials) = &author.initials { crate::model::valid_text(initials, 64)?; if initials.chars().any(char::is_control) { return Err(invalid()); } }
    Ok(())
}

pub fn validate(threads: &[ModernThread]) -> Result<()> {
    if threads.len() > 512 { return Err(Error::Limit("modern comment budget".into())); }
    let mut ids = BTreeSet::new();
    let mut authors = BTreeMap::new();
    let mut count = 0;
    for thread in threads {
        for (id, author, created, body) in std::iter::once((&thread.id, &thread.author, &thread.created, &thread.body)).chain(thread.replies.iter().map(|reply| (&reply.id, &reply.author, &reply.created, &reply.body))) {
            count += 1;
            if count > 1024 { return Err(Error::Limit("modern comment budget".into())); }
            if !ids.insert(guid(id)?) { return Err(invalid()); }
            validate_author(author)?;
            if authors.insert(guid(&author.id)?, author).is_some_and(|old| old != author) { return Err(invalid()); }
            crate::comments::validate_timestamp(created)?;
            crate::rich_text::validate_paragraphs_with_limit(body, 8000)?;
        }
    }
    Ok(())
}

pub(crate) fn validate_deck(deck: &crate::model::Deck) -> Result<()> {
    let mut ids = BTreeSet::new();
    let mut authors = BTreeMap::new();
    for threads in deck.slides.iter().filter_map(|slide| slide.review.as_ref().and_then(|review| review.modern_threads.as_deref())) {
        validate(threads)?;
        for (id, author) in threads.iter().flat_map(|thread| std::iter::once((&thread.id, &thread.author)).chain(thread.replies.iter().map(|reply| (&reply.id, &reply.author)))) {
            if !ids.insert(guid(id)?) { return Err(invalid()); }
            if authors.insert(guid(&author.id)?, author).is_some_and(|previous| previous != author) { return Err(invalid()); }
        }
    }
    Ok(())
}

pub(crate) fn validate_transition(before: &crate::model::Deck, after: &crate::model::Deck) -> Result<()> {
    validate_deck(after)?;
    let mut entries = BTreeMap::new();
    let mut known_authors = BTreeMap::new();
    let mut anchors = BTreeMap::new();
    for slide in &before.slides {
        for thread in slide.review.as_ref().and_then(|review| review.modern_threads.as_ref()).into_iter().flatten() {
            anchors.insert(guid(&thread.id)?, &thread.anchor);
            for (id, author, created, parent) in std::iter::once((&thread.id, &thread.author, &thread.created, None)).chain(thread.replies.iter().map(|reply| (&reply.id, &reply.author, &reply.created, Some(thread.id.as_str())))) {
                known_authors.insert(guid(&author.id)?, author);
                entries.insert(guid(id)?, (author, created, slide.id.as_str(), parent));
            }
        }
    }
    for slide in &after.slides {
        for thread in slide.review.as_ref().and_then(|review| review.modern_threads.as_ref()).into_iter().flatten() {
            match anchors.get(&guid(&thread.id)?) {
                Some(previous) if **previous != thread.anchor => return Err(Error::Invalid("existing modern anchor is immutable".into())),
                None if thread.anchor != Anchor::Unknown => return Err(Error::Unsupported("new modern anchors require explicit unknown".into())),
                _ => {},
            }
            for (id, author, created, parent) in std::iter::once((&thread.id, &thread.author, &thread.created, None)).chain(thread.replies.iter().map(|reply| (&reply.id, &reply.author, &reply.created, Some(thread.id.as_str())))) {
                if let Some(previous) = entries.get(&guid(id)?) {
                    if *previous != (author, created, slide.id.as_str(), parent) { return Err(Error::Invalid("existing modern identity metadata is immutable".into())); }
                }
                if let Some(previous) = known_authors.get(&guid(&author.id)?) {
                    if **previous != *author { return Err(Error::Invalid("existing modern author is immutable".into())); }
                } else if author.provider_id != "None" || author.user_id != author.name { return Err(Error::Invalid("new authors must be explicitly local offline identities".into())); }
            }
        }
    }
    Ok(())
}

fn validate_package_ids(package: &Package) -> Result<()> {
    let types = crate::pptx::parse(package.text("[Content_Types].xml")?)?;
    let mut seen = BTreeSet::new();
    for entry in types.root_element().children().filter(|node| node.has_tag_name((crate::review::CT, "Override")) && node.attribute("ContentType") == Some(MIME)) {
        let path = required(entry, "PartName")?;
        let parsed = crate::pptx::parse(package.text(path.trim_start_matches('/'))?)?;
        if !parsed.root_element().has_tag_name((NS, "cmLst")) { return Err(invalid()); }
        for node in parsed.descendants().filter(|node| node.has_tag_name((NS, "cm")) || node.has_tag_name((NS, "reply"))) {
            if !seen.insert(guid(&required(node, "id")?)?) { return Err(invalid()); }
        }
    }
    Ok(())
}

pub(crate) fn targets(package: &Package, source: &str, kind: &str) -> Result<BTreeMap<String, String>> {
    let path = crate::native::relations_path(source);
    if !package.part_names().contains(path.as_str()) { return Ok(BTreeMap::new()); }
    let parsed = crate::pptx::parse(package.text(&path)?)?;
    if !parsed.root_element().has_tag_name((crate::native::REL, "Relationships")) { return Err(invalid()); }
    let mut seen = BTreeSet::new();
    let mut result = BTreeMap::new();
    for node in parsed.root_element().children().filter(|node| node.is_element()) {
        if !node.has_tag_name((crate::native::REL, "Relationship")) { return Err(invalid()); }
        let id = required(node, "Id")?;
        if !seen.insert(id.clone()) { return Err(invalid()); }
        if node.attribute("Type") != Some(format!("{REL}/{kind}").as_str()) { continue; }
        if node.attribute("TargetMode").is_some_and(|mode| mode != "Internal") { return Err(Error::Unsupported("external modern comment reference".into())); }
        let target = crate::pptx::resolve(source, &required(node, "Target")?)?;
        package.part(&target)?;
        result.insert(id, target);
    }
    Ok(result)
}

fn required(node: roxmltree::Node<'_, '_>, name: &str) -> Result<String> { node.attribute(name).map(str::to_owned).ok_or_else(invalid) }

fn check_type(package: &Package, path: &str, mime: &str) -> Result<()> {
    let parsed = crate::pptx::parse(package.text("[Content_Types].xml")?)?;
    if !parsed.root_element().has_tag_name((crate::review::CT, "Types")) { return Err(invalid()); }
    let entries: Vec<_> = parsed.root_element().children().filter(|node| node.has_tag_name((crate::review::CT, "Override")) && node.attribute("PartName") == Some(format!("/{path}").as_str())).collect();
    if entries.len() != 1 || entries[0].attribute("ContentType") != Some(mime) { return Err(invalid()); }
    Ok(())
}

fn main_part(package: &Package) -> Result<String> {
    let roots = crate::pptx::relationship_targets(package, "", "officeDocument")?;
    if roots.len() != 1 { return Err(invalid()); }
    roots.into_values().next().ok_or_else(invalid)
}

fn authors(package: &Package) -> Result<BTreeMap<String, Author>> {
    let targets = targets(package, &main_part(package)?, "authors")?;
    if targets.len() != 1 { return Err(invalid()); }
    let path = targets.values().next().ok_or_else(invalid)?;
    check_type(package, path, AUTHORS_MIME)?;
    let parsed = crate::pptx::parse(package.text(path)?)?;
    if !parsed.root_element().has_tag_name((NS, "authorLst")) { return Err(invalid()); }
    let mut result = BTreeMap::new();
    for node in parsed.root_element().children().filter(|node| node.is_element()) {
        if result.len() >= 2048 { return Err(Error::Limit("modern authors budget".into())); }
        if !node.has_tag_name((NS, "author")) { return Err(invalid()); }
        let author = Author { id: required(node, "id")?, name: required(node, "name")?, user_id: required(node, "userId")?, provider_id: required(node, "providerId")?, initials: node.attribute("initials").map(str::to_owned) };
        validate_author(&author)?;
        if result.insert(guid(&author.id)?, author).is_some() { return Err(invalid()); }
    }
    Ok(result)
}

fn read_entry(node: roxmltree::Node<'_, '_>, authors: &BTreeMap<String, Author>) -> Result<ModernReply> {
    let author = authors.get(&guid(&required(node, "authorId")?)?).ok_or_else(invalid)?.clone();
    let status = match node.attribute("status").unwrap_or("active") { "active" => Status::Active, "resolved" => Status::Resolved, "closed" => Status::Closed, _ => return Err(invalid()) };
    let body = crate::native::child(node, NS, "txBody").map(|body| crate::native::read_rich_paragraphs_with_limit(body, &Default::default(), &Default::default(), 8000)).transpose()?.unwrap_or_default();
    Ok(ModernReply { id: required(node, "id")?, author, created: required(node, "created")?, status, body })
}

fn read_anchor(node: roxmltree::Node<'_, '_>) -> Anchor {
    fn slide(node: roxmltree::Node<'_, '_>) -> Option<(u32, u32)> {
        if !node.has_tag_name((PC, "sldMkLst")) || node.attributes().len() != 0 { return None; }
        let children: Vec<_> = node.children().filter(|node| node.is_element()).collect();
        if children.len() != 2 || !children[0].has_tag_name((PC, "docMk")) || children[0].attributes().len() != 0 || children[0].children().any(|node| node.is_element()) || !children[1].has_tag_name((PC, "sldMk")) || children[1].children().any(|node| node.is_element()) { return None; }
        let moniker = children[1];
        if moniker.attributes().any(|attribute| attribute.namespace().is_some() || !["sldId", "cId"].contains(&attribute.name())) { return None; }
        Some((moniker.attribute("sldId")?.parse().ok()?, moniker.attribute("cId")?.parse().ok()?))
    }
    if node.has_tag_name((NS, "unknownAnchor")) && node.attributes().len() == 0 && !node.children().any(|node| node.is_element()) { return Anchor::Unknown; }
    if let Some((slide_id, slide_creation_id)) = slide(node) { return Anchor::Slide { slide_id, slide_creation_id }; }
    if node.has_tag_name((AC, "deMkLst")) && node.attributes().len() == 0 {
        let children: Vec<_> = node.children().filter(|node| node.is_element()).collect();
        if children.len() == 2 {
            if let Some((slide_id, slide_creation_id)) = slide(children[0]) {
                let shape = children[1];
                if shape.has_tag_name((AC, "spMk")) && !shape.children().any(|node| node.is_element()) && shape.attributes().all(|attribute| attribute.namespace().is_none() && ["id", "creationId"].contains(&attribute.name())) {
                    if let (Some(shape_id), Some(creation)) = (shape.attribute("id").and_then(|value| value.parse().ok()), shape.attribute("creationId")) {
                        if guid(creation).is_ok() { return Anchor::Shape { slide_id, slide_creation_id, shape_id, shape_creation_id: creation.into() }; }
                    }
                }
            }
        }
    }
    Anchor::Preserved
}

fn check_children(node: roxmltree::Node<'_, '_>, reply: bool) -> Result<Anchor> {
    let mut last = 0;
    let mut seen = BTreeSet::new();
    let mut anchor = None;
    let mut anchor_kind = None;
    for child in node.children().filter(|node| node.is_element()) {
        let order = if !reply && (child.has_tag_name((NS, "unknownAnchor")) || child.has_tag_name((PC, "sldMkLst")) || child.has_tag_name((AC, "deMkLst")) || child.has_tag_name((AC, "txMkLst"))) {
            if anchor.is_some() {
                if anchor_kind != Some(child.tag_name().name()) || !["deMkLst", "txMkLst"].contains(&child.tag_name().name()) || last != 1 { return Err(invalid()); }
                anchor = Some(Anchor::Preserved);
            } else { anchor = Some(read_anchor(child)); anchor_kind = Some(child.tag_name().name()); }
            1
        } else if !reply && child.has_tag_name((NS, "pos")) { 2 }
        else if !reply && child.has_tag_name((NS, "replyLst")) { 3 }
        else if child.has_tag_name((NS, "txBody")) { 4 }
        else if child.has_tag_name((NS, "extLst")) { 5 }
        else { return Err(invalid()); };
        if order < last || (!seen.insert(order) && order != 1) { return Err(invalid()); }
        last = order;
    }
    if !reply && anchor.is_none() { return Err(invalid()); }
    Ok(anchor.unwrap_or(Anchor::Unknown))
}

pub(crate) fn read(package: &Package, slide_path: &str) -> Result<Option<Vec<ModernThread>>> {
    let targets = targets(package, slide_path, "comments")?;
    let slide = crate::pptx::parse(package.text(slide_path)?)?;
    let extensions: Vec<_> = crate::native::child(slide.root_element(), crate::native::P, "extLst").into_iter().flat_map(|node| node.children()).filter(|node| node.has_tag_name((crate::native::P, "ext")) && node.attribute("uri") == Some(EXT)).collect();
    if targets.is_empty() && extensions.is_empty() { return Ok(None); }
    if targets.len() != 1 || extensions.len() != 1 { return Err(invalid()); }
    let children: Vec<_> = extensions[0].children().filter(|node| node.is_element()).collect();
    if children.len() != 1 || !children[0].has_tag_name((NS, "commentRel")) { return Err(invalid()); }
    let id = children[0].attribute((crate::native::R, "id")).ok_or_else(invalid)?;
    let path = targets.get(id).ok_or_else(invalid)?;
    check_type(package, path, MIME)?;
    validate_package_ids(package)?;
    let authors = authors(package)?;
    let parsed = crate::pptx::parse(package.text(path)?)?;
    if !parsed.root_element().has_tag_name((NS, "cmLst")) { return Err(invalid()); }
    let mut threads = Vec::new();
    for node in parsed.root_element().children().filter(|node| node.is_element()) {
        if !node.has_tag_name((NS, "cm")) { return Err(invalid()); }
        let anchor = check_children(node, false)?;
        let entry = read_entry(node, &authors)?;
        let mut replies = Vec::new();
        if let Some(list) = crate::native::child(node, NS, "replyLst") {
            for reply in list.children().filter(|node| node.is_element()) {
                if !reply.has_tag_name((NS, "reply")) { return Err(invalid()); }
                check_children(reply, true)?;
                replies.push(read_entry(reply, &authors)?);
            }
        }
        threads.push(ModernThread { id: entry.id, author: entry.author, created: entry.created, status: entry.status, body: entry.body, anchor, replies });
    }
    validate(&threads)?;
    Ok(Some(threads))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Draft {
    pub author_name: String,
    #[serde(default)] pub initials: Option<String>,
    pub created: String,
    pub body: Vec<RichParagraph>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    Create { draft: Draft, anchor: Anchor },
    Reply { thread_id: String, draft: Draft },
    SetStatus { comment_id: String, status: Status },
    UpdateBody { comment_id: String, body: Vec<RichParagraph> },
    Remove { comment_id: String },
}

pub fn apply(deck: &crate::model::Deck, slide_id: &str, operation: Operation) -> Result<crate::model::Deck> {
    use sha2::{Digest, Sha256};
    let seed = serde_json::to_vec(&(deck, slide_id, &operation))?;
    let new_id = |label: &str| {
        let mut hash = Sha256::new(); hash.update(&seed); hash.update(label.as_bytes());
        let mut bytes = hash.finalize(); bytes[6] = (bytes[6] & 15) | 128; bytes[8] = (bytes[8] & 63) | 128;
        let value: String = bytes[..16].iter().map(|byte| format!("{byte:02X}")).collect();
        format!("{{{}-{}-{}-{}-{}}}", &value[..8], &value[8..12], &value[12..16], &value[16..20], &value[20..])
    };
    let entry = |draft: Draft| ModernReply { id: new_id("comment"), author: Author { id: new_id("author"), user_id: draft.author_name.clone(), name: draft.author_name, provider_id: "None".into(), initials: draft.initials }, created: draft.created, status: Status::Active, body: draft.body };
    let mut next = deck.clone();
    let slide = next.slides.iter_mut().find(|slide| slide.id == slide_id).ok_or_else(invalid)?;
    let threads = slide.review.get_or_insert_with(Default::default).modern_threads.get_or_insert_with(Vec::new);
    match operation {
        Operation::Create { draft, anchor } => {
            if anchor != Anchor::Unknown { return Err(Error::Unsupported("new modern comments currently require an explicit unknown anchor".into())); }
            let value = entry(draft);
            threads.push(ModernThread { id: value.id, author: value.author, created: value.created, status: value.status, anchor, replies: Vec::new(), body: value.body });
        }
        Operation::Reply { thread_id, draft } => threads.iter_mut().find(|thread| thread.id == thread_id).ok_or_else(invalid)?.replies.push(entry(draft)),
        Operation::SetStatus { comment_id, status } => {
            if let Some(thread) = threads.iter_mut().find(|thread| thread.id == comment_id) { thread.status = status; }
            else { threads.iter_mut().flat_map(|thread| &mut thread.replies).find(|reply| reply.id == comment_id).ok_or_else(invalid)?.status = status; }
        }
        Operation::UpdateBody { comment_id, body } => {
            if let Some(thread) = threads.iter_mut().find(|thread| thread.id == comment_id) { thread.body = body; }
            else { threads.iter_mut().flat_map(|thread| &mut thread.replies).find(|reply| reply.id == comment_id).ok_or_else(invalid)?.body = body; }
        }
        Operation::Remove { comment_id } => {
            if threads.iter().any(|thread| thread.id == comment_id) { threads.retain(|thread| thread.id != comment_id); }
            else {
                let thread = threads.iter_mut().find(|thread| thread.replies.iter().any(|reply| reply.id == comment_id)).ok_or_else(invalid)?;
                thread.replies.retain(|reply| reply.id != comment_id);
            }
        }
    }
    validate(threads)?;
    crate::model::validate_deck(&next)?;
    validate_deck(&next)?;
    Ok(next)
}

fn escape(value: &str) -> String { quick_xml::escape::escape(value).into_owned() }

fn link(package: &mut Package, source: &str, kind: &str, target: &str) -> Result<String> {
    let path = crate::native::relations_path(source);
    let xml = if package.part_names().contains(path.as_str()) { package.text(&path)?.to_owned() } else { format!("<Relationships xmlns=\"{}\"/>", crate::native::REL) };
    let parsed = crate::pptx::parse(&xml)?;
    let mut index = 1;
    let id = loop { let id = format!("rIdModernReview{index}"); if !parsed.root_element().children().any(|node| node.attribute("Id") == Some(&id)) { break id; } index += 1; };
    let fragment = format!("<Relationship xmlns=\"{}\" Id=\"{id}\" Type=\"{REL}/{kind}\" Target=\"/{}\"/>", crate::native::REL, escape(target));
    let mut edits = Vec::new();
    crate::native_save::insert_child(&xml, parsed.root_element(), &fragment, None, &mut edits)?;
    crate::review::put_part(package, &path, crate::native_save::apply(xml, edits)?)?;
    Ok(id)
}

fn body_xml(body: &[RichParagraph]) -> Result<String> {
    if body.is_empty() { return Ok(String::new()); }
    if body.iter().flat_map(|paragraph| &paragraph.runs).any(|run| run.field.is_some()) { return Err(Error::Unsupported("comment fields are preserved, not authored".into())); }
    let element: crate::model::Element = serde_json::from_value(serde_json::json!({"type":"text","id":"modern-body","x":0,"y":0,"width":100,"height":100,"font_size":18,"color":"000000","bold":false,"text":crate::rich_text::plain_text(body),"format":{"paragraphs":body}}))?;
    let xml = crate::pptx::element_xml(&element, 2, &BTreeMap::new(), &Default::default());
    let parsed = crate::pptx::parse(&xml)?;
    let node = parsed.descendants().find(|node| node.has_tag_name((crate::native::P, "txBody"))).ok_or_else(invalid)?;
    let children: String = node.children().filter(|node| node.is_element()).map(|node| crate::native_save::fragment(&xml, node)).collect();
    Ok(format!("<m:txBody xmlns:m=\"{NS}\" xmlns:a=\"{}\">{children}</m:txBody>", crate::native::A))
}

fn status_text(status: Status) -> &'static str { match status { Status::Active => "active", Status::Resolved => "resolved", Status::Closed => "closed" } }

fn properties(id: &str, author: &Author, created: &str, status: Status) -> String {
    format!("id=\"{}\" authorId=\"{}\" created=\"{}\" status=\"{}\"", escape(id), escape(&author.id), escape(created), status_text(status))
}

fn reply_xml(reply: &ModernReply) -> Result<String> {
    Ok(format!("<m:reply xmlns:m=\"{NS}\" {}>{}</m:reply>", properties(&reply.id, &reply.author, &reply.created, reply.status), body_xml(&reply.body)?))
}

fn thread_xml(thread: &ModernThread) -> Result<String> {
    if thread.anchor != Anchor::Unknown { return Err(Error::Unsupported("unrepresentable new modern comment anchor".into())); }
    let replies = thread.replies.iter().map(reply_xml).collect::<Result<Vec<_>>>()?.join("");
    let replies = if replies.is_empty() { replies } else { format!("<m:replyLst>{replies}</m:replyLst>") };
    Ok(format!("<m:cm xmlns:m=\"{NS}\" {}><m:unknownAnchor/>{replies}{}</m:cm>", properties(&thread.id, &thread.author, &thread.created, thread.status), body_xml(&thread.body)?))
}

fn editable_body(body: roxmltree::Node<'_, '_>) -> bool {
    body.descendants().filter(|node| node.is_element()).all(|node| {
        if node == body { return node.attributes().len() == 0; }
        if node.tag_name().namespace() != Some(crate::native::A) { return false; }
        if matches!(node.tag_name().name(), "rPr" | "defRPr" | "endParaRPr") {
            let latin = crate::native::child(node, crate::native::A, "latin").and_then(|node| node.attribute("typeface"));
            for (tag, major, minor) in [("ea", "+mj-ea", "+mn-ea"), ("cs", "+mj-cs", "+mn-cs")] {
                if let Some(family) = crate::native::child(node, crate::native::A, tag).and_then(|node| node.attribute("typeface")) {
                    if Some(family) != latin && !matches!((latin, family), (Some("+mj-lt"), value) if value == major) && !matches!((latin, family), (Some("+mn-lt"), value) if value == minor) { return false; }
                }
            }
        }
        let allowed: &[&str] = match node.tag_name().name() {
            "bodyPr" => &["lIns", "rIns", "tIns", "bIns", "wrap", "anchor"],
            "pPr" => &["algn", "lvl", "marL", "indent"],
            "rPr" | "defRPr" | "endParaRPr" => &["lang", "sz", "b", "i", "u", "dirty", "baseline"],
            "srgbClr" | "schemeClr" | "spcPct" | "spcPts" => &["val"],
            "latin" | "ea" | "cs" => &["typeface"],
            "t" => &["space"], "tab" => &["pos", "algn"], "buChar" => &["char"], "buAutoNum" => &["type", "startAt"],
            "p" | "r" | "lstStyle" | "noAutofit" | "solidFill" | "highlight" | "buNone" | "lnSpc" | "spcBef" | "spcAft" | "tabLst" => &[],
            _ => return false,
        };
        node.attributes().all(|attribute| allowed.contains(&attribute.name()) && (attribute.namespace().is_none() || (node.tag_name().name() == "t" && attribute.name() == "space" && attribute.namespace() == Some("http://www.w3.org/XML/1998/namespace"))))
    })
}

fn patch_entry(xml: &str, node: roxmltree::Node<'_, '_>, old: (&str, &Author, &str, Status, &[RichParagraph]), new: (&str, &Author, &str, Status, &[RichParagraph]), edits: &mut Vec<(std::ops::Range<usize>, String)>) -> Result<()> {
    if old.0 != new.0 || old.1 != new.1 || old.2 != new.2 { return Err(Error::Invalid("existing modern identity and author metadata are immutable".into())); }
    if old.3 != new.3 { crate::native_save::set_attribute(xml, node, "status", status_text(new.3), edits)?; }
    if old.4 != new.4 {
        let replacement = body_xml(new.4)?;
        if let Some(body) = crate::native::child(node, NS, "txBody") {
            if !editable_body(body) { return Err(Error::Unsupported("unmodeled modern rich body is preserved; body edit rejected".into())); }
            if replacement.is_empty() { edits.push((body.range(), replacement)); }
            else {
                let generated = crate::pptx::parse(&replacement)?;
                let previous: Vec<_> = body.children().filter(|node| node.has_tag_name((crate::native::A, "p"))).collect();
                let next: Vec<_> = generated.root_element().children().filter(|node| node.has_tag_name((crate::native::A, "p"))).collect();
                for (index, paragraph) in previous.iter().enumerate() {
                    let fragment = if let Some(next) = next.get(index) {
                        let mut fragment = replacement[next.range()].to_owned();
                        if let Some(end) = crate::native::child(*paragraph, crate::native::A, "endParaRPr") {
                            let parsed = crate::pptx::parse(&fragment)?;
                            if let Some(generated_end) = crate::native::child(parsed.root_element(), crate::native::A, "endParaRPr") {
                                let range = generated_end.range(); fragment.replace_range(range, &crate::native_design::isolated_row(xml, end)?);
                            }
                        }
                        fragment
                    } else { String::new() };
                    edits.push((paragraph.range(), fragment));
                }
                if next.len() > previous.len() {
                    let added: String = next[previous.len()..].iter().map(|node| &replacement[node.range()]).collect();
                    crate::native_save::insert_child(xml, body, &added, None, edits)?;
                }
            }
        } else if !replacement.is_empty() { crate::native_save::insert_child(xml, node, &replacement, crate::native::child(node, NS, "extLst"), edits)?; }
    }
    Ok(())
}

fn write_authors(package: &mut Package, after: &[ModernThread]) -> Result<()> {
    let main = main_part(package)?;
    let current = targets(package, &main, "authors")?;
    let known = if current.is_empty() { BTreeMap::new() } else { authors(package)? };
    let mut added = BTreeMap::new();
    for author in after.iter().flat_map(|thread| std::iter::once(&thread.author).chain(thread.replies.iter().map(|reply| &reply.author))) {
        let id = guid(&author.id)?;
        if let Some(old) = known.get(&id) { if old != author { return Err(invalid()); } }
        else {
            if author.provider_id != "None" || author.user_id != author.name { return Err(Error::Invalid("new authors must be explicitly local offline identities".into())); }
            added.insert(id, author);
        }
    }
    if added.is_empty() { return Ok(()); }
    let path = if let Some(path) = current.values().next() { path.clone() } else {
        let path = crate::review::unused_path(package, "ppt/authors/aislide-authors");
        crate::review::put_part(package, &path, format!("<m:authorLst xmlns:m=\"{NS}\"/>").into_bytes())?;
        crate::review::add_type(package, &path, AUTHORS_MIME)?;
        link(package, &main, "authors", &path)?;
        path
    };
    let fragment: String = added.values().map(|author| {
        let initials = author.initials.as_ref().map(|value| format!(" initials=\"{}\"", escape(value))).unwrap_or_default();
        format!("<m:author xmlns:m=\"{NS}\" id=\"{}\" name=\"{}\" userId=\"{}\" providerId=\"None\"{initials}/>", escape(&author.id), escape(&author.name), escape(&author.user_id))
    }).collect();
    let xml = package.text(&path)?.to_owned(); let parsed = crate::pptx::parse(&xml)?; let mut edits = Vec::new();
    crate::native_save::insert_child(&xml, parsed.root_element(), &fragment, None, &mut edits)?;
    package.replace_part(&path, crate::native_save::apply(xml, edits)?)
}

pub(crate) fn write(package: &mut Package, slide_path: &str, before: Option<&[ModernThread]>, after: Option<&[ModernThread]>, copied: bool) -> Result<()> {
    if copied && before.is_some_and(|threads| !threads.is_empty()) { return Err(Error::Unsupported("copying modern comment anchors requires explicit remapping".into())); }
    let Some(after) = after else { return Ok(()); };
    if before == Some(after) && !copied { return Ok(()); }
    if before.is_none() && after.is_empty() { return Ok(()); }
    crate::review::ensure_unprotected(package)?;
    for path in targets(package, slide_path, "comments")?.into_values().chain(targets(package, &main_part(package)?, "authors")?.into_values()) {
        let relations = crate::native::relations_path(&path);
        if !package.part_names().contains(relations.as_str()) { continue; }
        let parsed = crate::pptx::parse(package.text(&relations)?)?;
        if !parsed.root_element().has_tag_name((crate::native::REL, "Relationships")) { return Err(invalid()); }
        let mut ids = BTreeSet::new();
        for node in parsed.root_element().children().filter(|node| node.is_element()) {
            if !node.has_tag_name((crate::native::REL, "Relationship")) || !ids.insert(required(node, "Id")?) { return Err(invalid()); }
            if node.attribute("TargetMode").is_some_and(|mode| mode != "Internal") { return Err(Error::Unsupported("modern comment edits reject external references".into())); }
            let target = crate::pptx::resolve(&path, &required(node, "Target")?)?;
            package.part(&target)?;
        }
    }
    validate(after)?;
    let before = before.unwrap_or(&[]);
    let append_order = |previous: Vec<&str>, next: Vec<&str>| {
        let retained: Vec<_> = previous.into_iter().filter(|id| next.contains(id)).collect();
        next.starts_with(&retained)
    };
    if !append_order(before.iter().map(|thread| thread.id.as_str()).collect(), after.iter().map(|thread| thread.id.as_str()).collect()) {
        return Err(Error::Unsupported("reordering native modern comment threads is not supported".into()));
    }
    for old in before {
        if let Some(new) = after.iter().find(|thread| thread.id == old.id) {
            if !append_order(old.replies.iter().map(|reply| reply.id.as_str()).collect(), new.replies.iter().map(|reply| reply.id.as_str()).collect()) {
                return Err(Error::Unsupported("reordering native modern comment replies is not supported".into()));
            }
        }
    }
    let current = read(package, slide_path)?;
    if current.as_deref().unwrap_or(&[]) != before { return Err(Error::Conflict("modern comment origin mismatch".into())); }
    let targets = targets(package, slide_path, "comments")?;
    let path = if let Some(path) = targets.values().next() {
        let owners = crate::pptx::slide_paths(package)?.iter().map(|slide| self::targets(package, slide, "comments")).collect::<Result<Vec<_>>>()?.iter().filter(|targets| targets.values().any(|target| target == path)).count();
        if owners != 1 { return Err(Error::Unsupported("shared modern comment part".into())); }
        path.clone()
    } else {
        let path = crate::review::unused_path(package, "ppt/comments/aislide-modern");
        crate::review::put_part(package, &path, format!("<m:cmLst xmlns:m=\"{NS}\"/>").into_bytes())?;
        crate::review::add_type(package, &path, MIME)?;
        let id = link(package, slide_path, "comments", &path)?;
        let xml = package.text(slide_path)?.to_owned(); let parsed = crate::pptx::parse(&xml)?; let mut edits = Vec::new();
        let extension = format!("<p:ext xmlns:p=\"{}\" uri=\"{EXT}\"><m:commentRel xmlns:m=\"{NS}\" xmlns:r=\"{}\" r:id=\"{id}\"/></p:ext>", crate::native::P, crate::native::R);
        if let Some(list) = crate::native::child(parsed.root_element(), crate::native::P, "extLst") { crate::native_save::insert_child(&xml, list, &extension, None, &mut edits)?; }
        else { crate::native_save::insert_child(&xml, parsed.root_element(), &format!("<p:extLst xmlns:p=\"{}\">{extension}</p:extLst>", crate::native::P), None, &mut edits)?; }
        package.replace_part(slide_path, crate::native_save::apply(xml, edits)?)?;
        path
    };
    write_authors(package, after)?;
    let xml = package.text(&path)?.to_owned(); let parsed = crate::pptx::parse(&xml)?; let mut edits = Vec::new();
    for old in before {
        let node = parsed.root_element().children().find(|node| node.has_tag_name((NS, "cm")) && node.attribute("id") == Some(&old.id)).ok_or_else(invalid)?;
        let Some(new) = after.iter().find(|thread| thread.id == old.id) else { edits.push((node.range(), String::new())); continue; };
        if old.anchor != new.anchor { return Err(Error::Unsupported("existing modern anchor is immutable".into())); }
        patch_entry(&xml, node, (&old.id, &old.author, &old.created, old.status, &old.body), (&new.id, &new.author, &new.created, new.status, &new.body), &mut edits)?;
        if old.replies == new.replies { continue; }
        let list = crate::native::child(node, NS, "replyLst");
        for previous in &old.replies {
            let reply = list.and_then(|list| list.children().find(|node| node.has_tag_name((NS, "reply")) && node.attribute("id") == Some(&previous.id))).ok_or_else(invalid)?;
            let Some(next) = new.replies.iter().find(|reply| reply.id == previous.id) else { edits.push((reply.range(), String::new())); continue; };
            patch_entry(&xml, reply, (&previous.id, &previous.author, &previous.created, previous.status, &previous.body), (&next.id, &next.author, &next.created, next.status, &next.body), &mut edits)?;
        }
        let added = new.replies.iter().filter(|reply| !old.replies.iter().any(|old| old.id == reply.id)).map(reply_xml).collect::<Result<Vec<_>>>()?.join("");
        if !added.is_empty() {
            if let Some(list) = list { crate::native_save::insert_child(&xml, list, &added, None, &mut edits)?; }
            else { crate::native_save::insert_child(&xml, node, &format!("<m:replyLst xmlns:m=\"{NS}\">{added}</m:replyLst>"), crate::native::child(node, NS, "txBody").or_else(|| crate::native::child(node, NS, "extLst")), &mut edits)?; }
        }
    }
    let added = after.iter().filter(|thread| !before.iter().any(|old| old.id == thread.id)).map(thread_xml).collect::<Result<Vec<_>>>()?.join("");
    if !added.is_empty() { crate::native_save::insert_child(&xml, parsed.root_element(), &added, None, &mut edits)?; }
    if !edits.is_empty() { package.replace_part(&path, crate::native_save::apply(xml, edits)?)?; }
    Ok(())
}