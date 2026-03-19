use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use serde::Deserialize;
use std::fmt::Write as FmtWrite;

const BMO_BASE: &str = "https://bugzilla.mozilla.org/rest";

#[derive(Parser)]
#[command(name = "bmo", about = "Search Mozilla's Bugzilla (BMO)")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Free-text search for bugs (uses quicksearch + structured filters)
    Search(Box<SearchArgs>),
    /// Find bugs similar to a given bug
    Similar(SimilarArgs),
    /// Find bugs marked as duplicates of a given bug
    Duplicates(DuplicatesArgs),
    /// Fetch a single bug
    Get(GetArgs),
    /// Raw boolean chart query (f/o/v triplets)
    Advanced(AdvancedArgs),
}

#[derive(Clone, ValueEnum, Default)]
enum OutputFormat {
    #[default]
    Compact,
    Json,
    Markdown,
}

// -- Search --

#[derive(Parser)]
struct SearchArgs {
    /// Free-text search terms
    terms: Vec<String>,
    #[arg(long)]
    product: Option<String>,
    #[arg(long)]
    component: Option<String>,
    /// open | closed | all, or specific statuses comma-separated
    #[arg(long, default_value = "open")]
    status: String,
    /// S1,S2,S3,S4 or old-style severity names, comma-separated
    #[arg(long)]
    severity: Option<String>,
    /// defect | enhancement | task
    #[arg(long, name = "type")]
    bug_type: Option<String>,
    #[arg(long)]
    keywords: Option<String>,
    #[arg(long)]
    assignee: Option<String>,
    #[arg(long)]
    whiteboard: Option<String>,
    #[arg(long)]
    crash_signature: Option<String>,
    /// Flag filter, e.g. "needinfo?" or "review+"
    #[arg(long)]
    flag: Option<String>,
    /// ISO date, e.g. 2025-01-01
    #[arg(long)]
    created_after: Option<String>,
    /// ISO date
    #[arg(long)]
    changed_after: Option<String>,
    /// Also search in comment text (slower)
    #[arg(long)]
    comments: bool,
    #[arg(long, default_value = "20")]
    limit: u32,
    /// Skip the first N results (for pagination)
    #[arg(long, default_value = "0")]
    offset: u32,
    #[arg(long)]
    sort: Option<String>,
    /// Only return the count of matching bugs
    #[arg(long)]
    count: bool,
    #[arg(long, value_enum, default_value = "compact")]
    format: OutputFormat,
}

// -- Similar --

#[derive(Parser)]
struct SimilarArgs {
    /// Bug ID to find similar bugs for
    bug_id: u64,
    #[arg(long, default_value = "20")]
    limit: u32,
    #[arg(long, value_enum, default_value = "compact")]
    format: OutputFormat,
}

// -- Duplicates --

#[derive(Parser)]
struct DuplicatesArgs {
    /// Bug ID
    bug_id: u64,
    #[arg(long, default_value = "20")]
    limit: u32,
    #[arg(long, value_enum, default_value = "compact")]
    format: OutputFormat,
}

// -- Get --

#[derive(Parser)]
struct GetArgs {
    /// Bug ID
    bug_id: u64,
    /// Include comments
    #[arg(long)]
    comments: bool,
    /// Include change history
    #[arg(long)]
    history: bool,
    #[arg(long, value_enum, default_value = "compact")]
    format: OutputFormat,
}

// -- Advanced --

#[derive(Parser)]
struct AdvancedArgs {
    /// Field/operator/value triplets: "field:op:value" (repeatable)
    #[arg(long = "filter", short = 'f')]
    filters: Vec<String>,
    /// Combine filters with OR instead of AND
    #[arg(long)]
    or: bool,
    #[arg(long)]
    include_fields: Option<String>,
    #[arg(long, default_value = "20")]
    limit: u32,
    /// Skip the first N results (for pagination)
    #[arg(long, default_value = "0")]
    offset: u32,
    #[arg(long)]
    count: bool,
    #[arg(long, value_enum, default_value = "compact")]
    format: OutputFormat,
}

// -- Boolean chart builder --

struct ChartBuilder {
    params: Vec<(String, String)>,
    idx: u32,
}

impl ChartBuilder {
    fn new() -> Self {
        Self {
            params: Vec::new(),
            idx: 0,
        }
    }

    fn add(&mut self, field: &str, op: &str, value: &str) {
        let i = self.idx;
        self.params.push((format!("f{i}"), field.into()));
        self.params.push((format!("o{i}"), op.into()));
        self.params.push((format!("v{i}"), value.into()));
        self.idx += 1;
    }

    fn open_group(&mut self, junction: &str) {
        let i = self.idx;
        self.params.push((format!("f{i}"), "OP".into()));
        self.params.push((format!("j{i}"), junction.into()));
        self.idx += 1;
    }

    fn close_group(&mut self) {
        let i = self.idx;
        self.params.push((format!("f{i}"), "CP".into()));
        self.idx += 1;
    }

    fn into_params(self) -> Vec<(String, String)> {
        self.params
    }
}

