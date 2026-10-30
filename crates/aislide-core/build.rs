use std::{fs, path::{Path, PathBuf}, process::Command};

fn git_paths(root: &Path) -> Option<(PathBuf, PathBuf)> {
    let dot_git = root.join(".git");
    if dot_git.is_dir() { return Some((dot_git.clone(), dot_git)); }
    let text = fs::read_to_string(&dot_git).ok()?;
    let gitdir = text.strip_prefix("gitdir:")?.trim();
    let git_dir = root.join(gitdir);
    let common = fs::read_to_string(git_dir.join("commondir")).ok()
        .map_or_else(|| git_dir.clone(), |rel| git_dir.join(rel.trim()));
    Some((git_dir, common))
}

fn emit_git_rerun(manifest_dir: &Path) {
    let Some(repo_root) = manifest_dir.parent().and_then(Path::parent) else { return; };
    let Some((git_dir, common_dir)) = git_paths(repo_root) else { return; };
    let head = git_dir.join("HEAD");
    if head.exists() {
        println!("cargo:rerun-if-changed={}", head.display());
        if let Ok(content) = fs::read_to_string(&head) {
            if let Some(reference) = content.strip_prefix("ref:").map(str::trim) {
                let ref_path = common_dir.join(reference);
                if ref_path.exists() { println!("cargo:rerun-if-changed={}", ref_path.display()); }
            }
        }
    }
    let packed = common_dir.join("packed-refs");
    if packed.exists() { println!("cargo:rerun-if-changed={}", packed.display()); }
}

// Records the HEAD short SHA (or AISLIDE_GIT_COMMIT override); dirty working-tree state is not distinguished.
fn resolve_git_commit(manifest_dir: &Path) -> String {
    if let Ok(value) = std::env::var("AISLIDE_GIT_COMMIT") {
        let trimmed = value.trim();
        if (7..=40).contains(&trimmed.len()) && trimmed.bytes().all(|b| b.is_ascii_hexdigit()) {
            return trimmed.to_ascii_lowercase();
        }
        if trimmed == "unknown" { return "unknown".into(); }
    }
    if let Ok(output) = Command::new("git").args(["rev-parse", "--short", "HEAD"]).current_dir(manifest_dir).output() {
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
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into()));
    println!("cargo:rerun-if-env-changed=AISLIDE_GIT_COMMIT");
    emit_git_rerun(&manifest_dir);
    println!("cargo:rustc-env=AISLIDE_GIT_COMMIT={}", resolve_git_commit(&manifest_dir));
}
