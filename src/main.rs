mod cargo_proxy;

use clap::Parser;
use crap4rust::{analyze, run_shell, Error, FunctionMetric, VERSION};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, SystemTime};

const DEFAULT_COVERAGE: &str = "target/coverage/lcov.info";
const DEFAULT_TEST: &str =
    "cargo llvm-cov --workspace --lcov --output-path target/coverage/lcov.info";

#[derive(Parser, Debug)]
#[command(name = "crap4rust", version = VERSION, about = "Native Rust CRAP metric analyzer")]
struct Args {
    #[arg(value_name = "PATH_FRAGMENT")]
    filters: Vec<String>,
    #[arg(long, default_value = ".")]
    root: PathBuf,
    /// Cargo features to enable for source scope and built-in Cargo commands.
    #[arg(long, value_delimiter = ',', conflicts_with = "all_features")]
    features: Vec<String>,
    /// Disable Cargo default features. May be combined with --features.
    #[arg(long, conflicts_with = "all_features")]
    no_default_features: bool,
    /// Enable every Cargo feature. Fails normally if the project forbids that combination.
    #[arg(long)]
    all_features: bool,
    #[arg(long, default_value = DEFAULT_COVERAGE)]
    coverage: PathBuf,
    #[arg(long, default_value = DEFAULT_TEST)]
    test_command: String,
    #[arg(long, default_value_t = 1800)]
    timeout: u64,
    #[arg(long)]
    no_test: bool,
    #[arg(long)]
    allow_missing_coverage: bool,
    #[arg(long)]
    allow_empty: bool,
    #[arg(long)]
    include_tests: bool,
    #[arg(long)]
    json: bool,
    #[arg(long)]
    fail_over: Option<f64>,
}

#[derive(Serialize)]
struct Report<'a> {
    schema_version: u8,
    tool: &'static str,
    version: &'static str,
    root: String,
    summary: Summary,
    functions: &'a [FunctionMetric],
}

#[derive(Serialize)]
struct Summary {
    functions: usize,
    missing_coverage: usize,
    over_limit: usize,
    limit: Option<f64>,
}

fn resolve(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

fn is_safe_generated(root: &Path, path: &Path) -> bool {
    let Ok(candidate) = path.canonicalize() else {
        return false;
    };
    ["target", "coverage", ".coverage"].iter().any(|name| {
        root.join(name)
            .canonicalize()
            .ok()
            .is_some_and(|generated_root| candidate.starts_with(generated_root))
    })
}

fn prepare_coverage(root: &Path, coverage: &Path) -> Result<SystemTime, Error> {
    if coverage.exists() && is_safe_generated(root, coverage) {
        fs::remove_file(coverage)?;
    }
    if let Some(parent) = coverage.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(SystemTime::now())
}

fn print_table(metrics: &[FunctionMetric]) {
    println!("CRAP Report");
    println!("===========");
    println!(
        "{:<38} {:<42} {:>4} {:>7} {:>8}",
        "Function", "File", "CC", "Cov%", "CRAP"
    );
    for item in metrics {
        let coverage = item
            .coverage
            .map_or_else(|| "N/A".into(), |value| format!("{value:.1}%"));
        let crap = item
            .crap
            .map_or_else(|| "N/A".into(), |value| format!("{value:.2}"));
        println!(
            "{:<38} {:<42} {:>4} {:>7} {:>8}",
            item.name, item.file, item.complexity, coverage, crap
        );
    }
}

fn run() -> Result<u8, Error> {
    let args = Args::parse();
    let root = args.root.canonicalize()?;
    let cargo_args =
        cargo_proxy::feature_args(&args.features, args.all_features, args.no_default_features);
    let _cargo_proxy = cargo_proxy::install(&root, "crap4rust", &cargo_args)?;
    let coverage = resolve(&root, &args.coverage);
    if !args.no_test {
        let started = prepare_coverage(&root, &coverage)?;
        run_shell(&args.test_command, &root, Duration::from_secs(args.timeout))?;
        let metadata = fs::metadata(&coverage).map_err(|_| {
            Error::Coverage(format!(
                "coverage report was not created: {}",
                coverage.display()
            ))
        })?;
        if metadata.len() == 0 {
            return Err(Error::Coverage(format!(
                "coverage report is empty: {}",
                coverage.display()
            )));
        }
        if metadata.modified().ok().is_some_and(|time| time < started) {
            return Err(Error::Coverage(format!(
                "coverage report is stale: {}",
                coverage.display()
            )));
        }
    }
    let metrics = analyze(&root, &coverage, args.include_tests, &args.filters)?;
    if metrics.is_empty() && !args.allow_empty {
        return Err(Error::Coverage("no Rust functions were discovered".into()));
    }
    let missing = metrics
        .iter()
        .filter(|item| item.coverage.is_none())
        .count();
    if missing > 0 && !args.allow_missing_coverage {
        return Err(Error::Coverage(format!(
            "coverage is missing for {missing} function(s)"
        )));
    }
    let over = args.fail_over.map_or(0, |limit| {
        metrics
            .iter()
            .filter(|item| item.crap.is_some_and(|value| value > limit))
            .count()
    });
    if args.json {
        let report = Report {
            schema_version: 1,
            tool: "crap4rust",
            version: VERSION,
            root: root.to_string_lossy().to_string(),
            summary: Summary {
                functions: metrics.len(),
                missing_coverage: missing,
                over_limit: over,
                limit: args.fail_over,
            },
            functions: &metrics,
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&report).expect("serializable report")
        );
    } else {
        print_table(&metrics);
    }
    Ok(if over > 0 { 2 } else { 0 })
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("crap4rust: {error}");
            ExitCode::from(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn generated_target_file_is_safe_to_remove() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("target/coverage");
        fs::create_dir_all(&target).unwrap();
        let report = target.join("lcov.info");
        fs::write(&report, "old coverage").unwrap();
        let root = dir.path().canonicalize().unwrap();

        assert!(is_safe_generated(&root, &report));
        prepare_coverage(&root, &report).unwrap();
        assert!(!report.exists());
    }

    #[test]
    fn traversal_through_target_does_not_delete_project_file() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("target")).unwrap();
        let manifest = dir.path().join("Cargo.toml");
        fs::write(&manifest, "[package]\nname='safe'\nversion='0.1.0'\n").unwrap();
        let root = dir.path().canonicalize().unwrap();
        let traversal = root.join("target/../Cargo.toml");

        assert!(traversal.exists());
        assert!(!is_safe_generated(&root, &traversal));
        prepare_coverage(&root, &traversal).unwrap();
        assert!(manifest.exists());
    }

    #[cfg(unix)]
    #[test]
    fn generated_directory_symlink_cannot_escape_cleanup_boundary() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().unwrap();
        let outside = tempdir().unwrap();
        let target = dir.path().join("target/coverage");
        fs::create_dir_all(&target).unwrap();
        let outside_file = outside.path().join("outside.info");
        fs::write(&outside_file, "keep me").unwrap();
        let report = target.join("lcov.info");
        symlink(&outside_file, &report).unwrap();
        let root = dir.path().canonicalize().unwrap();

        assert!(!is_safe_generated(&root, &report));
        prepare_coverage(&root, &report).unwrap();
        assert_eq!(fs::read_to_string(&outside_file).unwrap(), "keep me");
        assert!(report.exists());
    }
}
