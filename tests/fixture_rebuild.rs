use std::fs;
use std::path::{Path, PathBuf};
use assert_cmd::prelude::*;
use std::process::Command;
use tempfile::tempdir;

fn recreate_t99_into(dest: &Path) {
    // Create dirs and empty files from manifest
    let manifest = include_str!("fixtures/T99_tree.txt");
    for line in manifest.lines() {
        if line.is_empty() { continue; }
        let mut parts = line.splitn(2, '\t');
        let kind = parts.next().unwrap();
        let rel_raw = parts.next().unwrap_or("");
        // Normalize path: drop leading ./ and any ParentDir/CurDir components
        let mut norm = PathBuf::new();
        for comp in Path::new(rel_raw).components() {
            use std::path::Component::*;
            match comp {
                RootDir | Prefix(_) => continue,
                CurDir => continue,
                ParentDir => continue,
                Normal(seg) => norm.push(seg),
            }
        }
        if norm.as_os_str().is_empty() { continue; }
        let target = dest.join(&norm);
        match kind {
            "D" => { fs::create_dir_all(&target).unwrap(); }
            "F" => {
                if let Some(p) = target.parent() { fs::create_dir_all(p).unwrap(); }
                fs::OpenOptions::new().create(true).write(true).truncate(true).open(&target).unwrap();
            }
            _ => {}
        }
    }

    // Write the content script to a temp file and execute it with DEST
    let contents = include_str!("fixtures/T99_contents.sh");
    let tmp = tempfile::Builder::new().prefix("apply_contents_").suffix(".sh").tempfile().unwrap();
    let script_path: PathBuf = tmp.path().to_path_buf();
    fs::write(&script_path, contents).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&script_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms).unwrap();
    }
    let status = Command::new(&script_path)
        .arg(dest)
        .status().unwrap();
    assert!(status.success(), "apply contents script failed");
}

#[test]
fn rebuild_fixture_and_scan() {
    let td = tempdir().unwrap();
    let dest = td.path().join("T99_Test_Root");
    recreate_t99_into(&dest);

    // Verify a few expected paths exist
    assert!(dest.join("99-99_Test_Range/99_TestCat/99.01_TestItem").exists());
    assert!(dest.join("99-99_Test_Range/99_TestCat/99.02_Example.url").exists());
    assert!(dest.join("99-99_Test_Range/99_TestCat/99.03_Website.webloc").exists());

    // Run a quick tree to ensure the scanner handles it
    let mut cmd = Command::cargo_bin("jd-helper").unwrap();
    cmd.arg("tree").arg(dest.to_str().unwrap());
    cmd.assert().success();
}


