use std::fs;

fn replace_once(text: &mut String, old: &str, new: &str, label: &str) {
    let start = text.find(old).unwrap_or_else(|| panic!("missing anchor: {label}"));
    assert!(
        text[start + old.len()..].find(old).is_none(),
        "duplicate anchor: {label}"
    );
    text.replace_range(start..start + old.len(), new);
}

fn main() {
    let path = "src/scope.rs";
    let mut source = fs::read_to_string(path).unwrap();
    replace_once(
        &mut source,
        "fn walk_modules(\n    items: &[syn::Item],\n    current_path: &Path,\n    module_dir: &Path,",
        "fn walk_modules(\n    items: &[syn::Item],\n    module_dir: &Path,",
        "walk_modules signature",
    );
    replace_once(
        &mut source,
        "            walk_modules(\n                nested,\n                current_path,\n                &nested_dir,",
        "            walk_modules(\n                nested,\n                &nested_dir,",
        "inline module recursion",
    );
    replace_once(
        &mut source,
        "    walk_modules(\n        &syntax.items,\n        &canonical,\n        &module_dir,",
        "    walk_modules(\n        &syntax.items,\n        &module_dir,",
        "file module traversal",
    );
    replace_once(
        &mut source,
        "        .filter(|path| path.file_name().and_then(|value| value.to_str()) != Some(\"build.rs\"))\n        .filter(|path| {",
        "        .filter(|path| path.file_name().and_then(|value| value.to_str()) != Some(\"build.rs\"))\n        .filter(|path| {\n            include_tests\n                || !path\n                    .file_name()\n                    .and_then(|value| value.to_str())\n                    .is_some_and(|name| name.ends_with(\"_test.rs\"))\n        })\n        .filter(|path| {",
        "fallback _test.rs exclusion",
    );
    fs::write(path, source).unwrap();
}
