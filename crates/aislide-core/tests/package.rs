use aislide_core::package::Package;
use std::io::{Cursor, Write};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

fn archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in entries {
        writer
            .start_file(*name, SimpleFileOptions::default().compression_method(CompressionMethod::Deflated))
            .unwrap();
        writer.write_all(data).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

#[test]
fn no_op_save_is_byte_identical() {
    let original = archive(&[("[Content_Types].xml", b"<Types/>"), ("ppt/slides/slide1.xml", b"<slide/>")]);
    let package = Package::open(original.clone()).unwrap();
    assert_eq!(package.save().unwrap(), original);
}

#[test]
fn changed_part_preserves_unknown_binary_parts() {
    let original = archive(&[("ppt/slides/slide1.xml", b"<slide/>"), ("custom/opaque.bin", &[0, 255, 19, 127])]);
    let mut package = Package::open(original).unwrap();
    package.replace_part("ppt/slides/slide1.xml", b"<slide changed='yes'/>".to_vec()).unwrap();
    let saved = Package::open(package.save().unwrap()).unwrap();
    assert_eq!(saved.parts()["custom/opaque.bin"], [0, 255, 19, 127]);
    assert_eq!(saved.parts()["ppt/slides/slide1.xml"], b"<slide changed='yes'/>");
}

#[test]
fn rejects_unsafe_part_names() {
    for name in ["../escape.xml", "/absolute.xml", "C:/escape.xml", "ppt/../escape.xml", "ppt\\escape.xml"] {
        assert!(Package::open(archive(&[(name, b"unsafe")])).is_err(), "accepted {name}");
    }
}

#[test]
fn rejects_invalid_archive() {
    assert!(Package::open(b"not a pptx".to_vec()).is_err());
}

#[test]
fn rejects_duplicate_zip_entries_before_the_library_coalesces_them() {
    let mut bytes = archive(&[("one.xml", b"first"), ("two.xml", b"second")]);
    for index in 0..bytes.len().saturating_sub(6) {
        if &bytes[index..index + 7] == b"two.xml" { bytes[index..index + 7].copy_from_slice(b"one.xml"); }
    }
    assert!(Package::open(bytes).is_err());
}

#[test]
fn rejects_unbounded_inflation() {
    let inflated = vec![0; 17 * 1024 * 1024];
    assert!(Package::open(archive(&[("large.bin", &inflated)])).is_err());
}