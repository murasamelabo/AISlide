use crate::{Error, Result, model::{Deck, valid_text, validate_deck}};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[path = "modern_comments.rs"]
pub mod modern;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Comment {
    pub id: String,
    pub author: String,
    pub initials: String,
    pub timestamp: String,
    pub text: String,
    #[serde(default)] pub x: i32,
    #[serde(default)] pub y: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub parent_id: Option<String>,
    #[serde(default)] pub resolved: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub native_author_id: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub native_index: Option<u32>,
}

pub fn validate_timestamp(value: &str) -> Result<()> {
    let invalid = || Error::Invalid("comment timestamp must be an explicitly supplied XML dateTime".into());
    let bytes = value.as_bytes();
    if bytes.len() < 19 || !value.is_ascii() || bytes[10] != b'T' || bytes[13] != b':' || bytes[16] != b':' { return Err(invalid()); }
    if [11, 12, 14, 15, 17, 18].iter().any(|index| !bytes[*index].is_ascii_digit()) { return Err(invalid()); }
    crate::fields::parse_date(&value[..10])?;
    let number = |part: &str| part.parse::<u32>().map_err(|_| invalid());
    if number(&value[11..13])? > 23 || number(&value[14..16])? > 59 || number(&value[17..19])? > 59 { return Err(invalid()); }
    let mut tail = &value[19..];
    if tail.starts_with('.') {
        let length = tail[1..].bytes().take_while(u8::is_ascii_digit).count();
        if length == 0 || length > 9 { return Err(invalid()); }
        tail = &tail[length + 1..];
    }
    if tail.is_empty() || tail == "Z" { return Ok(()); }
    if tail.len() != 6 || !tail.starts_with(['+', '-']) || tail.as_bytes()[3] != b':' { return Err(invalid()); }
    let hour = number(&tail[1..3])?; let minute = number(&tail[4..6])?;
    if hour > 14 || minute > 59 || (hour == 14 && minute != 0) { return Err(invalid()); }
    Ok(())
}

pub fn validate(comments: &[Comment]) -> Result<()> {
    if comments.len() > 512 { return Err(Error::Limit("comments per slide > 512".into())); }
    let mut ids = BTreeSet::new();
    let mut native_ids = BTreeSet::new();
    for comment in comments {
        for (value, limit) in [(&comment.id, 80), (&comment.author, 256), (&comment.initials, 64), (&comment.text, 8000)] { valid_text(value, limit)?; }
        if [&comment.id, &comment.author, &comment.initials].iter().any(|value| value.chars().any(char::is_control)) || comment.text.contains('\r') { return Err(Error::Invalid("comment attributes cannot contain controls; comment text uses LF".into())); }
        if comment.id.is_empty() || comment.author.trim().is_empty() || !ids.insert(&comment.id) { return Err(Error::Invalid("comment identity or local author missing/duplicated".into())); }
        validate_timestamp(&comment.timestamp)?;
        match (comment.native_author_id, comment.native_index) {
            (Some(author), Some(index)) if native_ids.insert((author, index)) => {},
            (None, None) => {},
            _ => return Err(Error::Invalid("incomplete or duplicate native comment identity".into())),
        }
    }
    for comment in comments {
        let mut visited = BTreeSet::from([comment.id.as_str()]);
        let mut parent = comment.parent_id.as_deref();
        while let Some(id) = parent {
            if !visited.insert(id) { return Err(Error::Invalid("comment reply cycle".into())); }
            parent = comments.iter().find(|comment| comment.id == id).ok_or_else(|| Error::Invalid("comment parent missing".into()))?.parent_id.as_deref();
        }
    }
    Ok(())
}

fn mutate(deck: &Deck, slide_id: &str, change: impl FnOnce(&mut Vec<Comment>) -> Result<()>) -> Result<Deck> {
    let mut next = deck.clone();
    let slide = next.slides.iter_mut().find(|slide| slide.id == slide_id).ok_or_else(|| Error::Invalid("comment slide missing".into()))?;
    change(&mut slide.review.get_or_insert_with(Default::default).comments)?;
    validate_deck(&next)?;
    Ok(next)
}

