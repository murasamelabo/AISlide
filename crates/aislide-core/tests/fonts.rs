use aislide_core::execute_request;
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};

fn metadata_font(flags: u16, selection: u16) -> Vec<u8> {
    let family: Vec<u8> = "AISlide Synthetic".encode_utf16().flat_map(u16::to_be_bytes).collect();
    let mut names = vec![0u8; 18];
    names[2..4].copy_from_slice(&1u16.to_be_bytes());
    names[4..6].copy_from_slice(&18u16.to_be_bytes());
    for (offset, value) in [(6, 3u16), (8, 1), (10, 0x409), (12, 1), (14, family.len() as u16)] {
        names[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
    }
    names.extend(family);
    let mut bytes = vec![0u8; 44 + 96];
    bytes[0..4].copy_from_slice(&0x10000u32.to_be_bytes());
    bytes[4..6].copy_from_slice(&2u16.to_be_bytes());
    bytes[12..16].copy_from_slice(b"OS/2");
    bytes[20..24].copy_from_slice(&44u32.to_be_bytes());
    bytes[24..28].copy_from_slice(&96u32.to_be_bytes());
    bytes[28..32].copy_from_slice(b"name");
    bytes[36..40].copy_from_slice(&140u32.to_be_bytes());
    bytes[40..44].copy_from_slice(&(names.len() as u32).to_be_bytes());
    bytes[44..46].copy_from_slice(&4u16.to_be_bytes());
    bytes[48..50].copy_from_slice(&400u16.to_be_bytes());
    bytes[52..54].copy_from_slice(&flags.to_be_bytes());
    bytes[106..108].copy_from_slice(&selection.to_be_bytes());
    bytes.extend(names);
    bytes
}

fn inspect(bytes: &[u8]) -> aislide_core::Result<Value> {
    execute_request(json!({"op":"inspect_font","base64":STANDARD.encode(bytes)}))
}

fn outline_font(flags: u16, selection: u16) -> Vec<u8> {
    let metadata = metadata_font(flags, selection);
    let mut tables = std::collections::BTreeMap::new();
    tables.insert(*b"OS/2", metadata[44..140].to_vec());
    let entries = [(1u16,"AISlide Synthetic"),(2,"Regular"),(4,"AISlide Synthetic Regular"),(5,"Version 1.0"),(6,"AISlideSynthetic-Regular"),(16,"AISlide Typographic")];
    let mut names = vec![0u8; 6+entries.len()*12];
    names[2..4].copy_from_slice(&(entries.len() as u16).to_be_bytes());
    let storage = names.len();
    names[4..6].copy_from_slice(&(storage as u16).to_be_bytes());
    for (index,(id,text)) in entries.into_iter().enumerate() {
        let encoded: Vec<_> = text.encode_utf16().flat_map(u16::to_be_bytes).collect();
        let offset = names.len()-storage;
        for (field,value) in [3u16,1,0x409,id,encoded.len() as u16,offset as u16].into_iter().enumerate() {
            let at = 6+index*12+field*2;
            names[at..at+2].copy_from_slice(&value.to_be_bytes());
        }
        names.extend(encoded);
    }
    tables.insert(*b"name", names);
    let mut head = vec![0u8; 54];
    head[..4].copy_from_slice(&0x10000u32.to_be_bytes());
    head[12..16].copy_from_slice(&0x5f0f3cf5u32.to_be_bytes());
    head[18..20].copy_from_slice(&1000u16.to_be_bytes());
    let mut hhea = vec![0u8; 36];
    hhea[..4].copy_from_slice(&0x10000u32.to_be_bytes());
    hhea[4..6].copy_from_slice(&800i16.to_be_bytes());
    hhea[6..8].copy_from_slice(&(-200i16).to_be_bytes());
    hhea[34..36].copy_from_slice(&2u16.to_be_bytes());
    let mut maxp = vec![0u8; 32];
    maxp[..4].copy_from_slice(&0x10000u32.to_be_bytes());
    maxp[4..6].copy_from_slice(&2u16.to_be_bytes());
    let mut cmap = vec![0,0,0,1,0,3,0,1,0,0,0,12,0,0,1,6,0,0];
    cmap.extend(vec![0u8; 256]);
    cmap[18 + 65] = 1;
    let mut glyph = vec![0,1,0,0,0,0,1,244,2,188,0,2,0,0,1,1,1];
    for value in [0i16,500,-250,0,0,700] { glyph.extend(value.to_be_bytes()); }
    glyph.push(0);
    let mut post = vec![0u8; 32];
    post[..4].copy_from_slice(&0x30000u32.to_be_bytes());
    tables.extend([(*b"head",head),(*b"hhea",hhea),(*b"hmtx",vec![2,88,0,0,2,88,0,0]),
        (*b"maxp",maxp),(*b"cmap",cmap),(*b"glyf",glyph),(*b"loca",vec![0,0,0,0,0,15]),(*b"post",post)]);
    let mut bytes = vec![0u8; 12 + tables.len() * 16];
    bytes[..4].copy_from_slice(&0x10000u32.to_be_bytes());
    bytes[4..6].copy_from_slice(&(tables.len() as u16).to_be_bytes());
    for (index, (tag, data)) in tables.into_iter().enumerate() {
        while bytes.len() % 4 != 0 { bytes.push(0); }
        let record = 12 + index * 16;
        bytes[record..record+4].copy_from_slice(&tag);
        let offset = bytes.len() as u32;
        bytes[record+8..record+12].copy_from_slice(&offset.to_be_bytes());
        bytes[record+12..record+16].copy_from_slice(&(data.len() as u32).to_be_bytes());
        bytes.extend(data);
    }
    bytes
}

fn font_document() -> Value {
    execute_request(json!({"op":"new_document","id":"font-test","deck":{
        "version":1,"title":"Fonts","width":1280,"height":720,"slides":[{
            "id":"slide-1","title":"Fonts","background":"FFFFFF","notes":"","elements":[{
                "type":"text","id":"text-1","x":40,"y":40,"width":400,"height":100,"text":"AAA",
                "font_size":30,"color":"000000","bold":false,"format":{"font_family":"AISlide Synthetic"}}]}]}})).unwrap()
}

#[test]
fn embedding_requires_consent_and_editable_full_font_and_is_undoable() {
    let document = font_document();
    for (flags, consent) in [(0,false),(2,true),(4,true),(0x200,true),(1,true),(12,true)] {
        let error = execute_request(json!({"op":"embed_font","document":document,"expected_revision":0,
            "base64":STANDARD.encode(outline_font(flags,0)),"license_acknowledged":consent})).unwrap_err();
        assert!(!error.to_string().contains("unknown variant"), "{error}");
    }
    let raw = outline_font(0x108,0);
    assert_eq!(inspect(&raw).unwrap()["usable"], true);
    let changed = execute_request(json!({"op":"embed_font","document":document,"expected_revision":0,
        "base64":STANDARD.encode(&raw),"license_acknowledged":true})).unwrap();
    assert_eq!(changed["document"]["deck"]["embedded_fonts"][0]["base64"], STANDARD.encode(&raw));
    let duplicate = execute_request(json!({"op":"embed_font","document":changed["document"],"expected_revision":1,
        "base64":STANDARD.encode(&raw),"license_acknowledged":true})).unwrap();
    assert_eq!(duplicate["document"]["revision"], 1);
    assert_eq!(duplicate["document"]["deck"]["embedded_fonts"].as_array().unwrap().len(),1);
    assert!(execute_request(json!({"op":"embed_font","document":changed["document"],"expected_revision":0,
        "base64":STANDARD.encode(&raw),"license_acknowledged":true})).is_err());
    let undone = execute_request(json!({"op":"undo_transaction","document":changed["document"],"expected_revision":1,"receipt":changed["receipt"]})).unwrap();
    assert_eq!(undone["document"]["hash"], document["hash"]);
}

#[test]
fn embedded_font_pptx_roundtrip_retains_exact_sfnt_and_non_target_parts() {
    let document = font_document();
    let raw = outline_font(0,0);
    let added = execute_request(json!({"op":"embed_font","document":document,"expected_revision":0,
        "base64":STANDARD.encode(&raw),"license_acknowledged":true})).unwrap();
    let exported = execute_request(json!({"op":"export","deck":added["document"]["deck"]})).unwrap();
    let bytes = STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap();
    let package = aislide_core::package::Package::open(bytes.clone()).unwrap();
    let presentation = roxmltree::Document::parse(package.text("ppt/presentation.xml").unwrap()).unwrap();
    assert_eq!(presentation.root_element().attribute("embedTrueTypeFonts"),Some("1"));
    assert_eq!(presentation.root_element().attribute("saveSubsetFonts"),Some("0"));
    let entry = presentation.descendants().find(|node| node.tag_name().name()=="embeddedFont").unwrap();
    assert_eq!(entry.children().find(|node|node.tag_name().name()=="font").unwrap().tag_name().namespace(), Some("http://schemas.openxmlformats.org/presentationml/2006/main"));
    assert!(entry.children().find(|node|node.tag_name().name()=="regular").unwrap().attribute(("http://schemas.openxmlformats.org/officeDocument/2006/relationships","id")).is_some());
    let font_part = package.parts().iter().find(|(name,_)|name.ends_with(".fntdata")).unwrap();
    assert!(font_part.1.ends_with(&raw));
    assert_eq!(&font_part.1[8..12], &0x10000u32.to_le_bytes());
    assert_eq!(&font_part.1[34..36], &0x504cu16.to_le_bytes());
    let opened = execute_request(json!({"op":"open_presentation","id":"reopen-font","base64":STANDARD.encode(&bytes)})).unwrap();
    assert_eq!(opened["document"]["deck"]["embedded_fonts"][0]["base64"],STANDARD.encode(&raw));
    let saved = execute_request(json!({"op":"export_presentation","document":opened["document"]})).unwrap();
    assert_eq!(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap(),bytes);
    let edited = execute_request(json!({"op":"transaction","document":opened["document"],"transaction":{
        "expected_revision":0,"expected_hash":opened["document"]["hash"],"operations":[{"op":"replace","path":"/deck/slides/0/elements/0/text","value":"AAAA"}]}})).unwrap();
    let output = execute_request(json!({"op":"export_presentation","document":edited["document"]})).unwrap();
    let output = aislide_core::package::Package::open(STANDARD.decode(output["base64"].as_str().unwrap()).unwrap()).unwrap();
    assert_eq!(output.part(font_part.0).unwrap(),font_part.1);
    assert_eq!(output.part("ppt/presentation.xml").unwrap(), package.part("ppt/presentation.xml").unwrap());
}

#[test]
fn font_inspection_reports_metadata_without_claiming_license_or_renderability() {
    let info = inspect(&metadata_font(0x108, 0x21)).unwrap();
    assert_eq!(info["family"], "AISlide Synthetic");
    assert_eq!(info["style"], "bold_italic");
    assert_eq!(info["fs_type"], 0x108);
    assert_eq!(info["permission"], "editable");
    assert_eq!(info["no_subsetting"], true);
    assert_eq!(info["license_verified"], false);
    assert_eq!(info["usable"], false);
    assert_eq!(info["sha256"].as_str().unwrap().len(), 64);
    assert!(info.get("path").is_none());
    for (flags, permission) in [(0, "installable"), (2, "restricted"), (4, "preview_print"),
        (8, "editable"), (0x200, "bitmap_only"), (1, "unknown"), (12, "unknown"), (0x4000, "unknown")] {
        assert_eq!(inspect(&metadata_font(flags, 0)).unwrap()["permission"], permission);
    }
}

#[test]
fn font_inspection_rejects_compressed_collections_and_invalid_table_ranges() {
    for signature in [b"wOFF", b"wOF2", b"ttcf"] {
        let mut bytes = metadata_font(0, 0);
        bytes[..4].copy_from_slice(signature);
        assert!(inspect(&bytes).is_err());
    }
    let mut bytes = metadata_font(0, 0);
    bytes[20..24].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(inspect(&bytes).is_err());
    assert!(inspect(&[]).is_err());
    assert!(inspect(&vec![0; 12 * 1024 * 1024 + 1]).is_err());
}

#[test]
fn malformed_names_and_ambiguous_tables_fail_before_large_allocations() {
    let mut bytes = metadata_font(0,0);
    let record = bytes[146..158].to_vec();
    let family = bytes[158..].to_vec();
    bytes.truncate(140);
    bytes.extend([0,0]); bytes.extend(300u16.to_be_bytes()); bytes.extend((6+300*12u16).to_be_bytes());
    for _ in 0..300 { bytes.extend(&record); }
    bytes.extend(family);
    let length = (bytes.len()-140) as u32;
    bytes[40..44].copy_from_slice(&length.to_be_bytes());
    assert!(inspect(&bytes).is_err(), "unbounded name records accepted");
    for (offset,value) in [(20,12u32),(24,u32::MAX),(36,44)] {
        let mut bytes = metadata_font(0,0); bytes[offset..offset+4].copy_from_slice(&value.to_be_bytes());
        assert!(inspect(&bytes).is_err());
    }
    let mut bytes = metadata_font(0,0); bytes[28..32].copy_from_slice(b"OS/2");
    assert!(inspect(&bytes).is_err());
    let mut bytes = metadata_font(0,0); bytes[44..46].copy_from_slice(&6u16.to_be_bytes());
    assert_eq!(inspect(&bytes).unwrap()["permission"],"unknown");
}

#[test]
fn four_style_slots_and_uncompressed_eot_permissions_are_exact() {
    let mut document = font_document();
    for (index,selection) in [0,0x20,1,0x21].into_iter().enumerate() {
        document = execute_request(json!({"op":"embed_font","document":document,"expected_revision":index,
            "base64":STANDARD.encode(outline_font(0x108,selection)),"license_acknowledged":true})).unwrap()["document"].clone();
    }
    let output = execute_request(json!({"op":"export","deck":document["deck"]})).unwrap();
    let package = aislide_core::package::Package::open(STANDARD.decode(output["base64"].as_str().unwrap()).unwrap()).unwrap();
    let parsed = roxmltree::Document::parse(package.text("ppt/presentation.xml").unwrap()).unwrap();
    let list: Vec<_> = parsed.descendants().filter(|node|node.tag_name().name()=="embeddedFont").collect();
    assert_eq!(list.len(),1);
    assert_eq!(list[0].children().filter(|node|node.is_element()).map(|node|node.tag_name().name()).collect::<Vec<_>>(),vec!["font","regular","bold","italic","boldItalic"]);
    for (name,data) in package.parts().iter().filter(|(name,_)|name.ends_with(".fntdata")) {
        let raw = aislide_core::fonts::from_eot(data).unwrap();
        assert_eq!(inspect(raw).unwrap()["fs_type"],0x108,"{name}");
        let mut tampered=data.clone(); tampered[32..34].copy_from_slice(&0u16.to_le_bytes());
        assert!(aislide_core::fonts::from_eot(&tampered).is_err());
        let mut compressed=data.clone(); compressed[12]=4;
        assert!(aislide_core::fonts::from_eot(&compressed).is_err());
    }
}

#[test]
fn document_font_rendering_is_local_and_missing_glyphs_are_reported() {
    let original = font_document();
    let added = execute_request(json!({"op":"embed_font","document":original,"expected_revision":0,
        "base64":STANDARD.encode(outline_font(0,0)),"license_acknowledged":true})).unwrap();
    let with_font = execute_request(json!({"op":"measure_layout","deck":added["document"]["deck"]})).unwrap();
    assert!(with_font["measurements"][0]["fonts"].as_array().unwrap().contains(&json!("AISlide Synthetic")));
    assert_eq!(with_font["measurements"][0]["missing_glyphs"],0);
    let clean = execute_request(json!({"op":"measure_layout","deck":original["deck"]})).unwrap();
    assert!(!clean["fonts"].as_array().unwrap().contains(&json!("AISlide Synthetic")));
    let mut deck = added["document"]["deck"].clone();
    deck["slides"][0]["elements"][0]["text"]=json!("\u{10ffff}");
    let missing = execute_request(json!({"op":"measure_layout","deck":deck})).unwrap();
    assert!(missing["issues"].as_array().unwrap().iter().any(|issue|issue["code"]=="MISSING_GLYPHS" || issue["code"]=="FONT_FALLBACK"));
}

#[test]
fn opaque_imported_fonts_and_protected_input_remain_untouched() {
    let document = font_document();
    let added = execute_request(json!({"op":"embed_font","document":document,"expected_revision":0,
        "base64":STANDARD.encode(outline_font(0,0)),"license_acknowledged":true})).unwrap();
    let output = execute_request(json!({"op":"export","deck":added["document"]["deck"]})).unwrap();
    let package = aislide_core::package::Package::open(STANDARD.decode(output["base64"].as_str().unwrap()).unwrap()).unwrap();
    let mut parts = package.parts().clone();
    let path = parts.keys().find(|name|name.ends_with(".fntdata")).unwrap().clone();
    parts.get_mut(&path).unwrap()[12]=4;
    parts.insert("ppt/unknown.bin".into(),vec![1,2,3,4]);
    let bytes = aislide_core::package::Package::from_parts(parts).unwrap().save().unwrap();
    let info = execute_request(json!({"op":"inspect_pptx_fonts","base64":STANDARD.encode(&bytes)})).unwrap();
    assert_eq!(info["fonts"][0]["status"],"opaque_preserved");
    let opened = execute_request(json!({"op":"open_presentation","id":"opaque","base64":STANDARD.encode(&bytes)})).unwrap();
    assert!(opened["document"]["deck"].get("embedded_fonts").is_none());
    assert_eq!(execute_request(json!({"op":"export_presentation","document":opened["document"]})).unwrap()["base64"],STANDARD.encode(bytes));
    let protected = [0xd0,0xcf,0x11,0xe0,0xa1,0xb1,0x1a,0xe1];
    let error = execute_request(json!({"op":"inspect_pptx_fonts","base64":STANDARD.encode(protected)})).unwrap_err();
    assert!(error.to_string().contains("does not remove protection"));
    assert_eq!(protected,[0xd0,0xcf,0x11,0xe0,0xa1,0xb1,0x1a,0xe1]);
}

#[test]
fn font_resources_respect_aggregate_limits_and_never_mutate_failed_documents() {
    let document = font_document();
    let mut raw = outline_font(0,0); raw.resize(9 * 1024 * 1024,0);
    let first = execute_request(json!({"op":"embed_font","document":document,"expected_revision":0,
        "base64":STANDARD.encode(raw),"license_acknowledged":true})).unwrap();
    let mut raw = outline_font(0,1); raw.resize(9 * 1024 * 1024,0);
    let second = execute_request(json!({"op":"embed_font","document":first["document"],"expected_revision":1,
        "base64":STANDARD.encode(raw),"license_acknowledged":true})).unwrap();
    let mut raw = outline_font(0,0x20); raw.resize(9 * 1024 * 1024,0);
    let error = execute_request(json!({"op":"embed_font","document":second["document"],"expected_revision":2,
        "base64":STANDARD.encode(raw),"license_acknowledged":true})).err().expect("third face must exceed combined budget");
    assert!(error.to_string().contains("combined fonts"),"{error}");
    assert_eq!(first["document"]["revision"],1);
    assert_eq!(second["document"]["revision"],2);
    let mut deck = document["deck"].clone();
    deck["embedded_fonts"] = json!(vec![json!({"family":"AISlide Synthetic","style":"regular","base64":STANDARD.encode(outline_font(0,0)),"license_acknowledged":true});9]);
    let error = execute_request(json!({"op":"validate","deck":deck})).unwrap_err();
    assert!(error.to_string().contains("8 embedded font faces"),"{error}");
    let mut deck = first["document"]["deck"].clone();
    deck["embedded_fonts"][0]["family"] = json!("Spoofed");
    assert!(execute_request(json!({"op":"validate","deck":deck})).is_err());
}

#[test]
fn external_and_missing_font_relationships_remain_inert_and_preserved() {
    let document = font_document();
    let added = execute_request(json!({"op":"embed_font","document":document,"expected_revision":0,
        "base64":STANDARD.encode(outline_font(0,0)),"license_acknowledged":true})).unwrap();
    let output = execute_request(json!({"op":"export","deck":added["document"]["deck"]})).unwrap();
    let package = aislide_core::package::Package::open(STANDARD.decode(output["base64"].as_str().unwrap()).unwrap()).unwrap();
    let relationship_xml=package.text("ppt/_rels/presentation.xml.rels").unwrap();
    let parsed=roxmltree::Document::parse(relationship_xml).unwrap();
    let relation=parsed.root_element().children().find(|node|node.attribute("Type").is_some_and(|kind|kind.ends_with("/font"))).unwrap();
    let id=relation.attribute("Id").unwrap();
    for target in ["TargetMode=\"External\" Target=\"https://example.invalid/font.fntdata\"", "Target=\"/ppt/fonts/missing.fntdata\""] {
        let mut parts=package.parts().clone();
        let mut xml=relationship_xml.to_owned();
        xml.replace_range(relation.range(),&format!("<Relationship xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\" Id=\"{id}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/font\" {target}/>"));
        parts.insert("ppt/_rels/presentation.xml.rels".into(),xml.into_bytes());
        let bytes=aislide_core::package::Package::from_parts(parts).unwrap().save().unwrap();
        let inspected=execute_request(json!({"op":"inspect_pptx_fonts","base64":STANDARD.encode(&bytes)})).unwrap();
        assert_eq!(inspected["fonts"][0]["status"],"opaque_preserved");
        let opened=execute_request(json!({"op":"open_presentation","id":"inert-font","base64":STANDARD.encode(&bytes)})).unwrap();
        assert_eq!(execute_request(json!({"op":"export_presentation","document":opened["document"]})).unwrap()["base64"],STANDARD.encode(bytes));
    }
}