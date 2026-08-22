use proc_macro2::Span;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::time::Duration;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{BinOp, Block, ExprBinary, ExprClosure, ItemFn};
use thiserror::Error;
use wait_timeout::ChildExt;
use walkdir::{DirEntry, WalkDir};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Rust parse error in {path}: {source}")]
    Parse { path: PathBuf, source: syn::Error },
    #[error("coverage report error: {0}")]
    Coverage(String),
    #[error("command timed out after {0:?}")]
    Timeout(Duration),
    #[error("command failed with exit code {code}: {command}")]
    Command { code: i32, command: String },
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct FunctionMetric {
    pub name: String,
    pub file: String,
    pub start_line: usize,
    pub end_line: usize,
    pub complexity: usize,
    pub coverage: Option<f64>,
    pub crap: Option<f64>,
}

#[derive(Debug, Default)]
pub struct Coverage {
    files: HashMap<String, HashMap<usize, u64>>,
}

#[derive(Debug)]
pub struct CommandResult {
    pub status: ExitStatus,
}

fn ignored(entry: &DirEntry) -> bool {
    matches!(
        entry.file_name().to_str(),
        Some(".git" | "target" | "vendor" | "node_modules" | ".venv" | "venv" | "build" | "dist")
    )
}

fn is_test_path(path: &Path, root: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(root) else {
        return false;
    };
    relative
        .components()
        .any(|part| part.as_os_str() == "tests")
        || relative
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with("_test.rs"))
}

pub fn discover_files(root: &Path, include_tests: bool, filters: &[String]) -> Vec<PathBuf> {
    let mut files: Vec<_> = WalkDir::new(root)
        .into_iter()
        .filter_entry(|entry| !ignored(entry))
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("rs"))
        .filter(|path| include_tests || !is_test_path(path, root))
        .filter(|path| {
            if filters.is_empty() {
                return true;
            }
            let relative = path.strip_prefix(root).unwrap_or(path).to_string_lossy();
            filters.iter().any(|filter| relative.contains(filter))
        })
        .collect();
    files.sort();
    files
}

fn slice_span(source: &str, span: Span) -> String {
    let range = span.byte_range();
    source.get(range).unwrap_or_default().trim().to_string()
}

struct ComplexityVisitor {
    value: usize,
}

impl ComplexityVisitor {
    fn for_block(block: &Block) -> usize {
        let mut visitor = Self { value: 1 };
        visitor.visit_block(block);
        visitor.value
    }
}

impl<'ast> Visit<'ast> for ComplexityVisitor {
    fn visit_expr_if(&mut self, node: &'ast syn::ExprIf) {
        self.value += 1;
        visit::visit_expr_if(self, node);
    }

    fn visit_expr_for_loop(&mut self, node: &'ast syn::ExprForLoop) {
        self.value += 1;
        visit::visit_expr_for_loop(self, node);
    }

    fn visit_expr_while(&mut self, node: &'ast syn::ExprWhile) {
        self.value += 1;
        visit::visit_expr_while(self, node);
    }

    fn visit_expr_loop(&mut self, node: &'ast syn::ExprLoop) {
        self.value += 1;
        visit::visit_expr_loop(self, node);
    }

    fn visit_arm(&mut self, node: &'ast syn::Arm) {
        self.value += 1;
        visit::visit_arm(self, node);
    }

    fn visit_expr_binary(&mut self, node: &'ast ExprBinary) {
        if matches!(node.op, BinOp::And(_) | BinOp::Or(_)) {
            self.value += 1;
        }
        visit::visit_expr_binary(self, node);
    }

    fn visit_expr_try(&mut self, node: &'ast syn::ExprTry) {
        self.value += 1;
        visit::visit_expr_try(self, node);
    }

