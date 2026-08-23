use std::fs;

fn replace_once(text: &mut String, old: &str, new: &str, label: &str) {
    let first = text
        .find(old)
        .unwrap_or_else(|| panic!("missing patch anchor: {}", label));
    assert!(
        text[first + old.len()..].find(old).is_none(),
        "duplicate patch anchor: {}",
        label
    );
    text.replace_range(first..first + old.len(), new);
}

fn replace_between(text: &mut String, start: &str, end: &str, replacement: &str, label: &str) {
    let from = text
        .find(start)
        .unwrap_or_else(|| panic!("missing start anchor: {}", label));
    let relative = text[from..]
        .find(end)
        .unwrap_or_else(|| panic!("missing end anchor: {}", label));
    let to = from + relative;
    text.replace_range(from..to, replacement);
}

fn main() {
    let mut cargo = fs::read_to_string("Cargo.toml").unwrap();
    replace_once(
        &mut cargo,
        "version = \"2.0.1\"",
        "version = \"2.0.2\"",
        "package version",
    );
    fs::write("Cargo.toml", cargo).unwrap();

    let mut source = fs::read_to_string("src/lib.rs").unwrap();
    replace_once(
        &mut source,
        "use proc_macro2::{Span, TokenStream, TokenTree};",
        "mod scope;\n\nuse proc_macro2::Span;",
        "scope module and proc_macro2 import",
    );
    replace_once(
        &mut source,
        "use syn::{Attribute, BinOp, Block, ExprBinary, ExprClosure, ItemFn};",
        "use syn::{BinOp, Block, ExprBinary};",
        "syn imports",
    );
    replace_once(
        &mut source,
        "    #[error(\"coverage report error: {0}\")]\n    Coverage(String),",
        "    #[error(\"coverage report error: {0}\")]\n    Coverage(String),\n    #[error(\"Rust scope error: {0}\")]\n    Scope(String),",
        "scope error",
    );

    replace_between(
        &mut source,
        "#[derive(Clone, Copy)]\nstruct CfgPossibility",
        "fn slice_span",
        "",
        "legacy cfg evaluator",
    );

    replace_once(
        &mut source,
        r###"    fn visit_arm(&mut self, node: &'ast syn::Arm) {
        self.value += 1;
        visit::visit_arm(self, node);
    }"###,
        r###"    fn visit_arm(&mut self, node: &'ast syn::Arm) {
        self.value += 1;
        if node.guard.is_some() {
            self.value += 1;
        }
        visit::visit_arm(self, node);
    }"###,
        "match guard complexity",
    );
    replace_once(
        &mut source,
        r###"    fn visit_expr_try(&mut self, node: &'ast syn::ExprTry) {
        self.value += 1;
        visit::visit_expr_try(self, node);
    }

    fn visit_expr_closure(&mut self, _node: &'ast ExprClosure) {}
    fn visit_item_fn(&mut self, _node: &'ast ItemFn) {}"###,
        r###"    fn visit_local(&mut self, node: &'ast syn::Local) {
        if node.init.as_ref().is_some_and(|init| init.diverge.is_some()) {
            self.value += 1;
        }
        visit::visit_local(self, node);
    }

    fn visit_expr_try(&mut self, node: &'ast syn::ExprTry) {
        self.value += 1;
        visit::visit_expr_try(self, node);
    }"###,
        "closure nested-item and let-else complexity",
    );

    let new_collection = r###"fn collect_items(
    items: &[syn::Item],
    source: &str,
    file: &str,
    module_prefix: &str,
    cfg: &scope::CfgContext,
    out: &mut Vec<FunctionMetric>,
) {
    for item in items {
        match item {
            syn::Item::Fn(function) => {
                if !cfg.attrs_active(&function.attrs) {
                    continue;
                }
                let local = function.sig.ident.to_string();
                let name = if module_prefix.is_empty() {
                    local
                } else {
                    format!("{module_prefix}::{local}")
                };
                out.push(metric(name, file, function.span(), &function.block));
            }
            syn::Item::Impl(implementation) => {
                if !cfg.attrs_active(&implementation.attrs) {
                    continue;
                }
                let owner = slice_span(source, implementation.self_ty.span());
                for member in &implementation.items {
                    if let syn::ImplItem::Fn(function) = member {
                        if !cfg.attrs_active(&function.attrs) {
                            continue;
                        }
                        out.push(metric(
                            format!("{owner}::{}", function.sig.ident),
                            file,
                            function.span(),
                            &function.block,
                        ));
                    }
                }
            }
            syn::Item::Trait(trait_item) => {
                if !cfg.attrs_active(&trait_item.attrs) {
                    continue;
                }
                let owner = trait_item.ident.to_string();
                for member in &trait_item.items {
                    if let syn::TraitItem::Fn(function) = member {
                        if !cfg.attrs_active(&function.attrs) {
                            continue;
                        }
                        if let Some(block) = &function.default {
                            out.push(metric(
                                format!("{owner}::{}", function.sig.ident),
                                file,
                                function.span(),
                                block,
                            ));
                        }
                    }
                }
            }
            syn::Item::Mod(module) => {
                if !cfg.attrs_active(&module.attrs) {
                    continue;
                }
                if let Some((_, items)) = &module.content {
                    let next = if module_prefix.is_empty() {
                        module.ident.to_string()
                    } else {
                        format!("{module_prefix}::{}", module.ident)
                    };
                    collect_items(items, source, file, &next, cfg, out);
                }
            }
            _ => {}
        }
    }
}

