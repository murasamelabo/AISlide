use aislide_core::execute_request;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[test]
fn architecture_icons_catalog_matches_exact_compiled_metadata() {
    let bytes = include_bytes!("../src/architecture-icons.json");
    assert_eq!(format!("{:x}", Sha256::digest(bytes)), "bd9db6956f49a686dd1fc6f66d0f60394e7144929ed72b91053efeee95b0a8a7");
    let manifest: Value = serde_json::from_slice(bytes).unwrap();
    let catalog = execute_request(json!({"op":"architecture_icons"})).unwrap();
    for field in ["version", "release", "providers", "icons"] {
        assert_eq!(catalog[field], manifest[field], "{field}");
    }
    assert!(catalog["configured"].is_boolean());
    assert!(!catalog["message"].as_str().unwrap().is_empty());
    let icons = catalog["icons"].as_array().unwrap();
    assert_eq!(icons.len(), 1498);
    for (provider, count) in [("azure", 645), ("aws", 808), ("gcp", 45)] {
        assert_eq!(icons.iter().filter(|icon| icon["provider"] == provider).count(), count);
    }
    let find = |id: &str| icons.iter().find(|icon| icon["id"] == id).unwrap();
    let entra = find("azure/entra/microsoft-entra-id");
    assert_eq!(entra["name"], "Microsoft Entra ID");
    assert!(entra["aliases"].as_array().unwrap().contains(&json!("Azure Active Directory")));
    let networking = find("gcp/category/networking");
    assert_eq!(networking["name"], "Google Cloud Networking");
    assert!(networking["aliases"].as_array().unwrap().contains(&json!("VPC")));
    assert!(networking["aliases"].as_array().unwrap().contains(&json!("Cloud Load Balancing")));
    let identity = find("gcp/category/security-identity");
    assert_eq!(identity["name"], "Google Cloud Security Identity");
    assert!(identity["aliases"].as_array().unwrap().contains(&json!("IAM")));
    for icon in icons {
        let mut keys: Vec<_> = icon.as_object().unwrap().keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(keys, ["aliases", "categories", "height", "id", "kind", "name", "png_bytes", "png_sha256", "provider", "source", "width"]);
        let source = icon["source"].as_object().unwrap();
        assert_eq!(source.len(), 3);
        assert!(source.contains_key("archive_id") && source.contains_key("entry") && source.contains_key("sha256"));
    }
}

#[test]
fn architecture_icons_requests_reject_paths_and_invalid_ids() {
    for request in [
        json!({"op":"architecture_icons", "root":"C:/other"}),
        json!({"op":"architecture_icon_assets", "ids":[], "directory":"C:/other"}),
        json!({"op":"architecture_icon_assets", "ids":[], "url":"https://example.invalid/icon.png"}),
    ] {
        assert!(execute_request(request).unwrap_err().to_string().contains("unknown field"));
    }
    for (ids, expected) in [
        (json!([]), "1..60"),
        (json!(["not-a-catalog-id"]), "unknown architecture icon ID"),
        (json!(["../consent.json"]), "unknown architecture icon ID"),
    ] {
        let error = execute_request(json!({"op":"architecture_icon_assets", "ids":ids})).unwrap_err();
        assert!(error.to_string().contains(expected), "{error}");
    }
    let manifest: Value = serde_json::from_slice(include_bytes!("../src/architecture-icons.json")).unwrap();
    let id = manifest["icons"][0]["id"].clone();
    let error = execute_request(json!({"op":"architecture_icon_assets", "ids":[id, id]})).unwrap_err();
    assert!(error.to_string().contains("duplicate"), "{error}");
    let error = execute_request(json!({"op":"architecture_icon_assets", "ids":vec![id; 61]})).unwrap_err();
    assert!(error.to_string().contains("1..60"), "{error}");
}

#[cfg(windows)]
#[test]
fn architecture_icons_unsafe_host_roots_keep_catalog_available() {
    if std::env::var_os("AISLIDE_ROOTGUARD_CHILD").is_some() {
        let catalog = execute_request(json!({"op":"architecture_icons"})).unwrap();
        assert_eq!(catalog["configured"], false);
        assert_eq!(catalog["icons"].as_array().unwrap().len(), 1498);
        assert!(catalog["message"].as_str().unwrap().contains("absolute local path"));
        let known = catalog["icons"][0]["id"].clone();
        let error = execute_request(json!({"op":"architecture_icon_assets", "ids":[known]})).unwrap_err();
        assert!(error.to_string().contains("absolute local path"), "{error}");
        let error = execute_request(json!({"op":"architecture_icon_assets", "ids":["unknown"]})).unwrap_err();
        assert!(error.to_string().contains("unknown architecture icon ID"), "{error}");
        return;
    }
    for (variable, root) in [
        ("AISLIDE_ICON_PACK_ROOT", "\\\\server.invalid\\share\\pack"),
        ("AISLIDE_ICON_PACK_ROOT", "\\\\?\\C:\\pack"),
        ("AISLIDE_ICON_PACK_ROOT", "\\\\.\\C:\\pack"),
        ("AISLIDE_ICON_PACK_ROOT", "Z:pack"),
        ("LOCALAPPDATA", "\\\\server.invalid\\share"),
        ("LOCALAPPDATA", "\\\\?\\C:\\Users"),
        ("LOCALAPPDATA", "Z:Users"),
    ] {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "architecture_icons_unsafe_host_roots_keep_catalog_available", "--nocapture"])
            .env("AISLIDE_ROOTGUARD_CHILD", "1")
            .env_remove("AISLIDE_ICON_PACK_ROOT")
            .env(variable, root)
            .output().unwrap();
        assert!(output.status.success(), "{variable}={root}: {}{}",
            String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    }
}