use std::process::Command;

fn resolve_git_commit() -> String {
    if let Ok(value) = std::env::var("AISLIDE_GIT_COMMIT") {
        let trimmed = value.trim();
        if !trimmed.is_empty() && trimmed.bytes().all(|b| b.is_ascii_hexdigit()) && (7..=40).contains(&trimmed.len()) {
            return trimmed.to_ascii_lowercase();
        }
        if trimmed == "unknown" {
            return "unknown".into();
        }
    }
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into());
    if let Ok(output) = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(&manifest_dir)
        .output()
    {
        if output.status.success() {
            if let Ok(stdout) = String::from_utf8(output.stdout) {
                let trimmed = stdout.trim();
                if (7..=40).contains(&trimmed.len()) && trimmed.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return trimmed.to_ascii_lowercase();
                }
            }
        }
    }
    "unknown".into()
}

fn main() {
    println!("cargo:rerun-if-env-changed=AISLIDE_GIT_COMMIT");
    println!("cargo:rustc-env=AISLIDE_GIT_COMMIT={}", resolve_git_commit());
}
