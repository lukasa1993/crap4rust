use std::fs;

fn replace_once(text: &mut String, old: &str, new: &str, label: &str) {
    let start = text.find(old).unwrap_or_else(|| panic!("missing anchor: {label}"));
    assert!(text[start + old.len()..].find(old).is_none(), "duplicate anchor: {label}");
    text.replace_range(start..start + old.len(), new);
}

fn replace_between(text: &mut String, start: &str, end: &str, new: &str, label: &str) {
    let from = text.find(start).unwrap_or_else(|| panic!("missing start anchor: {label}"));
    let relative = text[from..].find(end).unwrap_or_else(|| panic!("missing end anchor: {label}"));
    text.replace_range(from..from + relative, new);
}

fn main() {
    let mut scope = fs::read_to_string("src/scope.rs").unwrap();
    if !scope.contains("fn include_literal(") {
        replace_once(&mut scope, "use syn::parse::Parser;", "use syn::parse::{ParseStream, Parser};", "parser import");
        replace_between(&mut scope, "fn static_include_path(", "fn item_attrs(", r####"fn include_literal(tokens: TokenStream) -> Option<LitStr> {
    let parser = |input: ParseStream<'_>| {
        let literal: LitStr = input.parse()?;
        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
        }
        if !input.is_empty() {
            return Err(input.error("include! expects one string literal"));
        }
        Ok(literal)
    };
    parser.parse2(tokens).ok()
}

fn static_include_path(item: &syn::ItemMacro, source_dir: &Path) -> Option<PathBuf> {
    if !item.mac.path.is_ident("include") {
        return None;
    }
    let literal = include_literal(item.mac.tokens.clone())?;
    let path = PathBuf::from(literal.value());
    if path.extension().and_then(|value| value.to_str()) != Some("rs") {
        return None;
    }
    Some(if path.is_absolute() {
        path
    } else {
        source_dir.join(path)
    })
}

"####, "static include parser");
    }
    if !scope.contains("fn absolute_static_include_with_trailing_comma_is_discovered()") {
        let marker = "    #[test]\n    fn static_include_keeps_current_module_prefix()";
        let index = scope.find(marker).expect("missing static include test");
        scope.insert_str(index, r####"    #[test]
    fn absolute_static_include_with_trailing_comma_is_discovered() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname='absolute-include-fixture'\nversion='0.1.0'\nedition='2021'\n",
        )
        .unwrap();
        let shared = dir.path().join("shared.rs");
        fs::write(&shared, "pub fn shared() -> bool { true }\n").unwrap();
        let literal = format!("{:?}", shared.to_string_lossy().as_ref());
        fs::write(
            dir.path().join("src/lib.rs"),
            format!("include!({literal},);\n"),
        )
        .unwrap();
        let files = discover(dir.path(), false, &[]).unwrap();
        let expected = shared.canonicalize().unwrap();
        assert!(files.iter().any(|file| file.path == expected));
    }

"####);
    }
    fs::write("src/scope.rs", scope).unwrap();
}
