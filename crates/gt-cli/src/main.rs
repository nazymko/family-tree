#![forbid(unsafe_code)]
//! `gt`: command-line front end for the gene-tree engine.
//!
//! Exit codes: 0 success, 1 user error (bad input, existing project), 2 internal error.
//! The system clock is read here and nowhere else.

use clap::{Parser, Subcommand, ValueEnum};
use gt_core::DayOrder;
use gt_core::GedcomVersion;
use gt_core::privacy::{LivingMode, PrivacyPolicy, filter};
use gt_gedcom::{ExportOptions, ImportOptions, export};
use gt_store::{BackupReason, LocationWarning, Project, StoreError, check_location};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Meta key remembering which GEDCOM version the project was imported from.
const META_GEDCOM_VERSION: &str = "source_gedcom_version";

#[derive(Parser)]
#[command(name = "gt", version, about = "gene-tree command-line tool")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Parse a GEDCOM or Древо Жизни (.atdb) file and print the import report; creates nothing.
    Check {
        file: PathBuf,
        /// Read ambiguous numeric dates as month/day/year.
        #[arg(long)]
        mdy: bool,
    },
    /// Import a GEDCOM or Древо Жизни (.atdb) file into a new project folder.
    Import {
        file: PathBuf,
        #[arg(long)]
        into: PathBuf,
        #[arg(long)]
        mdy: bool,
        /// Folder to search for media files (repeatable).
        #[arg(long = "media-root")]
        media_root: Vec<PathBuf>,
        /// Also write the full import report to this file.
        #[arg(long)]
        report: Option<PathBuf>,
        /// Overwrite the --report file if it exists.
        #[arg(long)]
        force: bool,
    },
    /// Export a project to a GEDCOM file, applying the privacy filter.
    Export {
        dir: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        /// GEDCOM version (default: the version the project was imported from, else 5.5.1).
        #[arg(long, value_enum)]
        gedcom: Option<GedArg>,
        #[arg(long, value_enum, default_value = "placeholder")]
        living: LivingArg,
        #[arg(long)]
        include_private: bool,
        /// Overwrite the output file if it exists.
        #[arg(long)]
        force: bool,
    },
    /// Search a project.
    Search {
        dir: PathBuf,
        query: String,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// Make a consistent backup of a project.
    Backup {
        dir: PathBuf,
        #[arg(long, default_value_t = 10)]
        keep: usize,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum GedArg {
    #[value(name = "5.5.1")]
    V551,
    #[value(name = "7.0")]
    V70,
}

#[derive(Clone, Copy, ValueEnum)]
enum LivingArg {
    Exclude,
    Placeholder,
    All,
}

enum CliError {
    User(String),
    Internal(String),
}

impl From<StoreError> for CliError {
    fn from(e: StoreError) -> Self {
        match e {
            StoreError::AlreadyExists(_)
            | StoreError::NotAProject(_)
            | StoreError::NewerSchema { .. }
            | StoreError::Locked(_)
            | StoreError::Io(_) => CliError::User(e.to_string()),
            StoreError::Sql(_) | StoreError::Corrupt(_) => CliError::Internal(e.to_string()),
        }
    }
}

fn user(msg: impl Into<String>) -> CliError {
    CliError::User(msg.into())
}

/// Julian day number of today (UTC). The only clock read in the program.
fn today_jdn() -> i32 {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // 1970-01-01 is JDN 2440588 (day number, no fraction).
    2_440_588 + (secs / 86_400) as i32
}

fn day_order(mdy: bool) -> DayOrder {
    if mdy { DayOrder::Mdy } else { DayOrder::Dmy }
}

fn one_line(s: &str) -> String {
    s.lines().next().unwrap_or_default().trim().to_string()
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(c) => c,
        Err(e) => {
            if e.use_stderr() {
                let text = e.to_string();
                eprintln!("error: {}", one_line(text.trim_start_matches("error: ")));
                return ExitCode::from(1);
            }
            print!("{e}");
            return ExitCode::SUCCESS;
        }
    };
    match run(cli.cmd) {
        Ok(()) => ExitCode::SUCCESS,
        Err(CliError::User(m)) => {
            eprintln!("error: {}", one_line(&m));
            ExitCode::from(1)
        }
        Err(CliError::Internal(m)) => {
            eprintln!("internal error: {}", one_line(&m));
            ExitCode::from(2)
        }
    }
}

fn run(cmd: Cmd) -> Result<(), CliError> {
    match cmd {
        Cmd::Check { file, mdy } => {
            let out = convert_file(&file, mdy, Vec::new())?.outcome;
            print!("{}", out.report.to_text());
            Ok(())
        }
        Cmd::Import {
            file,
            into,
            mdy,
            media_root,
            report,
            force,
        } => cmd_import(&file, &into, mdy, media_root, report.as_deref(), force),
        Cmd::Export {
            dir,
            output,
            gedcom,
            living,
            include_private,
            force,
        } => cmd_export(&dir, &output, gedcom, living, include_private, force),
        Cmd::Search { dir, query, limit } => {
            let project = Project::open(&dir)?;
            for hit in project.search(&query, limit)? {
                println!(
                    "{}  {}  {}",
                    format!("{:?}", hit.kind).to_lowercase(),
                    hit.label,
                    hit.id
                );
            }
            Ok(())
        }
        Cmd::Backup { dir, keep } => {
            let project = Project::open(&dir)?;
            let path = project.backup(BackupReason::Manual, keep)?;
            println!("{}", path.display());
            Ok(())
        }
    }
}

/// Reads `file` with the converter for its format: a Древо Жизни `.atdb` database (found by its
/// SQLite header and tables), else GEDCOM. This is the CLI's counterpart of the format seam in
/// `gt_session::import_format`; a new format adds one branch here.
/// Detects the format (GEDCOM, GEDZIP, Gramps XML, Древо Жизни) and converts the file. Keep the
/// result's `cleanup` alive until media has been copied (GEDZIP media lives in a scratch folder).
fn convert_file(
    file: &Path,
    mdy: bool,
    media_roots: Vec<PathBuf>,
) -> Result<gt_import::Converted, CliError> {
    let opts = import_options(file, mdy, media_roots);
    let format = gt_import::detect(file).map_err(|e| user(e.message))?;
    gt_import::convert(format, file, &opts).map_err(|e| user(e.message))
}

fn import_options(file: &Path, mdy: bool, media_roots: Vec<PathBuf>) -> ImportOptions {
    let dir = file
        .parent()
        .map(Path::to_path_buf)
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| PathBuf::from("."));
    ImportOptions {
        day_order: day_order(mdy),
        media_roots,
        gedcom_dir: Some(dir),
    }
}

