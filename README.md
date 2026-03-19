# bmo-search

A CLI for searching Mozilla's Bugzilla (BMO) from the terminal.

## Install

### Via cargo-binstall (recommended)

```
cargo binstall bmo-search
```

### Via cargo

```
cargo install bmo-search
```

### From source

```
git clone https://github.com/jnjaeschke/bmo-search.git
cd bmo-search
cargo install --path .
```

## Usage

The binary is available as `bmo-search`. All commands support `--format compact` (default), `--format json`, and `--format markdown` output.

### Search

Free-text search with optional structured filters:

```
bmo-search search "webcompat navigation"
bmo-search search --product Firefox --component Networking --status open
bmo-search search "crash" --severity S1,S2 --created-after 2025-01-01
bmo-search search "layout bug" --comments --limit 50
bmo-search search --keywords regression --assignee someone@mozilla.org
bmo-search search --whiteboard "[webcompat]" --flag "needinfo?"
```

#### Filters

| Flag                | Description                                          |
| ------------------- | ---------------------------------------------------- |
| `--product`         | Product name                                         |
| `--component`       | Component name                                       |
| `--status`          | `open`, `closed`, `all`, or comma-separated statuses |
| `--severity`        | S1–S4 or comma-separated values                      |
| `--type`            | `defect`, `enhancement`, or `task`                   |
| `--keywords`        | Keyword filter                                       |
| `--assignee`        | Assignee email                                       |
| `--whiteboard`      | Whiteboard substring                                 |
| `--crash-signature` | Crash signature substring                            |
| `--flag`            | Flag filter (e.g. `needinfo?`, `review+`)            |
| `--created-after`   | ISO date (e.g. `2025-01-01`)                         |
| `--changed-after`   | ISO date                                             |
| `--comments`        | Also search in comment text (slower)                 |
| `--limit`           | Max results (default: 20)                            |
| `--offset`          | Skip first N results                                 |
| `--sort`            | Sort order                                           |
| `--count`           | Only return the count of matching bugs               |

### Get

Fetch a single bug with optional comments and history:

```
bmo-search get 1234567
bmo-search get 1234567 --comments
bmo-search get 1234567 --comments --history --format json
```

### Similar

Find bugs similar to a given bug:

```
bmo-search similar 1234567
```

### Duplicates

Find bugs marked as duplicates of a given bug:

```
bmo-search duplicates 1234567
```

### Advanced

Raw boolean chart queries using `field:operator:value` triplets:

```
bmo-search advanced -f "product:equals:Firefox" -f "severity:equals:S1"
bmo-search advanced -f "assigned_to:equals:someone@mozilla.org" -f "status:notequals:RESOLVED" --or
```

## Authentication

Public BMO data is accessible without authentication. For access to restricted bugs, set the `BMO_API_KEY` environment variable:

```
export BMO_API_KEY=your-api-key
```

You can generate an API key at https://bugzilla.mozilla.org/userprefs.cgi?tab=apikey.

### Security considerations

If your API key grants access to security-sensitive bugs, be mindful of how you use this tool:

- Use a **read-only token scoped to the minimum necessary access**.
- Avoid piping output from security bugs into files, logs, or shared systems without review.
- If using this tool from an LLM agent or automation, be aware that the agent could query and output security-sensitive bug details. Restrict the token's permissions accordingly.

## Contributing

Code must pass the following quality gates before merging:

```sh
cargo fmt -- --check
cargo clippy -- -D warnings
cargo test
```

These checks run automatically in CI on every push and pull request.

## License

[MIT](LICENSE)
