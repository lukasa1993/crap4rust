use std::fs;

fn replace_once(text: &mut String, old: &str, new: &str, label: &str) {
    let start = text
        .find(old)
        .unwrap_or_else(|| panic!("missing anchor: {}", label));
    assert!(
        text[start + old.len()..].find(old).is_none(),
        "duplicate anchor: {}",
        label
    );
    text.replace_range(start..start + old.len(), new);
}

fn replace_between(text: &mut String, start: &str, end: &str, new: &str, label: &str) {
    let from = text
        .find(start)
        .unwrap_or_else(|| panic!("missing start anchor: {}", label));
    let relative = text[from..]
        .find(end)
        .unwrap_or_else(|| panic!("missing end anchor: {}", label));
    text.replace_range(from..from + relative, new);
}

fn insert_before(text: &mut String, marker: &str, addition: &str, label: &str) {
    let index = text
        .find(marker)
        .unwrap_or_else(|| panic!("missing insertion anchor: {}", label));
    text.insert_str(index, addition);
}

fn main() {
    let mut source = fs::read_to_string("src/scope.rs").unwrap();

    let include_function = r###"fn built_in_include(path: &syn::Path) -> bool {
    let segments: Vec<_> = path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect();
    matches!(segments.as_slice(), [name] if name == "include")
        || matches!(segments.as_slice(), [prefix, name]
            if matches!(prefix.as_str(), "std" | "core") && name == "include")
}

fn static_include_path(item: &syn::ItemMacro, source_dir: &Path) -> Option<PathBuf> {
    if !built_in_include(&item.mac.path) {
        return None;
    }
    let literal = include_literal(item.mac.tokens.clone())?;
    let path = PathBuf::from(literal.value());
    Some(if path.is_absolute() {
        path
    } else {
        source_dir.join(path)
    })
}

"###;
    replace_between(
        &mut source,
        "fn static_include_path(",
        "fn item_attrs(",
        include_function,
        "include macro recognition",
    );

    replace_once(
        &mut source,
        "    let canonical = path\n        .canonicalize()\n        .map_err(|error| format!(\"cannot resolve Rust source {}: {error}\", path.display()))?;\n    let key = (canonical.clone(), module_prefix.to_string());",
        "    let lexical_source_dir = path.parent().unwrap_or(module_dir).to_path_buf();\n    let canonical = path\n        .canonicalize()\n        .map_err(|error| format!(\"cannot resolve Rust source {}: {error}\", path.display()))?;\n    let key = (canonical.clone(), module_prefix.to_string());",
        "lexical include base",
    );

    replace_once(
        &mut source,
        "    let source_dir = canonical.parent().unwrap_or(module_dir);\n    walk_items(\n        &syntax.items,\n        module_dir,\n        source_dir,",
        "    walk_items(\n        &syntax.items,\n        module_dir,\n        &lexical_source_dir,",
        "lexical include traversal",
    );

    let tests = r###"    #[test]
    fn qualified_extensionless_include_is_discovered() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname='qualified-extensionless-include-fixture'\nversion='0.1.0'\nedition='2021'\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("src/lib.rs"),
            "core::include!(\"generated\",);\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("src/generated"),
            "pub fn included() -> bool { true }\n",
        )
        .unwrap();
        let files = discover(dir.path(), false, &[]).unwrap();
        assert!(files.iter().any(|file| file.path.ends_with("src/generated")));
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_module_include_uses_lexical_source_directory() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::create_dir_all(dir.path().join("shared")).unwrap();
        fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname='crap-symlink-include-fixture'\nversion='0.1.0'\nedition='2021'\n",
        )
        .unwrap();
        fs::write(dir.path().join("src/lib.rs"), "mod foo;\n").unwrap();
        fs::write(
            dir.path().join("shared/foo.rs"),
            "include!(\"part.rs\");\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("src/part.rs"),
            "pub fn lexical() -> bool { true }\n",
        )
        .unwrap();
        symlink("../shared/foo.rs", dir.path().join("src/foo.rs")).unwrap();
        let files = discover(dir.path(), false, &[]).unwrap();
        assert!(files.iter().any(|file| file.path.ends_with("src/part.rs")));
    }

"###;
    insert_before(
        &mut source,
        "    #[test]\n    fn absolute_static_include_with_trailing_comma_is_discovered()",
        tests,
        "include regression tests",
    );

    fs::write("src/scope.rs", source).unwrap();
}
