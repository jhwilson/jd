use assert_cmd::prelude::*;
use std::process::Command;
use tempfile::tempdir;
use std::fs;

#[test]
fn scan_and_write_index_roundtrip() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("R50_Research");
    fs::create_dir(&root).unwrap();
    fs::create_dir(root.join("30-39_Research_Area")).unwrap();
    fs::create_dir(root.join("30-39_Research_Area/30_Topic")).unwrap();
    fs::create_dir(root.join("30-39_Research_Area/30_Topic/30.01_ItemDir")).unwrap();
    fs::write(root.join("30-39_Research_Area/30_Topic/30.02_Doc.txt"), b"hello").unwrap();

    // tree
    let mut cmd = Command::cargo_bin("jd-helper").unwrap();
    let _ = cmd.arg("tree").arg(root.to_str().unwrap()).assert().success();
    // write-index
    let mut cmd = Command::cargo_bin("jd-helper").unwrap();
    let out_path = root.join(".jd_index.json");
    cmd.arg("write-index").arg(root.to_str().unwrap()).arg("--out").arg(out_path.to_str().unwrap()).assert().success();
    let s = fs::read_to_string(out_path).unwrap();
    assert!(s.contains("30-39"));
}