// -- API response types --

#[derive(Deserialize, Debug)]
struct BugListResponse {
    bugs: Option<Vec<Bug>>,
    bug_count: Option<u64>,
}

#[derive(Deserialize, serde::Serialize, Debug)]
struct Bug {
    id: u64,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    resolution: String,
    #[serde(default)]
    product: String,
    #[serde(default)]
    component: String,
    #[serde(default)]
    severity: String,
    #[serde(default)]
    priority: String,
    #[serde(default)]
    assigned_to: String,
    #[serde(default)]
    keywords: Vec<String>,
    #[serde(default)]
    last_change_time: String,
    #[serde(default)]
    creation_time: String,
    #[serde(default)]
    cf_crash_signature: String,
    #[serde(default)]
    cf_webcompat_priority: String,
    #[serde(default)]
    whiteboard: String,
    #[serde(default, rename = "type")]
    bug_type: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    depends_on: Vec<u64>,
    #[serde(default)]
    blocks: Vec<u64>,
    #[serde(default)]
    see_also: Vec<String>,
    #[serde(default)]
    dupe_of: Option<u64>,
    #[serde(default)]
    duplicates: Vec<u64>,
    #[serde(default)]
    flags: Vec<Flag>,
    #[serde(default)]
    comments: Option<Vec<Comment>>,
}

#[derive(Deserialize, serde::Serialize, Debug)]
struct Flag {
    #[serde(default)]
    name: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    requestee: Option<String>,
}

#[derive(Deserialize, serde::Serialize, Debug, Clone)]
struct Comment {
    #[serde(default)]
    id: u64,
    #[serde(default)]
    text: String,
    #[serde(default)]
    creator: String,
    #[serde(default)]
    creation_time: String,
    #[serde(default)]
    count: u64,
}

#[derive(Deserialize, Debug)]
struct CommentsResponse {
    bugs: std::collections::HashMap<String, CommentsBug>,
}

#[derive(Deserialize, Debug)]
struct CommentsBug {
    comments: Vec<Comment>,
}

#[derive(Deserialize, Debug)]
struct PossibleDuplicatesResponse {
    bugs: Vec<Bug>,
}

#[derive(Deserialize, Debug)]
struct HistoryResponse {
    bugs: Vec<HistoryBug>,
}

#[derive(Deserialize, Debug)]
struct HistoryBug {
    history: Vec<HistoryEntry>,
}

#[derive(Deserialize, serde::Serialize, Debug)]
struct HistoryEntry {
    #[serde(default)]
    when: String,
    #[serde(default)]
    who: String,
    #[serde(default)]
    changes: Vec<HistoryChange>,
}

#[derive(Deserialize, serde::Serialize, Debug)]
struct HistoryChange {
    #[serde(default)]
    field_name: String,
    #[serde(default)]
    removed: String,
    #[serde(default)]
    added: String,
}

fn client() -> Result<reqwest::Client> {
    let mut headers = reqwest::header::HeaderMap::new();
    if let Ok(key) = std::env::var("BMO_API_KEY") {
        headers.insert(
            "X-BUGZILLA-API-KEY",
            key.parse().context("invalid API key")?,
        );
    }
    reqwest::Client::builder()
        .default_headers(headers)
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .context("failed to build HTTP client")
}

fn status_to_list(s: &str) -> Vec<&str> {
    match s.to_lowercase().as_str() {
        "open" => vec!["NEW", "ASSIGNED", "REOPENED", "UNCONFIRMED"],
        "closed" => vec!["RESOLVED", "VERIFIED", "CLOSED"],
        "all" => vec![],
        _ => s.split(',').collect(),
    }
}

// -- Formatting --

fn format_date_short(d: &str) -> &str {
    d.get(..10).unwrap_or(d)
}

fn format_assignee_short(a: &str) -> &str {
    a.split('@').next().unwrap_or(a)
}