pub fn add(deck: &Deck, slide_id: &str, comment: Comment) -> Result<Deck> {
    if comment.parent_id.is_some() || comment.native_author_id.is_some() || comment.native_index.is_some() { return Err(Error::Invalid("new comments cannot supply parent/native identity".into())); }
    mutate(deck, slide_id, |comments| { comments.push(comment); Ok(()) })
}

pub fn reply(deck: &Deck, slide_id: &str, parent_id: &str, mut comment: Comment) -> Result<Deck> {
    if comment.native_author_id.is_some() || comment.native_index.is_some() { return Err(Error::Invalid("new replies cannot supply native identity".into())); }
    comment.parent_id = Some(parent_id.into());
    mutate(deck, slide_id, |comments| { comments.push(comment); Ok(()) })
}

pub fn resolve(deck: &Deck, slide_id: &str, comment_id: &str, resolved: bool) -> Result<Deck> {
    mutate(deck, slide_id, |comments| {
        comments.iter_mut().find(|comment| comment.id == comment_id).ok_or_else(|| Error::Invalid("comment missing".into()))?.resolved = resolved;
        Ok(())
    })
}

pub fn remove(deck: &Deck, slide_id: &str, comment_id: &str) -> Result<Deck> {
    mutate(deck, slide_id, |comments| {
        if !comments.iter().any(|comment| comment.id == comment_id) { return Err(Error::Invalid("comment missing".into())); }
        let mut removed = BTreeSet::from([comment_id.to_owned()]);
        loop {
            let old = removed.len();
            for comment in comments.iter() { if comment.parent_id.as_ref().is_some_and(|id| removed.contains(id)) { removed.insert(comment.id.clone()); } }
            if old == removed.len() { break; }
        }
        comments.retain(|comment| !removed.contains(&comment.id));
        Ok(())
    })
}

pub(crate) fn read(package: &crate::package::Package, path: &str) -> Result<Vec<Comment>> {
    use crate::native::{P, child};
    let targets = crate::review::targets(package, path, "comments")?;
    if targets.is_empty() { return Ok(Vec::new()); }
    if targets.len() != 1 { return Err(Error::Unsupported("multiple legacy comment parts on one slide".into())); }
    let main = crate::pptx::relationship_targets(package, "", "officeDocument")?.into_values().next().ok_or_else(|| Error::Invalid("presentation missing".into()))?;
    let authors = crate::review::targets(package, &main, "commentAuthors")?;
    if authors.len() != 1 { return Err(Error::Unsupported("legacy comment authors missing or ambiguous".into())); }
    let authors = crate::pptx::parse(package.text(authors.values().next().ok_or_else(|| Error::Invalid("author part missing".into()))?)?)?;
    let mut result = Vec::new();
    for path in targets.values() {
        let parsed = crate::pptx::parse(package.text(path)?)?;
        if !parsed.root_element().has_tag_name((P, "cmLst")) { return Err(Error::Unsupported("comment list namespace".into())); }
        for node in parsed.root_element().children().filter(|node| node.has_tag_name((P, "cm"))) {
            let author_id = node.attribute("authorId").ok_or_else(|| Error::Invalid("comment author ID missing".into()))?;
            let index = node.attribute("idx").ok_or_else(|| Error::Invalid("comment index missing".into()))?;
            let author = authors.root_element().children().find(|node| node.has_tag_name((P, "cmAuthor")) && node.attribute("id") == Some(author_id)).ok_or_else(|| Error::Invalid("comment author not found".into()))?;
            let state = child(node, P, "extLst").into_iter().flat_map(|list| list.children()).filter(|extension| extension.has_tag_name((P, "ext"))).flat_map(|extension| extension.children()).find(|entry| entry.has_tag_name((crate::review::NS, "comment")));
            let position = child(node, P, "pos");
            let coordinate = |name| position.and_then(|node| node.attribute(name)).unwrap_or("0").parse::<i32>().map_err(|_| Error::Invalid("comment coordinate".into()));
            result.push(Comment {
                id: state.and_then(|node| node.attribute("id")).map(str::to_owned).unwrap_or_else(|| format!("native-{author_id}-{index}")),
                author: author.attribute("name").unwrap_or("").into(), initials: author.attribute("initials").unwrap_or("").into(),
                timestamp: node.attribute("dt").unwrap_or("").into(), text: child(node, P, "text").map(|node| node.children().filter(|child| child.is_text()).filter_map(|child| child.text()).collect::<String>()).unwrap_or_default(),
                x: coordinate("x")?, y: coordinate("y")?, parent_id: state.and_then(|node| node.attribute("parent")).map(str::to_owned),
                resolved: state.is_some_and(|node| node.attribute("resolved") == Some("1")),
                native_author_id: Some(author_id.parse().map_err(|_| Error::Invalid("comment author ID".into()))?),
                native_index: Some(index.parse().map_err(|_| Error::Invalid("comment index".into()))?),
            });
        }
    }
    validate(&result)?;
    Ok(result)
}