fn describe(w: LocationWarning) -> String {
    match w {
        LocationWarning::SyncedFolder(p) => format!(
            "this folder is inside a {p} sync folder; SQLite databases can be corrupted there"
        ),
        LocationWarning::NetworkShare => {
            "this folder is on a network share; SQLite databases can be corrupted there".into()
        }
    }
}

/// Refuses to overwrite an existing output file unless `force` is set.
fn check_overwrite(path: &Path, force: bool) -> Result<(), CliError> {
    if !force && path.exists() {
        return Err(user(format!(
            "{} exists; pass --force to overwrite",
            path.display()
        )));
    }
    Ok(())
}

/// Sibling scratch folder `<target>.partial-<pid>` that the import is built in.
fn partial_dir(into: &Path) -> PathBuf {
    let mut name = into.as_os_str().to_os_string();
    name.push(format!(".partial-{}", std::process::id()));
    PathBuf::from(name)
}

/// Builds the project in `tmp` and returns the import outcome. The project is closed on return.
fn build_project(
    tmp: &Path,
    file: &Path,
    mdy: bool,
    media_roots: Vec<PathBuf>,
) -> Result<gt_gedcom::ImportOutcome, CliError> {
    let mut project = Project::create(tmp)?;
    let converted = convert_file(file, mdy, media_roots)?;
    let _scratch = converted.cleanup; // removed when this function returns, after the copy
    let mut outcome = converted.outcome;
    for (id, src) in &outcome.resolved_media {
        let (rel, sha) = project.add_media_file(src)?;
        if let Some(m) = outcome.tree.media.get_mut(id) {
            m.path = rel;
            m.sha256 = Some(sha);
        }
    }
    project.save_tree(&outcome.tree, today_jdn())?;
    if let Some(v) = &outcome.report.version {
        project.meta_set(META_GEDCOM_VERSION, v)?;
    }
    Ok(outcome)
}