fn format_flags(flags: &[Flag]) -> String {
    flags
        .iter()
        .map(|f| {
            if let Some(ref r) = f.requestee {
                format!("{}{}({})", f.name, f.status, r)
            } else {
                format!("{}{}", f.name, f.status)
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_bugs_compact(bugs: &[Bug]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "FOUND {} bugs", bugs.len());
    let _ = writeln!(out, "ID|SEVERITY|STATUS|COMPONENT|SUMMARY|CHANGED");
    for b in bugs {
        let status = if b.resolution.is_empty() {
            b.status.clone()
        } else {
            format!("{}:{}", b.status, b.resolution)
        };
        let _ = writeln!(
            out,
            "{}|{}|{}|{}::{}|{}|{}",
            b.id,
            b.severity,
            status,
            b.product,
            b.component,
            b.summary,
            format_date_short(&b.last_change_time)
        );
    }
    out
}

fn format_bugs_markdown(bugs: &[Bug]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "Found **{}** bugs\n", bugs.len());
    let _ = writeln!(
        out,
        "| Bug | Severity | Status | Component | Summary | Changed |"
    );
    let _ = writeln!(
        out,
        "|-----|----------|--------|-----------|---------|---------|"
    );
    for b in bugs {
        let status = if b.resolution.is_empty() {
            b.status.clone()
        } else {
            format!("{} ({})", b.status, b.resolution)
        };
        let _ = writeln!(
            out,
            "| [{}](https://bugzilla.mozilla.org/show_bug.cgi?id={}) | {} | {} | {}::{} | {} | {} |",
            b.id,
            b.id,
            b.severity,
            status,
            b.product,
            b.component,
            b.summary.replace('|', "\\|"),
            format_date_short(&b.last_change_time)
        );
    }
    out
}

fn format_bugs(bugs: &[Bug], format: &OutputFormat) -> String {
    match format {
        OutputFormat::Compact => format_bugs_compact(bugs),
        OutputFormat::Json => serde_json::to_string_pretty(bugs).unwrap_or_default(),
        OutputFormat::Markdown => format_bugs_markdown(bugs),
    }
}

fn format_bug_detail_compact(b: &Bug, history: &[HistoryEntry]) -> String {
    let mut out = String::new();
    let status = if b.resolution.is_empty() {
        b.status.clone()
    } else {
        format!("{}:{}", b.status, b.resolution)
    };
    let _ = writeln!(out, "Bug {} - {}", b.id, b.summary);
    let _ = writeln!(
        out,
        "Status: {} | Severity: {} | Priority: {}",
        status, b.severity, b.priority
    );
    let _ = writeln!(out, "Product: {} :: {}", b.product, b.component);
    let _ = writeln!(
        out,
        "Type: {} | Assignee: {}",
        b.bug_type,
        format_assignee_short(&b.assigned_to)
    );
    let _ = writeln!(
        out,
        "Created: {} | Changed: {}",
        format_date_short(&b.creation_time),
        format_date_short(&b.last_change_time)
    );
    if !b.keywords.is_empty() {
        let _ = writeln!(out, "Keywords: {}", b.keywords.join(", "));
    }
    if !b.whiteboard.is_empty() {
        let _ = writeln!(out, "Whiteboard: {}", b.whiteboard);
    }
    if !b.cf_crash_signature.is_empty() {
        let _ = writeln!(out, "Crash signature: {}", b.cf_crash_signature);
    }
    if !b.cf_webcompat_priority.is_empty() {
        let _ = writeln!(out, "Webcompat priority: {}", b.cf_webcompat_priority);
    }
    if !b.url.is_empty() {
        let _ = writeln!(out, "URL: {}", b.url);
    }
    if !b.flags.is_empty() {
        let _ = writeln!(out, "Flags: {}", format_flags(&b.flags));
    }
    if !b.depends_on.is_empty() {
        let deps: Vec<String> = b.depends_on.iter().map(|d| d.to_string()).collect();
        let _ = writeln!(out, "Depends on: {}", deps.join(", "));
    }
    if !b.blocks.is_empty() {
        let blocks: Vec<String> = b.blocks.iter().map(|d| d.to_string()).collect();
        let _ = writeln!(out, "Blocks: {}", blocks.join(", "));
    }
    if !b.see_also.is_empty() {
        let _ = writeln!(out, "See also: {}", b.see_also.join(", "));
    }
    if let Some(ref dupe) = b.dupe_of {
        let _ = writeln!(out, "Duplicate of: {}", dupe);
    }
    if !b.duplicates.is_empty() {
        let dupes: Vec<String> = b.duplicates.iter().map(|d| d.to_string()).collect();
        let _ = writeln!(out, "Duplicates: {}", dupes.join(", "));
    }
    if let Some(ref comments) = b.comments {
        let _ = writeln!(out, "\n--- COMMENTS ({}) ---", comments.len());
        for c in comments {
            let _ = writeln!(
                out,
                "\n[Comment #{}] {} ({})",
                c.count,
                format_assignee_short(&c.creator),
                format_date_short(&c.creation_time)
            );
            let _ = writeln!(out, "{}", c.text);
        }
    }
    if !history.is_empty() {
        let _ = writeln!(out, "\n--- HISTORY ({} entries) ---", history.len());
        for entry in history {
            let _ = writeln!(
                out,
                "\n{} - {}",
                format_date_short(&entry.when),
                format_assignee_short(&entry.who)
            );
            for ch in &entry.changes {
                if ch.removed.is_empty() {
                    let _ = writeln!(out, "  {}: +{}", ch.field_name, ch.added);
                } else if ch.added.is_empty() {
                    let _ = writeln!(out, "  {}: -{}", ch.field_name, ch.removed);
                } else {
                    let _ = writeln!(out, "  {}: {} -> {}", ch.field_name, ch.removed, ch.added);
                }
            }
        }
    }
    out
}

fn format_bug_detail_markdown(b: &Bug, history: &[HistoryEntry]) -> String {
    let mut out = String::new();
    let status = if b.resolution.is_empty() {
        b.status.clone()
    } else {
        format!("{} ({})", b.status, b.resolution)
    };
    let _ = writeln!(out, "# Bug {} - {}\n", b.id, b.summary);
    let _ = writeln!(out, "| Field | Value |");
    let _ = writeln!(out, "|-------|-------|");
    let _ = writeln!(out, "| Status | {} |", status);
    let _ = writeln!(out, "| Severity | {} |", b.severity);
    let _ = writeln!(out, "| Priority | {} |", b.priority);
    let _ = writeln!(out, "| Product | {} :: {} |", b.product, b.component);
    let _ = writeln!(out, "| Type | {} |", b.bug_type);
    let _ = writeln!(
        out,
        "| Assignee | {} |",
        format_assignee_short(&b.assigned_to)
    );
    let _ = writeln!(out, "| Created | {} |", format_date_short(&b.creation_time));
    let _ = writeln!(
        out,
        "| Changed | {} |",
        format_date_short(&b.last_change_time)
    );
    if !b.keywords.is_empty() {
        let _ = writeln!(out, "| Keywords | {} |", b.keywords.join(", "));
    }
    if !b.whiteboard.is_empty() {
        let _ = writeln!(out, "| Whiteboard | {} |", b.whiteboard.replace('|', "\\|"));
    }
    if !b.cf_crash_signature.is_empty() {
        let _ = writeln!(out, "| Crash signature | `{}` |", b.cf_crash_signature);
    }
    if !b.cf_webcompat_priority.is_empty() {
        let _ = writeln!(out, "| Webcompat priority | {} |", b.cf_webcompat_priority);
    }
    if !b.url.is_empty() {
        let _ = writeln!(out, "| URL | {} |", b.url);
    }
    if !b.flags.is_empty() {
        let _ = writeln!(out, "| Flags | {} |", format_flags(&b.flags));
    }
    if !b.depends_on.is_empty() {
        let deps: Vec<String> = b
            .depends_on
            .iter()
            .map(|d| {
                format!(
                    "[{}](https://bugzilla.mozilla.org/show_bug.cgi?id={})",
                    d, d
                )
            })
            .collect();
        let _ = writeln!(out, "| Depends on | {} |", deps.join(", "));
    }
    if !b.blocks.is_empty() {
        let blocks: Vec<String> = b
            .blocks
            .iter()
            .map(|d| {
                format!(
                    "[{}](https://bugzilla.mozilla.org/show_bug.cgi?id={})",
                    d, d
                )
            })
            .collect();
        let _ = writeln!(out, "| Blocks | {} |", blocks.join(", "));
    }
    if !b.see_also.is_empty() {
        let _ = writeln!(out, "| See also | {} |", b.see_also.join(", "));
    }
    if let Some(ref dupe) = b.dupe_of {
        let _ = writeln!(
            out,
            "| Duplicate of | [{}](https://bugzilla.mozilla.org/show_bug.cgi?id={}) |",
            dupe, dupe
        );
    }
    if !b.duplicates.is_empty() {
        let dupes: Vec<String> = b
            .duplicates
            .iter()
            .map(|d| {
                format!(
                    "[{}](https://bugzilla.mozilla.org/show_bug.cgi?id={})",
                    d, d
                )
            })
            .collect();
        let _ = writeln!(out, "| Duplicates | {} |", dupes.join(", "));
    }
    if let Some(ref comments) = b.comments {
        let _ = writeln!(out, "\n## Comments ({})\n", comments.len());
        for c in comments {
            let _ = writeln!(
                out,
                "### Comment #{} - {} ({})\n",
                c.count,
                format_assignee_short(&c.creator),
                format_date_short(&c.creation_time)
            );
            let _ = writeln!(out, "{}\n", c.text);
        }
    }
    if !history.is_empty() {
        let _ = writeln!(out, "\n## History ({} entries)\n", history.len());
        let _ = writeln!(out, "| Date | Who | Field | Removed | Added |");
        let _ = writeln!(out, "|------|-----|-------|---------|-------|");
        for entry in history {
            for ch in &entry.changes {
                let _ = writeln!(
                    out,
                    "| {} | {} | {} | {} | {} |",
                    format_date_short(&entry.when),
                    format_assignee_short(&entry.who),
                    ch.field_name,
                    ch.removed.replace('|', "\\|"),
                    ch.added.replace('|', "\\|"),
                );
            }
        }
    }
    out
}

fn format_bug_detail(b: &Bug, history: &[HistoryEntry], format: &OutputFormat) -> String {
    match format {
        OutputFormat::Compact => format_bug_detail_compact(b, history),
        OutputFormat::Json => {
            if history.is_empty() {
                serde_json::to_string_pretty(b).unwrap_or_default()
            } else {
                let mut val = serde_json::to_value(b).unwrap_or_default();
                if let Some(obj) = val.as_object_mut() {
                    obj.insert(
                        "history".into(),
                        serde_json::to_value(history).unwrap_or_default(),
                    );
                }
                serde_json::to_string_pretty(&val).unwrap_or_default()
            }
        }
        OutputFormat::Markdown => format_bug_detail_markdown(b, history),
    }
}

// -- Commands --

async fn cmd_search(args: SearchArgs) -> Result<()> {
    let client = client()?;
    let mut params: Vec<(String, String)> = Vec::new();

    // Build quicksearch if we have free-text terms and no comment search
    if !args.terms.is_empty() && !args.comments {
        let qs = args.terms.join(" ");
        params.push(("quicksearch".into(), qs));
    }

    // Structured filters via boolean chart
    let mut chart = ChartBuilder::new();

    if !args.terms.is_empty() && args.comments {
        // Search in both summary and comments using a grouped OR
        let terms_str = args.terms.join(" ");
        chart.open_group("OR");
        chart.add("short_desc", "allwordssubstr", &terms_str);
        chart.add("longdesc", "allwordssubstr", &terms_str);
        chart.close_group();
    }

    if let Some(ref product) = args.product {
        params.push(("product".into(), product.clone()));
    }
    if let Some(ref component) = args.component {
        params.push(("component".into(), component.clone()));
    }

    let statuses = status_to_list(&args.status);
    for s in &statuses {
        params.push(("bug_status".into(), s.to_string()));
    }

    if let Some(ref sev) = args.severity {
        for s in sev.split(',') {
            params.push(("severity".into(), s.trim().to_string()));
        }
    }

    if let Some(ref t) = args.bug_type {
        params.push(("type".into(), t.clone()));
    }

    if let Some(ref kw) = args.keywords {
        chart.add("keywords", "anywordssubstr", kw);
    }

    if let Some(ref a) = args.assignee {
        params.push(("assigned_to".into(), a.clone()));
    }

    if let Some(ref wb) = args.whiteboard {
        chart.add("status_whiteboard", "substring", wb);
    }

    if let Some(ref cs) = args.crash_signature {
        chart.add("cf_crash_signature", "substring", cs);
    }

    if let Some(ref flag) = args.flag {
        chart.add("flagtypes.name", "substring", flag);
    }

    if let Some(ref d) = args.created_after {
        chart.add("creation_ts", "greaterthan", d);
    }

    if let Some(ref d) = args.changed_after {
        chart.add("delta_ts", "greaterthan", d);
    }

    params.extend(chart.into_params());

    params.push(("limit".into(), args.limit.to_string()));

    if args.offset > 0 {
        params.push(("offset".into(), args.offset.to_string()));
    }

    if let Some(ref s) = args.sort {
        params.push(("order".into(), s.clone()));
    }

    if args.count {
        params.push(("count_only".into(), "1".into()));
    }

    if !args.count {
        params.push((
            "include_fields".into(),
            "id,summary,status,resolution,product,component,severity,priority,assigned_to,keywords,last_change_time,creation_time,cf_crash_signature,cf_webcompat_priority,whiteboard,type,flags".into(),
        ));
    }

    let url = reqwest::Url::parse_with_params(&format!("{BMO_BASE}/bug"), &params)
        .context("failed to build URL")?;

    let resp: BugListResponse = client
        .get(url)
        .send()
        .await
        .context("BMO request failed")?
        .error_for_status()
        .context("BMO returned error")?
        .json()
        .await
        .context("failed to parse BMO response")?;

    if args.count {
        println!("{}", resp.bug_count.unwrap_or(0));
        return Ok(());
    }

    let bugs = resp.bugs.unwrap_or_default();
    print!("{}", format_bugs(&bugs, &args.format));
    Ok(())
}

async fn cmd_similar(args: SimilarArgs) -> Result<()> {
    let client = client()?;

    // First fetch the source bug to get its summary and product
    let bug_url = format!(
        "{BMO_BASE}/bug/{}?include_fields=id,summary,product,component,keywords",
        args.bug_id
    );
    let bug_resp: BugListResponse = client
        .get(&bug_url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let source_bugs = bug_resp.bugs.unwrap_or_default();
    let source = source_bugs.first().context("bug not found")?;

    // Use possible_duplicates endpoint
    let mut params: Vec<(String, String)> = vec![
        ("summary".into(), source.summary.clone()),
        ("limit".into(), args.limit.to_string()),
        (
            "include_fields".into(),
            "id,summary,status,resolution,product,component,severity,priority,last_change_time,keywords".into(),
        ),
    ];
    if !source.product.is_empty() {
        params.push(("product".into(), source.product.clone()));
    }

    let dup_url =
        reqwest::Url::parse_with_params(&format!("{BMO_BASE}/bug/possible_duplicates"), &params)?;
    let dup_resp: PossibleDuplicatesResponse = client
        .get(dup_url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let mut bugs: Vec<Bug> = dup_resp
        .bugs
        .into_iter()
        .filter(|b| b.id != args.bug_id)
        .collect();
    bugs.truncate(args.limit as usize);

    println!("Similar to bug {} - {}", source.id, source.summary);
    print!("{}", format_bugs(&bugs, &args.format));
    Ok(())
}

async fn cmd_duplicates(args: DuplicatesArgs) -> Result<()> {
    let client = client()?;

    // Fetch the source bug to get its duplicates list
    let bug_url = format!(
        "{BMO_BASE}/bug/{}?include_fields=id,summary,duplicates",
        args.bug_id
    );
    let bug_resp: BugListResponse = client
        .get(&bug_url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let source = bug_resp.bugs.unwrap_or_default();
    let source = source.first().context("bug not found")?;

    if source.duplicates.is_empty() {
        println!("No bugs marked as duplicate of bug {}", args.bug_id);
        return Ok(());
    }

    // Fetch the duplicate bugs
    let ids: Vec<String> = source.duplicates.iter().map(|id| id.to_string()).collect();
    let mut params: Vec<(String, String)> = vec![(
        "include_fields".into(),
        "id,summary,status,resolution,product,component,severity,last_change_time".into(),
    )];
    for id in &ids {
        params.push(("id".into(), id.clone()));
    }
    params.push(("limit".into(), args.limit.to_string()));

    let url = reqwest::Url::parse_with_params(&format!("{BMO_BASE}/bug"), &params)?;
    let resp: BugListResponse = client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let bugs = resp.bugs.unwrap_or_default();

    println!("Bugs marked as duplicate of bug {}", args.bug_id);
    print!("{}", format_bugs(&bugs, &args.format));
    Ok(())
}

async fn cmd_get(args: GetArgs) -> Result<()> {
    let client = client()?;

    let url = format!("{BMO_BASE}/bug/{}", args.bug_id);
    let resp: BugListResponse = client
        .get(&url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let bugs = resp.bugs.unwrap_or_default();
    let mut bug = bugs.into_iter().next().context("bug not found")?;

    if args.comments {
        let comments_url = format!("{BMO_BASE}/bug/{}/comment", args.bug_id);
        let comments_resp: CommentsResponse = client
            .get(&comments_url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        if let Some(cb) = comments_resp.bugs.get(&args.bug_id.to_string()) {
            bug.comments = Some(cb.comments.clone());
        }
    }

    let history = if args.history {
        let history_url = format!("{BMO_BASE}/bug/{}/history", args.bug_id);
        let history_resp: HistoryResponse = client
            .get(&history_url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        history_resp
            .bugs
            .into_iter()
            .next()
            .map(|h| h.history)
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    print!("{}", format_bug_detail(&bug, &history, &args.format));
    Ok(())
}

async fn cmd_advanced(args: AdvancedArgs) -> Result<()> {
    let client = client()?;
    let mut params: Vec<(String, String)> = Vec::new();

    let mut chart = ChartBuilder::new();
    for filter in &args.filters {
        let parts: Vec<&str> = filter.splitn(3, ':').collect();
        if parts.len() != 3 {
            bail!("filter must be 'field:operator:value', got '{}'", filter);
        }
        chart.add(parts[0], parts[1], parts[2]);
    }

    if args.or {
        params.push(("j_top".into(), "OR".into()));
    }

    params.extend(chart.into_params());

    params.push(("limit".into(), args.limit.to_string()));

    if args.offset > 0 {
        params.push(("offset".into(), args.offset.to_string()));
    }

    if args.count {
        params.push(("count_only".into(), "1".into()));
    }

    if let Some(ref fields) = args.include_fields {
        params.push(("include_fields".into(), fields.clone()));
    } else if !args.count {
        params.push((
            "include_fields".into(),
            "id,summary,status,resolution,product,component,severity,priority,last_change_time"
                .into(),
        ));
    }

    let url = reqwest::Url::parse_with_params(&format!("{BMO_BASE}/bug"), &params)?;
    let resp: BugListResponse = client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    if args.count {
        println!("{}", resp.bug_count.unwrap_or(0));
        return Ok(());
    }

    let bugs = resp.bugs.unwrap_or_default();
    print!("{}", format_bugs(&bugs, &args.format));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_bug(id: u64, summary: &str, status: &str, resolution: &str) -> Bug {
        Bug {
            id,
            summary: summary.into(),
            status: status.into(),
            resolution: resolution.into(),
            product: "Firefox".into(),
            component: "General".into(),
            severity: "S2".into(),
            priority: "P1".into(),
            assigned_to: "dev@mozilla.org".into(),
            keywords: vec![],
            last_change_time: "2025-03-15T12:00:00Z".into(),
            creation_time: "2025-01-01T00:00:00Z".into(),
            cf_crash_signature: String::new(),
            cf_webcompat_priority: String::new(),
            whiteboard: String::new(),
            bug_type: "defect".into(),
            url: String::new(),
            depends_on: vec![],
            blocks: vec![],
            see_also: vec![],
            dupe_of: None,
            duplicates: vec![],
            flags: vec![],
            comments: None,
        }
    }

    // -- status_to_list --

    #[test]
    fn status_to_list_open() {
        let result = status_to_list("open");
        assert_eq!(result, vec!["NEW", "ASSIGNED", "REOPENED", "UNCONFIRMED"]);
    }

    #[test]
    fn status_to_list_closed() {
        let result = status_to_list("closed");
        assert_eq!(result, vec!["RESOLVED", "VERIFIED", "CLOSED"]);
    }

    #[test]
    fn status_to_list_all() {
        let result = status_to_list("all");
        assert!(result.is_empty());
    }

    #[test]
    fn status_to_list_case_insensitive() {
        assert_eq!(status_to_list("Open"), status_to_list("open"));
        assert_eq!(status_to_list("CLOSED"), status_to_list("closed"));
    }

    #[test]
    fn status_to_list_custom() {
        let result = status_to_list("NEW,ASSIGNED");
        assert_eq!(result, vec!["NEW", "ASSIGNED"]);
    }

    // -- format_date_short --

    #[test]
    fn format_date_short_iso() {
        assert_eq!(format_date_short("2025-03-15T12:00:00Z"), "2025-03-15");
    }

    #[test]
    fn format_date_short_exact_10() {
        assert_eq!(format_date_short("2025-03-15"), "2025-03-15");
    }

    #[test]
    fn format_date_short_too_short() {
        assert_eq!(format_date_short("2025"), "2025");
    }

    #[test]
    fn format_date_short_empty() {
        assert_eq!(format_date_short(""), "");
    }

    // -- format_assignee_short --

    #[test]
    fn format_assignee_short_email() {
        assert_eq!(format_assignee_short("dev@mozilla.org"), "dev");
    }

    #[test]
    fn format_assignee_short_no_at() {
        assert_eq!(format_assignee_short("nobody"), "nobody");
    }

    #[test]
    fn format_assignee_short_empty() {
        assert_eq!(format_assignee_short(""), "");
    }

    // -- format_flags --

    #[test]
    fn format_flags_empty() {
        assert_eq!(format_flags(&[]), "");
    }

    #[test]
    fn format_flags_single() {
        let flags = vec![Flag {
            name: "needinfo".into(),
            status: "?".into(),
            requestee: None,
        }];
        assert_eq!(format_flags(&flags), "needinfo?");
    }

    #[test]
    fn format_flags_with_requestee() {
        let flags = vec![Flag {
            name: "needinfo".into(),
            status: "?".into(),
            requestee: Some("dev@mozilla.org".into()),
        }];
        assert_eq!(format_flags(&flags), "needinfo?(dev@mozilla.org)");
    }

    #[test]
    fn format_flags_multiple() {
        let flags = vec![
            Flag { name: "review".into(), status: "+".into(), requestee: None },
            Flag { name: "needinfo".into(), status: "?".into(), requestee: None },
        ];
        assert_eq!(format_flags(&flags), "review+, needinfo?");
    }

    // -- ChartBuilder --

    #[test]
    fn chart_builder_single_filter() {
        let mut chart = ChartBuilder::new();
        chart.add("product", "equals", "Firefox");
        let params = chart.into_params();
        assert_eq!(params, vec![
            ("f0".into(), "product".into()),
            ("o0".into(), "equals".into()),
            ("v0".into(), "Firefox".into()),
        ]);
    }

    #[test]
    fn chart_builder_multiple_filters() {
        let mut chart = ChartBuilder::new();
        chart.add("product", "equals", "Firefox");
        chart.add("severity", "equals", "S1");
        let params = chart.into_params();
        assert_eq!(params.len(), 6);
        assert_eq!(params[0], ("f0".into(), "product".into()));
        assert_eq!(params[3], ("f1".into(), "severity".into()));
    }

    #[test]
    fn chart_builder_grouped() {
        let mut chart = ChartBuilder::new();
        chart.open_group("OR");
        chart.add("short_desc", "substring", "crash");
        chart.add("longdesc", "substring", "crash");
        chart.close_group();
        let params = chart.into_params();
        assert_eq!(params[0], ("f0".into(), "OP".into()));
        assert_eq!(params[1], ("j0".into(), "OR".into()));
        assert_eq!(params[2], ("f1".into(), "short_desc".into()));
        assert_eq!(params[5], ("f2".into(), "longdesc".into()));
        assert_eq!(params[8], ("f3".into(), "CP".into()));
    }

    // -- format_bugs_compact --

    #[test]
    fn format_bugs_compact_empty() {
        let output = format_bugs_compact(&[]);
        assert!(output.starts_with("FOUND 0 bugs"));
    }

    #[test]
    fn format_bugs_compact_with_resolution() {
        let bug = make_bug(123, "test bug", "RESOLVED", "FIXED");
        let output = format_bugs_compact(&[bug]);
        assert!(output.contains("RESOLVED:FIXED"));
        assert!(output.contains("123"));
        assert!(output.contains("test bug"));
    }

    #[test]
    fn format_bugs_compact_open() {
        let bug = make_bug(456, "open bug", "NEW", "");
        let output = format_bugs_compact(&[bug]);
        assert!(output.contains("|NEW|"));
        assert!(!output.contains("NEW:"));
    }

    // -- format_bugs_markdown --

    #[test]
    fn format_bugs_markdown_has_table_header() {
        let output = format_bugs_markdown(&[]);
        assert!(output.contains("| Bug |"));
        assert!(output.contains("|--"));
    }

    #[test]
    fn format_bugs_markdown_links_to_bugzilla() {
        let bug = make_bug(789, "linked bug", "NEW", "");
        let output = format_bugs_markdown(&[bug]);
        assert!(output.contains("https://bugzilla.mozilla.org/show_bug.cgi?id=789"));
    }

    #[test]
    fn format_bugs_markdown_escapes_pipes() {
        let bug = make_bug(1, "summary | with pipe", "NEW", "");
        let output = format_bugs_markdown(&[bug]);
        assert!(output.contains("summary \\| with pipe"));
    }

    // -- format_bug_detail --

    #[test]
    fn format_bug_detail_compact_basic() {
        let bug = make_bug(100, "detail bug", "NEW", "");
        let output = format_bug_detail_compact(&bug, &[]);
        assert!(output.contains("Bug 100 - detail bug"));
        assert!(output.contains("Status: NEW"));
        assert!(output.contains("Assignee: dev"));
        assert!(output.contains("Product: Firefox :: General"));
    }

    #[test]
    fn format_bug_detail_compact_with_comments() {
        let mut bug = make_bug(100, "detail bug", "NEW", "");
        bug.comments = Some(vec![Comment {
            id: 1,
            text: "first comment".into(),
            creator: "user@mozilla.org".into(),
            creation_time: "2025-01-02T00:00:00Z".into(),
            count: 0,
        }]);
        let output = format_bug_detail_compact(&bug, &[]);
        assert!(output.contains("COMMENTS (1)"));
        assert!(output.contains("first comment"));
        assert!(output.contains("user"));
    }

    #[test]
    fn format_bug_detail_compact_with_history() {
        let bug = make_bug(100, "detail bug", "NEW", "");
        let history = vec![HistoryEntry {
            when: "2025-02-01T00:00:00Z".into(),
            who: "editor@mozilla.org".into(),
            changes: vec![HistoryChange {
                field_name: "status".into(),
                removed: "NEW".into(),
                added: "ASSIGNED".into(),
            }],
        }];
        let output = format_bug_detail_compact(&bug, &history);
        assert!(output.contains("HISTORY (1 entries)"));
        assert!(output.contains("status: NEW -> ASSIGNED"));
    }

    #[test]
    fn format_bug_detail_json_includes_history() {
        let bug = make_bug(100, "json bug", "NEW", "");
        let history = vec![HistoryEntry {
            when: "2025-02-01T00:00:00Z".into(),
            who: "editor@mozilla.org".into(),
            changes: vec![],
        }];
        let output = format_bug_detail(&bug, &history, &OutputFormat::Json);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(parsed.get("history").is_some());
        assert_eq!(parsed["id"], 100);
    }

    #[test]
    fn format_bug_detail_json_no_history_key_when_empty() {
        let bug = make_bug(100, "json bug", "NEW", "");
        let output = format_bug_detail(&bug, &[], &OutputFormat::Json);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(parsed.get("history").is_none());
    }
}

fn is_llm_environment() -> bool {
    let has = |k| std::env::var(k).is_ok_and(|v| !v.is_empty());
    has("CLAUDECODE") || has("CODEX_SANDBOX") || has("GEMINI_CLI") || has("OPENCODE")
}

fn print_llm_help() {
    print!(concat!(
        "bmo-search: Search Mozilla's Bugzilla (BMO)\n",
        "search [TERMS...] [--product P] [--component C] [--status open|closed|all(open)] [--severity S1,S2,...] [--type defect|enhancement|task] [--keywords K] [--assignee A] [--whiteboard W] [--crash-signature S] [--flag F] [--created-after DATE] [--changed-after DATE] [--comments] [--limit N(20)] [--offset N(0)] [--sort S] [--count] [--format compact|json|markdown]\n",
        "get <BUG_ID> [--comments] [--history] [--format compact|json|markdown]\n",
        "similar <BUG_ID> [--limit N(20)] [--format compact|json|markdown]\n",
        "duplicates <BUG_ID> [--limit N(20)] [--format compact|json|markdown]\n",
        "advanced -f \"field:op:value\" [-f ...] [--or] [--include-fields F] [--limit N(20)] [--offset N(0)] [--count] [--format compact|json|markdown]\n",
        "Auth: set BMO_API_KEY env var for restricted bugs\n",
        "Ex: search \"webcompat navigation\" --product Firefox --status open\n",
        "Ex: search --severity S1,S2 --created-after 2025-01-01 --count\n",
        "Ex: get 1234567 --comments --history --format json\n",
        "Ex: advanced -f \"product:equals:Firefox\" -f \"severity:equals:S1\" --or\n",
    ));
}

#[tokio::main]
async fn main() -> Result<()> {
    if is_llm_environment() && std::env::args().any(|a| a == "--help" || a == "-h") {
        print_llm_help();
        return Ok(());
    }
    let cli = Cli::parse();
    match cli.command {
        Command::Search(args) => cmd_search(*args).await,
        Command::Similar(args) => cmd_similar(args).await,
        Command::Duplicates(args) => cmd_duplicates(args).await,
        Command::Get(args) => cmd_get(args).await,
        Command::Advanced(args) => cmd_advanced(args).await,
    }
}
