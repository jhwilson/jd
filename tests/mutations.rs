use assert_cmd::prelude::*;
use std::process::Command;
use tempfile::tempdir;
use std::fs;
use std::path::PathBuf;

fn cargo_bin() -> Command { Command::cargo_bin("jd-helper").unwrap() }

fn set_home(cmd: &mut Command, home: &PathBuf) { cmd.env("HOME", home); }

#[test]
fn mutate_dir_file_link_rename_move_delete() {
    let td = tempdir().unwrap();
    let home = td.path().join("home"); fs::create_dir_all(&home).unwrap();
    let root = td.path().join("R50_Research"); fs::create_dir(&root).unwrap();
    // seed
    fs::create_dir(root.join("30-39_Research")) .unwrap();
    fs::create_dir(root.join("30-39_Research/30_Topic")) .unwrap();

    // scan to get ids
    let output = {
        let mut cmd = cargo_bin(); set_home(&mut cmd, &home);
        cmd.arg("scan").arg(root.to_str().unwrap());
        let out = cmd.assert().success().get_output().stdout.clone();
        String::from_utf8(out).unwrap()
    };
    let v: serde_json::Value = serde_json::from_str(&output).unwrap();
    // find id of category 30_Topic by path suffix
    fn find_by_suffix(node: &serde_json::Value, path_end: &str) -> Option<String> {
        if node.get("path").and_then(|s| s.as_str()).map(|s| s.ends_with(path_end)).unwrap_or(false) {
            return node.get("id").and_then(|s| s.as_str()).map(|s| s.to_string());
        }
        if let Some(arr) = node.get("children").and_then(|c| c.as_array()) {
            for ch in arr { if let Some(x) = find_by_suffix(ch, path_end) { return Some(x); } }
        }
        None
    }
    let mut cat_id_opt = None;
    if let Some(arr) = v.get("roots").and_then(|r| r.as_array()) {
        for r in arr { if let Some(x) = find_by_suffix(r, "30-39_Research/30_Topic") { cat_id_opt = Some(x); break; } }
    }
    let cat_id = cat_id_opt.expect("cat id");

    // new dir under category
    {
        let mut cmd = cargo_bin(); set_home(&mut cmd, &home);
        cmd.arg("new").arg("dir").arg("--parent").arg(&cat_id).arg("--name").arg("30.01_ItemDir").arg(root.to_str().unwrap());
        cmd.assert().success();
        assert!(root.join("30-39_Research/30_Topic/30.01_ItemDir").exists());
    }
    // new file
    {
        let mut cmd = cargo_bin(); set_home(&mut cmd, &home);
        cmd.arg("new").arg("file").arg("--parent").arg(&cat_id).arg("--name").arg("30.02_Note.txt").arg(root.to_str().unwrap());
        cmd.assert().success();
        assert!(root.join("30-39_Research/30_Topic/30.02_Note.txt").exists());
    }
    // new link
    {
        let mut cmd = cargo_bin(); set_home(&mut cmd, &home);
        cmd.arg("new").arg("link").arg("--parent").arg(&cat_id).arg("--name").arg("30.03_Link.url").arg("--url").arg("https://example.com").arg(root.to_str().unwrap());
        cmd.assert().success();
        let p = root.join("30-39_Research/30_Topic/30.03_Link.url");
        assert!(p.exists());
        assert!(fs::read_to_string(p).unwrap().contains("https://example.com"));
    }

    // rescan to get id of 30.02 file, then rename it
    let file_id = {
        let output = {
            let mut cmd = cargo_bin(); set_home(&mut cmd, &home);
            cmd.arg("scan").arg(root.to_str().unwrap());
            let out = cmd.assert().success().get_output().stdout.clone();
            String::from_utf8(out).unwrap()
        };
        let v: serde_json::Value = serde_json::from_str(&output).unwrap();
        fn find(node: &serde_json::Value) -> Option<String> {
            if node.get("path").and_then(|s| s.as_str()).map(|s| s.ends_with("30.02_Note.txt")).unwrap_or(false) { return node.get("id").and_then(|s| s.as_str()).map(|s| s.to_string()); }
            if let Some(arr) = node.get("children").and_then(|c| c.as_array()) {
                for ch in arr { if let Some(x) = find(ch) { return Some(x); } }
            }
            None
        }
        let mut got = None;
        if let Some(arr) = v.get("roots").and_then(|r| r.as_array()) {
            for r in arr { if let Some(x) = find(r) { got = Some(x); break; } }
        }
        got.unwrap()
    };
    {
        let mut cmd = cargo_bin(); set_home(&mut cmd, &home);
        cmd.arg("rename").arg("--id").arg(&file_id).arg("--name").arg("Renamed").arg(root.to_str().unwrap());
        cmd.assert().success();
        assert!(root.join("30-39_Research/30_Topic/30.02_Renamed.txt").exists());
    }

    // move the renamed file into item dir 30.01
    let item_dir_id = {
        let output = {
            let mut cmd = cargo_bin(); set_home(&mut cmd, &home);
            cmd.arg("scan").arg(root.to_str().unwrap());
            let out = cmd.assert().success().get_output().stdout.clone();
            String::from_utf8(out).unwrap()
        };
        let v: serde_json::Value = serde_json::from_str(&output).unwrap();
        fn find(node: &serde_json::Value, path_end: &str) -> Option<String> {
            if node.get("path").and_then(|s| s.as_str()).map(|s| s.ends_with(path_end)).unwrap_or(false) { return node.get("id").and_then(|s| s.as_str()).map(|s| s.to_string()); }
            if let Some(arr) = node.get("children").and_then(|c| c.as_array()) {
                for ch in arr { if let Some(x) = find(ch, path_end) { return Some(x); } }
            }
            None
        }
        let mut got = None;
        if let Some(arr) = v.get("roots").and_then(|r| r.as_array()) {
            for r in arr { if let Some(x) = find(r, "30.01_ItemDir") { got = Some(x); break; } }
        }
        got.unwrap()
    };
    {
        let mut cmd = cargo_bin(); set_home(&mut cmd, &home);
        cmd.arg("move").arg("--id").arg(&file_id).arg("--parent").arg(&item_dir_id).arg(root.to_str().unwrap());
        cmd.assert().success();
        assert!(root.join("30-39_Research/30_Topic/30.01_ItemDir/30.02_Renamed.txt").exists());
    }

    // delete the link
    let link_id = {
        let output = {
            let mut cmd = cargo_bin(); set_home(&mut cmd, &home);
            cmd.arg("scan").arg(root.to_str().unwrap());
            let out = cmd.assert().success().get_output().stdout.clone();
            String::from_utf8(out).unwrap()
        };
        let v: serde_json::Value = serde_json::from_str(&output).unwrap();
        fn find(node: &serde_json::Value) -> Option<String> {
            if node.get("path").and_then(|s| s.as_str()).map(|s| s.ends_with("30.03_Link.url")).unwrap_or(false) { return node.get("id").and_then(|s| s.as_str()).map(|s| s.to_string()); }
            if let Some(arr) = node.get("children").and_then(|c| c.as_array()) {
                for ch in arr { if let Some(x) = find(ch) { return Some(x); } }
            }
            None
        }
        let mut got = None;
        if let Some(arr) = v.get("roots").and_then(|r| r.as_array()) {
            for r in arr { if let Some(x) = find(r) { got = Some(x); break; } }
        }
        got.unwrap()
    };
    {
        let mut cmd = cargo_bin(); set_home(&mut cmd, &home);
        cmd.arg("delete").arg("--id").arg(&link_id).arg(root.to_str().unwrap());
        cmd.assert().success();
        assert!(!root.join("30-39_Research/30_Topic/30.03_Link.url").exists());
    }

    // ensure write-index mirrors disk
    {
        let mut cmd = cargo_bin(); set_home(&mut cmd, &home);
        let out_path = root.join(".jd_index.json");
        cmd.arg("write-index").arg(root.to_str().unwrap()).arg("--out").arg(out_path.to_str().unwrap());
        cmd.assert().success();
        let s = fs::read_to_string(out_path).unwrap();
        assert!(s.contains("30.01"));
        assert!(s.contains("30.02_Renamed.txt"));
    }
}