fn author_for(package: &mut crate::package::Package, comment: &Comment, allocate_index: bool) -> Result<(u32, u32)> {
    use crate::native::P;
    let main = crate::pptx::relationship_targets(package, "", "officeDocument")?.into_values().next().ok_or_else(|| Error::Invalid("presentation missing".into()))?;
    let targets = crate::review::targets(package, &main, "commentAuthors")?;
    if targets.len() > 1 { return Err(Error::Unsupported("multiple legacy author parts".into())); }
    let path = if let Some(path) = targets.values().next() { path.clone() } else {
        let path = crate::review::unused_path(package, "ppt/aislide-commentAuthors");
        crate::review::put_part(package, &path, format!("<p:cmAuthorLst xmlns:p=\"{P}\"/>").into_bytes())?;
        crate::review::add_type(package, &path, "application/vnd.openxmlformats-officedocument.presentationml.commentAuthors+xml")?;
        crate::review::link(package, &main, "commentAuthors", &path)?;
        path
    };
    let xml = package.text(&path)?.to_owned(); let parsed = crate::pptx::parse(&xml)?;
    let matches_author = |node: &roxmltree::Node<'_, '_>| node.has_tag_name((P, "cmAuthor")) && node.attribute("name") == Some(&comment.author) && node.attribute("initials") == Some(&comment.initials);
    let author = parsed.root_element().children().filter(matches_author).find(|node| comment.native_author_id.is_some_and(|id| node.attribute("id") == Some(id.to_string().as_str())))
        .or_else(|| parsed.root_element().children().find(matches_author));
    let id = match author {
        Some(node) => node.attribute("id").unwrap_or("").parse::<u32>().map_err(|_| Error::Invalid("native author identity".into()))?,
        None => parsed.root_element().children().filter_map(|node| node.attribute("id")).map(|value| value.parse::<u32>().map_err(|_| Error::Invalid("native author identity".into()))).collect::<Result<Vec<_>>>()?.into_iter().max().map(|value| value.checked_add(1).ok_or_else(|| Error::Limit("comment author IDs exhausted".into()))).transpose()?.unwrap_or(0),
    };
    if !allocate_index && comment.native_author_id == Some(id) { return Ok((id, comment.native_index.ok_or_else(|| Error::Invalid("native comment index missing".into()))?)); }
    let mut maximum = author.and_then(|node| node.attribute("lastIdx")).unwrap_or("0").parse::<u32>().map_err(|_| Error::Invalid("native author last index".into()))?;
    let types = crate::pptx::parse(package.text("[Content_Types].xml")?)?;
    for entry in types.root_element().children().filter(|node| node.attribute("ContentType") == Some("application/vnd.openxmlformats-officedocument.presentationml.comments+xml")) {
        if let Some(path) = entry.attribute("PartName").map(|path| path.trim_start_matches('/')).filter(|path| package.part_names().contains(*path)) {
            let document = crate::pptx::parse(package.text(path)?)?;
            for node in document.root_element().children().filter(|node| node.has_tag_name((P, "cm")) && node.attribute("authorId") == Some(id.to_string().as_str())) {
                maximum = maximum.max(node.attribute("idx").unwrap_or("").parse().map_err(|_| Error::Invalid("native comment index".into()))?);
            }
        }
    }
    let index = maximum.checked_add(1).ok_or_else(|| Error::Limit("comment indices exhausted".into()))?;
    let mut edits = Vec::new();
    if let Some(author) = author { crate::native_save::set_attribute(&xml, author, "lastIdx", &index.to_string(), &mut edits)?; }
    else { crate::native_save::insert_child(&xml, parsed.root_element(), &format!("<p:cmAuthor xmlns:p=\"{P}\" id=\"{id}\" name=\"{}\" initials=\"{}\" lastIdx=\"{index}\" clrIdx=\"{id}\"/>", quick_xml::escape::escape(&comment.author), quick_xml::escape::escape(&comment.initials)), None, &mut edits)?; }
    package.replace_part(&path, crate::native_save::apply(xml, edits)?)?;
    Ok((id, index))
}