    fn visit_expr_closure(&mut self, _node: &'ast ExprClosure) {}
    fn visit_item_fn(&mut self, _node: &'ast ItemFn) {}
}

fn metric(name: String, file: &str, span: Span, block: &Block) -> FunctionMetric {
    let start = span.start();
    let end = span.end();
    FunctionMetric {
        name,
        file: file.to_string(),
        start_line: start.line.max(1),
        end_line: end.line.max(start.line).max(1),
        complexity: ComplexityVisitor::for_block(block),
        coverage: None,
        crap: None,
    }
}

fn collect_items(
    items: &[syn::Item],
    source: &str,
    file: &str,
    module_prefix: &str,
    out: &mut Vec<FunctionMetric>,
) {
    for item in items {
        match item {
            syn::Item::Fn(function) => {
                let local = function.sig.ident.to_string();
                let name = if module_prefix.is_empty() {
                    local
                } else {
                    format!("{module_prefix}::{local}")
                };
                out.push(metric(name, file, function.span(), &function.block));
            }
            syn::Item::Impl(implementation) => {
                let owner = slice_span(source, implementation.self_ty.span());
                for member in &implementation.items {
                    if let syn::ImplItem::Fn(function) = member {
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
                let owner = trait_item.ident.to_string();
                for member in &trait_item.items {
                    if let syn::TraitItem::Fn(function) = member {
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
                if let Some((_, items)) = &module.content {
                    let next = if module_prefix.is_empty() {
                        module.ident.to_string()
                    } else {
                        format!("{module_prefix}::{}", module.ident)
                    };
                    collect_items(items, source, file, &next, out);
                }
            }
            _ => {}
        }
    }
}

pub fn extract_functions(path: &Path, root: &Path) -> Result<Vec<FunctionMetric>, Error> {
    let source = fs::read_to_string(path)?;
    let syntax = syn::parse_file(&source).map_err(|source_error| Error::Parse {
        path: path.to_path_buf(),
        source: source_error,
    })?;
    let relative = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    let mut metrics = Vec::new();
    collect_items(&syntax.items, &source, &relative, "", &mut metrics);
    metrics.sort_by_key(|item| (item.start_line, item.name.clone()));
    Ok(metrics)
}

pub fn score(complexity: usize, coverage_percent: f64) -> f64 {
    let uncovered = 1.0 - coverage_percent / 100.0;
    complexity as f64 * complexity as f64 * uncovered.powi(3) + complexity as f64
}

fn normalize_path(value: &str) -> String {
    value
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_string()
}

pub fn load_lcov(path: &Path) -> Result<Coverage, Error> {
    let text = fs::read_to_string(path)
        .map_err(|error| Error::Coverage(format!("cannot read {}: {error}", path.display())))?;
    let mut files: HashMap<String, HashMap<usize, u64>> = HashMap::new();
    let mut current: Option<String> = None;
    for raw in text.lines() {
        if let Some(filename) = raw.strip_prefix("SF:") {
            let filename = normalize_path(filename.trim());
            files.entry(filename.clone()).or_default();
            current = Some(filename);
        } else if let (Some(filename), Some(data)) = (current.as_ref(), raw.strip_prefix("DA:")) {
            let mut fields = data.split(',');
            let line = fields.next().and_then(|value| value.parse::<usize>().ok());
            let count = fields.next().and_then(|value| value.parse::<u64>().ok());
            if let (Some(line), Some(count)) = (line, count) {
                files
                    .entry(filename.clone())
                    .or_default()
                    .insert(line, count);
            }
        }
    }
    if files.values().all(HashMap::is_empty) {
        return Err(Error::Coverage(format!(
            "{} contains no executable line data",
            path.display()
        )));
    }
    Ok(Coverage { files })
}

fn coverage_lines<'a>(
    coverage: &'a Coverage,
    root: &Path,
    filename: &str,
) -> Option<&'a HashMap<usize, u64>> {
    let normalized = normalize_path(filename);
    if let Some(value) = coverage.files.get(&normalized) {
        return Some(value);
    }
    let absolute = normalize_path(&root.join(filename).to_string_lossy());
    if let Some(value) = coverage.files.get(&absolute) {
        return Some(value);
    }
    let suffix: Vec<_> = coverage
        .files
        .iter()
        .filter(|(candidate, _)| candidate.ends_with(&format!("/{normalized}")))
        .map(|(_, lines)| lines)
        .collect();
    if suffix.len() == 1 {
        Some(suffix[0])
    } else {
        None
    }
}

pub fn apply_coverage(root: &Path, metrics: &mut [FunctionMetric], coverage: &Coverage) {
    for item in metrics {
        let Some(lines) = coverage_lines(coverage, root, &item.file) else {
            continue;
        };
        let relevant: Vec<_> = lines
            .iter()
            .filter(|(line, _)| **line >= item.start_line && **line <= item.end_line)
            .collect();
        if relevant.is_empty() {
            continue;
        }
        let covered = relevant.iter().filter(|(_, count)| **count > 0).count();
        let percent = 100.0 * covered as f64 / relevant.len() as f64;
        item.coverage = Some(percent);
        item.crap = Some(score(item.complexity, percent));
    }
}

pub fn analyze(
    root: &Path,
    coverage_path: &Path,
    include_tests: bool,
    filters: &[String],
) -> Result<Vec<FunctionMetric>, Error> {
    let coverage = load_lcov(coverage_path)?;
    let mut metrics = Vec::new();
    for path in discover_files(root, include_tests, filters) {
        metrics.extend(extract_functions(&path, root)?);
    }
    apply_coverage(root, &mut metrics, &coverage);
    metrics.sort_by(|left, right| {
        right
            .crap
            .partial_cmp(&left.crap)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.file.cmp(&right.file))
            .then_with(|| left.start_line.cmp(&right.start_line))
    });
    Ok(metrics)
}

pub fn run_shell(command: &str, root: &Path, timeout: Duration) -> Result<CommandResult, Error> {
    #[cfg(windows)]
    let mut child = Command::new("cmd")
        .args(["/C", command])
        .current_dir(root)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;
    #[cfg(not(windows))]
    let mut child = Command::new("sh")
        .args(["-c", command])
        .current_dir(root)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;
    match child.wait_timeout(timeout)? {
        Some(status) if status.success() => Ok(CommandResult { status }),
        Some(status) => Err(Error::Command {
            code: status.code().unwrap_or(1),
            command: command.to_string(),
        }),
        None => {
            let _ = child.kill();
            let _ = child.wait();
            Err(Error::Timeout(timeout))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn score_matches_crap_formula() {
        assert!((score(10, 50.0) - 22.5).abs() < 1e-9);
        assert!((score(10, 100.0) - 10.0).abs() < 1e-9);
    }

    #[test]
    fn extracts_free_and_impl_functions_with_rust_complexity() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("sample.rs");
        fs::write(&source, "pub fn choose(a: bool, b: bool) -> Result<i32, ()> {\n if a && b { Ok(1) } else { Err(())? }\n}\nstruct Thing;\nimpl Thing { fn value(&self, x: i32) -> i32 { match x { 0 => 1, _ => 2 } } }\n").unwrap();
        let metrics = extract_functions(&source, dir.path()).unwrap();
        let choose = metrics.iter().find(|item| item.name == "choose").unwrap();
        assert!(choose.complexity >= 4);
        assert!(metrics.iter().any(|item| item.name == "Thing::value"));
    }

    #[test]
    fn maps_lcov_to_function_executable_lines() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("sample.rs");
        fs::write(
            &source,
            "fn choose(x: bool) -> i32 {\n if x { 1 } else { 0 }\n}\n",
        )
        .unwrap();
        let coverage = dir.path().join("lcov.info");
        fs::write(
            &coverage,
            format!(
                "SF:{}\nDA:1,1\nDA:2,0\nDA:3,1\nend_of_record\n",
                source.display()
            ),
        )
        .unwrap();
        let metrics = analyze(dir.path(), &coverage, false, &[]).unwrap();
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].coverage, Some(200.0 / 3.0));
        assert!(metrics[0].crap.is_some());
    }

    #[test]
    fn missing_lcov_is_an_error() {
        let dir = tempdir().unwrap();
        assert!(analyze(dir.path(), &dir.path().join("missing.info"), false, &[]).is_err());
    }
}