fn cmd_import(
    file: &Path,
    into: &Path,
    mdy: bool,
    media_roots: Vec<PathBuf>,
    report_path: Option<&Path>,
    force: bool,
) -> Result<(), CliError> {
    if let Some(p) = report_path {
        check_overwrite(p, force)?;
    }
    if !file.is_file() {
        return Err(user(format!("cannot read {}: not a file", file.display())));
    }
    if into.join("project.db").exists() || into.exists() && !is_empty_dir(into) {
        return Err(StoreError::AlreadyExists(into.to_path_buf()).into());
    }
    let tmp = partial_dir(into);
    if tmp.exists() {
        // Leftover of an earlier run with the same pid.
        let _ = std::fs::remove_dir_all(&tmp);
    }
    // The report is written before the rename: if it fails, nothing is left behind and the
    // user sees one error, instead of a saved project plus a failing exit code.
    let result = build_project(&tmp, file, mdy, media_roots).and_then(|outcome| {
        let text = outcome.report.to_text();
        if let Some(p) = report_path {
            std::fs::write(p, &text)
                .map_err(|e| user(format!("cannot write {}: {e}", p.display())))?;
        }
        if into.exists() {
            // An empty folder is allowed as a target; make room for the rename.
            std::fs::remove_dir(into).map_err(StoreError::from)?;
        }
        std::fs::rename(&tmp, into).map_err(StoreError::from)?;
        Ok((outcome, text))
    });
    let (outcome, text) = match result {
        Ok(v) => v,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&tmp);
            return Err(e);
        }
    };
    let t = &outcome.tree;
    println!(
        "Imported {} persons, {} families, {} events, {} places, {} sources, {} media, {} notes into {}",
        t.persons.len(),
        t.families.len(),
        t.events.len(),
        t.places.len(),
        t.sources.len(),
        t.media.len(),
        t.notes.len(),
        into.display()
    );
    println!(
        "Media files copied: {}; not found: {}",
        outcome.resolved_media.len(),
        outcome.report.missing_media.len()
    );
    print!("{text}");
    if let Some(w) = check_location(into) {
        println!("warning: {}", describe(w));
    }
    Ok(())
}

fn is_empty_dir(p: &Path) -> bool {
    std::fs::read_dir(p).is_ok_and(|mut d| d.next().is_none())
}

fn version_from_text(s: &str) -> Option<GedcomVersion> {
    let s = s.trim();
    if s.starts_with('7') {
        Some(GedcomVersion::V70)
    } else if s.starts_with("5.5") {
        Some(GedcomVersion::V551)
    } else {
        None
    }
}

/// The version recorded at import, else `tree_header.source_version` when it names a GEDCOM
/// version, else 5.5.1. (The header field normally holds the producer's software version.)
fn default_version(project: &Project, header_version: Option<&str>) -> GedcomVersion {
    let stored: Option<String> = project.meta_get(META_GEDCOM_VERSION).ok().flatten();
    stored
        .as_deref()
        .and_then(version_from_text)
        .or_else(|| header_version.and_then(version_from_text))
        .unwrap_or(GedcomVersion::V551)
}

fn cmd_export(
    dir: &Path,
    output: &Path,
    gedcom: Option<GedArg>,
    living: LivingArg,
    include_private: bool,
    force: bool,
) -> Result<(), CliError> {
    check_overwrite(output, force)?;
    let project = Project::open(dir)?;
    let tree = project.load_tree()?;
    let version = match gedcom {
        Some(GedArg::V551) => GedcomVersion::V551,
        Some(GedArg::V70) => GedcomVersion::V70,
        None => default_version(&project, tree.header.source_version.as_deref()),
    };
    let policy = PrivacyPolicy {
        living: match living {
            LivingArg::Exclude => LivingMode::Exclude,
            LivingArg::Placeholder => LivingMode::Placeholder,
            LivingArg::All => LivingMode::IncludeAll,
        },
        include_private,
        rule: project.living_rule(),
    };
    let (filtered, summary) = filter(&tree, &policy, today_jdn());
    let bytes = export(&filtered, &ExportOptions { version });
    std::fs::write(output, bytes)
        .map_err(|e| user(format!("cannot write {}: {e}", output.display())))?;
    println!(
        "Exported {} persons as GEDCOM {} to {}",
        filtered.persons.len(),
        if version == GedcomVersion::V70 {
            "7.0"
        } else {
            "5.5.1"
        },
        output.display()
    );
    println!(
        "Privacy: {} living hidden ({} placeholder), {} other records removed",
        summary.hidden_persons, summary.placeholder_persons, summary.removed_records
    );
    for (note, person) in &summary.mention_warnings {
        println!("warning: note {note} mentions hidden person {person}");
    }
    for m in &summary.group_media {
        println!("warning: media {m} is also linked from a hidden person");
    }
    Ok(())
}