fn state_xml(comment: &Comment) -> String {
    let parent = comment.parent_id.as_ref().map(|id| format!(" parent=\"{}\"", quick_xml::escape::escape(id))).unwrap_or_default();
    format!("<p:ext xmlns:p=\"{}\" uri=\"{}\"><rv:comment xmlns:rv=\"{}\" id=\"{}\"{parent} resolved=\"{}\"/></p:ext>", crate::native::P, crate::review::NS, crate::review::NS, quick_xml::escape::escape(&comment.id), if comment.resolved { 1 } else { 0 })
}

pub(crate) fn write(package: &mut crate::package::Package, path: &str, before: &[Comment], after: &[Comment], copied: bool) -> Result<()> {
    use crate::native::{P, child};
    use crate::native_save::{apply, insert_child, set_attribute};
    if before == after && !copied { return Ok(()); }
    crate::review::ensure_unprotected(package)?;
    validate(after)?;
    let targets = crate::review::targets(package, path, "comments")?;
    if targets.len() > 1 { return Err(Error::Unsupported("multiple comment parts".into())); }
    let comment_path = if let Some(target) = targets.values().next() { target.clone() } else {
        if after.is_empty() { return Ok(()); }
        let target = crate::review::unused_path(package, "ppt/comments/aislide-comments");
        crate::review::put_part(package, &target, format!("<p:cmLst xmlns:p=\"{P}\"/>").into_bytes())?;
        crate::review::add_type(package, &target, "application/vnd.openxmlformats-officedocument.presentationml.comments+xml")?;
        crate::review::link(package, path, "comments", &target)?;
        target
    };
    let owners = crate::pptx::slide_paths(package)?.iter().map(|slide| crate::review::targets(package, slide, "comments")).collect::<Result<Vec<_>>>()?.iter().filter(|targets| targets.values().any(|target| target == &comment_path)).count();
    if owners > 1 { return Err(Error::Unsupported("shared comment part must be separated before editing".into())); }
    let xml = package.text(&comment_path)?.to_owned(); let parsed = crate::pptx::parse(&xml)?; let mut edits = Vec::new();
    let mut processed = BTreeSet::new();
    for node in parsed.root_element().children().filter(|node| node.has_tag_name((P, "cm"))) {
        let author = node.attribute("authorId").and_then(|value| value.parse::<u32>().ok());
        let index = node.attribute("idx").and_then(|value| value.parse::<u32>().ok());
        let old = before.iter().find(|comment| comment.native_author_id == author && comment.native_index == index);
        let Some(old) = old else { continue };
        processed.insert(old.id.clone());
        let Some(next) = after.iter().find(|comment| comment.id == old.id) else { edits.push((node.range(), String::new())); continue };
        if old.native_author_id != next.native_author_id || old.native_index != next.native_index { return Err(Error::Invalid("native comment identity cannot be reassigned".into())); }
        if old == next && !copied { continue; }
        let (author, index) = author_for(package, next, copied || old.author != next.author || old.initials != next.initials)?;
        set_attribute(&xml, node, "authorId", &author.to_string(), &mut edits)?;
        set_attribute(&xml, node, "idx", &index.to_string(), &mut edits)?;
        set_attribute(&xml, node, "dt", &next.timestamp, &mut edits)?;
        if old.text != next.text {
            let text = child(node, P, "text").ok_or_else(|| Error::Unsupported("comment text missing".into()))?;
            if text.children().any(|child| child.is_element()) { return Err(Error::Unsupported("comment text contains unmodeled markup".into())); }
            edits.push((text.range(), format!("<p:text xmlns:p=\"{P}\">{}</p:text>", quick_xml::escape::escape(&next.text))));
        }
        if let Some(position) = child(node, P, "pos") { set_attribute(&xml, position, "x", &next.x.to_string(), &mut edits)?; set_attribute(&xml, position, "y", &next.y.to_string(), &mut edits)?; }
        if let Some(list) = child(node, P, "extLst") {
            if let Some(owned) = list.children().find(|node| node.has_tag_name((P, "ext")) && node.attribute("uri") == Some(crate::review::NS)) { edits.push((owned.range(), state_xml(next))); }
            else { insert_child(&xml, list, &state_xml(next), None, &mut edits)?; }
        } else { insert_child(&xml, node, &format!("<p:extLst xmlns:p=\"{P}\">{}</p:extLst>", state_xml(next)), None, &mut edits)?; }
    }
    let mut added = String::new();
    for comment in after.iter().filter(|comment| !processed.contains(&comment.id)) {
        let (author, index) = author_for(package, comment, true)?;
        added.push_str(&format!("<p:cm xmlns:p=\"{P}\" authorId=\"{author}\" dt=\"{}\" idx=\"{index}\"><p:pos x=\"{}\" y=\"{}\"/><p:text>{}</p:text><p:extLst>{}</p:extLst></p:cm>", quick_xml::escape::escape(&comment.timestamp), comment.x, comment.y, quick_xml::escape::escape(&comment.text), state_xml(comment)));
    }
    if !added.is_empty() { insert_child(&xml, parsed.root_element(), &added, None, &mut edits)?; }
    if !edits.is_empty() { package.replace_part(&comment_path, apply(xml, edits)?)?; }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn local_comment_lifecycle_is_pure_and_validated() {
        let deck: Deck = serde_json::from_value(json!({"version":1,"title":"Synthetic","width":1280,"height":720,"slides":[{"id":"s","title":"Test","background":"FFFFFF","notes":"","elements":[]}]})).unwrap();
        let comment: Comment = serde_json::from_value(json!({"id":"c","author":"Local reviewer","initials":"LR","timestamp":"2026-09-17T10:00:00+09:00","text":"@Other is plain local text"})).unwrap();
        let next = add(&deck, "s", comment.clone()).unwrap();
        assert!(deck.slides[0].review.is_none());
        assert!(add(&next, "s", comment.clone()).is_err());
        let next = reply(&next, "s", "c", Comment { id: "reply".into(), ..comment }).unwrap();
        let next = resolve(&next, "s", "c", true).unwrap();
        assert!(next.slides[0].review.as_ref().unwrap().comments[0].resolved);
        assert!(remove(&next, "s", "c").unwrap().slides[0].review.as_ref().unwrap().comments.is_empty());
        assert!(validate_timestamp("2026-02-30T10:00:00Z").is_err());
        assert!(validate_timestamp("2026-09-17T10:00:00+14:01").is_err());
    }
}