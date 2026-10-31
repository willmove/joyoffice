use serde_json::{Value, json};
use std::{path::Path, process::Command};
pub fn collect(root: &Path, destination: &Path) -> Result<(), String> {
    let host = Command::new("rustc").arg("-vV").output().map_err(|e| e.to_string())?;
    let host = String::from_utf8_lossy(&host.stdout);
    let triple = host.lines().find_map(|l| l.strip_prefix("host: ")).ok_or("Missing host triple")?;
    let output = Command::new("cargo")
        .args(["metadata", "--locked", "--format-version", "1", "--filter-platform", triple, "--features", "joysheet/automation"])
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into());
    }
    let metadata: Value = serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
    let mut records = Vec::new();
    let mut notice = String::from(
        "# Third-party dependencies\n\nResolved host graph, including build/test dependencies; an overinclusive inventory. License expressions are upstream declarations, not a legal conclusion. Texts include packaged files and attributed source supplements. SOURCE.json records supplement provenance; DECLARATION.md identifies standard-text fallbacks where a published crate omitted its original text.\n\n",
    );
    for p in metadata["packages"].as_array().into_iter().flatten() {
        let name = p["name"].as_str().ok_or("Missing package name")?;
        let version = p["version"].as_str().unwrap_or("");
        let manifest = Path::new(p["manifest_path"].as_str().ok_or("Missing manifest")?);
        let directory = manifest.parent().ok_or("Invalid manifest")?;
        let mut texts = Vec::new();
        find(directory, 0, &mut texts)?;
        if let Some(file) = p["license_file"].as_str() {
            let path = directory.join(file);
            if path.is_file() && !texts.contains(&path) {
                texts.push(path);
            }
        }
        let packaged_texts = texts.len();
        if directory.starts_with(root) && p["source"].is_null() {
            for name in ["LICENSE-MIT", "LICENSE-APACHE", "NOTICE"] {
                texts.push(root.join(name));
            }
        }
        let supplement = root.join("docs/third-party").join(name);
        if supplement.is_dir() {
            find(&supplement, 0, &mut texts)?;
            for entry in std::fs::read_dir(&supplement).map_err(|e| e.to_string())? {
                let path = entry.map_err(|e| e.to_string())?.path();
                if path.is_file() && !texts.contains(&path) {
                    texts.push(path);
                }
            }
        }
        if texts.is_empty() {
            return Err(format!("Missing license text for {name} {version}; add an attributed supplement"));
        }
        let out = destination.join(format!("{name}-{version}"));
        std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
        let mut files = Vec::new();
        for (index, file) in texts.iter().enumerate() {
            let file_name = format!("{index}-{}", file.file_name().and_then(|f| f.to_str()).unwrap_or("LICENSE"));
            std::fs::copy(file, out.join(&file_name)).map_err(|e| e.to_string())?;
            files.push(file_name);
        }
        notice.push_str(&format!(
            "- {name} {version}: {} · {}\n",
            p["license"].as_str().unwrap_or("see license_file"),
            p["repository"].as_str().unwrap_or("workspace")
        ));
        records.push(json!({"name":name,"version":version,"license":p["license"],"repository":p["repository"],"source":p["source"],"packagedTextCount":packaged_texts,"supplement":supplement.is_dir(),"texts":files}));
    }
    std::fs::write(destination.join("THIRD-PARTY-NOTICES.md"), notice).map_err(|e| e.to_string())?;
    std::fs::write(
        destination.join("inventory.json"),
        serde_json::to_vec_pretty(&json!({"host":triple,"packages":records})).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
fn find(path: &Path, depth: usize, out: &mut Vec<std::path::PathBuf>) -> Result<(), String> {
    if depth > 3 {
        return Ok(());
    }
    for entry in std::fs::read_dir(path).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();
        if path.is_dir() {
            if ![".git", "target", "node_modules", "test", "tests"].contains(&name.as_str()) {
                find(&path, depth + 1, out)?;
            }
        } else if ["license", "licence", "copying", "notice", "ofl", "copyright"].iter().any(|v| name.starts_with(v)) {
            out.push(path);
        }
    }
    Ok(())
}