fn extract_scoped(scoped: &scope::ScopedFile, root: &Path) -> Result<Vec<FunctionMetric>, Error> {
    let source = fs::read_to_string(&scoped.path)?;
    let syntax = syn::parse_file(&source).map_err(|source_error| Error::Parse {
        path: scoped.path.clone(),
        source: source_error,
    })?;
    let relative = scoped
        .path
        .strip_prefix(root)
        .unwrap_or(&scoped.path)
        .to_string_lossy()
        .replace('\\', "/");
    let mut metrics = Vec::new();
    collect_items(
        &syntax.items,
        &source,
        &relative,
        &scoped.module_prefix,
        &scoped.cfg,
        &mut metrics,
    );
    metrics.sort_by_key(|item| (item.start_line, item.name.clone()));
    Ok(metrics)
}

fn extract_functions_with_tests(
    path: &Path,
    root: &Path,
    include_tests: bool,
) -> Result<Vec<FunctionMetric>, Error> {
    let canonical = path.canonicalize()?;
    let scoped = scope::discover(root, include_tests, &[]).map_err(Error::Scope)?;
    let file = scoped
        .into_iter()
        .find(|file| {
            file.path
                .canonicalize()
                .ok()
                .is_some_and(|candidate| candidate == canonical)
        })
        .ok_or_else(|| {
            Error::Scope(format!(
                "Rust source is not active in the selected Cargo scope: {}",
                path.display()
            ))
        })?;
    extract_scoped(&file, root)
}

"###;
    replace_between(
        &mut source,
        "fn collect_items(\n",
        "pub fn extract_functions",
        new_collection,
        "function collection",
    );

    replace_once(
        &mut source,
        r###"    let coverage = load_lcov(coverage_path)?;
    let mut metrics = Vec::new();
    for path in discover_files(root, include_tests, filters) {
        metrics.extend(extract_functions_with_tests(&path, root, include_tests)?);
    }
    apply_coverage(root, &mut metrics, &coverage);"###,
        r###"    let coverage = load_lcov(coverage_path)?;
    let scoped = scope::discover(root, include_tests, filters).map_err(Error::Scope)?;
    let mut metrics = Vec::new();
    for file in &scoped {
        metrics.extend(extract_scoped(file, root)?);
    }
    apply_coverage(root, &mut metrics, &coverage);"###,
        "Cargo-scoped analysis",
    );

    replace_once(
        &mut source,
        r###"    #[test]
    fn inline_test_modules_are_excluded_by_default() {"###,
        r###"    #[test]
    fn closure_nested_items_match_guards_and_let_else_contribute_complexity() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("sample.rs");
        fs::write(
            &source,
            "fn outer(value: Option<i32>) -> i32 {\n let decide = |a: bool, b: bool| if a && b { 1 } else { 0 };\n let Some(value) = value else { return 0; };\n fn nested(x: i32) -> i32 { match x { n if n > 0 => n, _ => 0 } }\n decide(true, true) + nested(value)\n}\n",
        )
        .unwrap();
        let metrics = extract_functions(&source, dir.path()).unwrap();
        assert_eq!(metrics.len(), 1);
        assert!(metrics[0].complexity >= 7, "complexity was {}", metrics[0].complexity);
    }

    #[test]
    fn inline_test_modules_are_excluded_by_default() {"###,
        "complexity regression test",
    );

    fs::write("src/lib.rs", source).unwrap();
}
