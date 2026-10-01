# `grove`: your GitHub repos as a cbonsai-style forest

## Context
You want a Linux CLI tool that is both useful and genuinely beautiful. It shows every repo on your GitHub account as a procedurally grown, cbonsai-quality ASCII tree. The trees stand side by side in a forest, and each tree's shape and color come from that repo's real activity. That makes it a quick health overview of all your projects (useful) and something nice to leave open in a pane (aesthetic).

Decisions so far:
- **Stack:** Rust + ratatui
- **Data:** GitHub API only
- **Modes:** an interactive TUI, plus `--print` for a static snapshot
- **Layout:** a forest of trees next to each other
- **Quality bar:** cbonsai

Project root: `/home/sol/PROJ/CLI` (currently empty). The binary is called `grove`.

## Data (verified against the live API)
- Auth: use `GITHUB_TOKEN` if set, otherwise fall back to `gh auth token`. `gh` is installed and logged in as itsF4LCON.
- I ran one paginated GraphQL query (`viewer.repositories`, `ownerAffiliations: OWNER`, `isFork: false`, ordered by PUSHED_AT) and it returns everything needed:
  - name/url/description, stargazerCount, forkCount, isArchived, createdAt, pushedAt
  - primaryLanguage{name,color}, open issues and PR counts
  - `defaultBranchRef.target` with `history.totalCount`, `recent: history(since: now-90d).totalCount`, and `statusCheckRollup.state`
  - The account currently has 36 non-fork repos.
- The model must handle these nullable fields:
  - `primaryLanguage` (null on some repos)
  - `statusCheckRollup` (null on most repos)
  - `defaultBranchRef` (null on an empty repo, which renders as a seedling)
- Defaults: only repos you own, with forks and archived repos excluded. Flags: `--include-forks`, `--include-archived`, `--user <login>`, `--org <name>`.
- Cache: `~/.cache/grove/repos.json` with a 15-minute TTL. `--refresh` forces a fetch.
  - `--print` never blocks a shell greeting: it uses the cache, falls back to a 3-second network timeout, and prints stale data if offline.

## Repo stats → tree (the visual mapping)
| Signal | Effect on the tree |
|---|---|
| Hash of `nameWithOwner` | RNG seed, so each repo always grows the same recognizable shape |
| Total commits and age | Trunk height and thickness, and overall life/size (a young repo is a sapling, an old busy one a big tree) |
| Commits in the last 90 days | Leaf density |
| Days since last push | Season: spring/summer greens when active → autumn orange/red after ~2 months → sparse brown → bare winter branches after ~1 year |
| Stars | Blossoms (`❀` / `✿` in pink and white), scaled logarithmically |
| CI failing | Red blight specks on the leaves (no change when CI is null or passing) |
| Primary language | Slight tint on the leaf hue, plus a colored dot next to the name using GitHub's language color |
| Empty repo | A seedling sprout |

**Pots vs ground:** in a forest the trees grow from a **shared ground line** of soil, moss and grass texture, with each repo name under its trunk, instead of each sitting in a cbonsai pot. Tell me if you'd rather keep the pots.

## Making it beautiful
- **Growth algorithm:** an independent Rust reimplementation of the *idea* behind cbonsai. cbonsai is GPL-3.0, and none of its code is ported.
  - Branches are recursive, with a life counter and types: trunk, shootLeft, shootRight, dying, dead.
  - Each branch drifts by a random dx/dy biased by its type.
  - Branch characters (`/ \ | _ ~`) are chosen from the direction of travel.
  - Leaf clusters (`& * ' . ~`) sprout on dying and dead branches.
- **Color:** truecolor gradients when `COLORTERM` is truecolor (your terminal supports it), falling back to 256 colors.
  - Leaves get per-cell jittered shades within a season palette.
  - Bark is brown with a bold/dim mix.
- **Depth:** sizes vary, so the skyline is uneven. Neighboring canopies may overlap. Trees are drawn back to front, and the non-selected ones are slightly dimmed.
- **Glyphs:** single-width only (no emoji), with `unicode-width` used defensively.
- **Animation (TUI):** trees grow in step by step on load, like cbonsai's live mode. Autumn trees shed leaves now and then and petals drift from blossoming trees. `--still` turns the ambient animation off.

## Interaction
- `grove` opens the full-screen forest.
  - `←/→` or `h/l` select a tree, and the forest scrolls horizontally.
  - `Enter` opens a detail panel: description, language, stars and forks, open PRs and issues, CI, last push, commits in the last 90 days.
  - `o` opens the repo with `xdg-open`.
  - `s` cycles the sort order: recent push, activity, stars, name.
  - `r` refreshes, `?` shows a legend explaining the visual mapping, and `q` quits.
- `grove --print [--width N]` renders the forest once to stdout with ANSI colors and exits.

## Module layout (`src/`)
- `main.rs`: clap CLI and mode dispatch
- `github.rs`: auth, GraphQL query, pagination, serde types → `RepoStats`
- `cache.rs`: JSON cache with TTL
- `mapping.rs`: `RepoStats` → `TreeParams` (seed, life, leaf density, season, blossoms, blight, tint)
- `tree/rng.rs`: small seeded PRNG (deterministic)
- `tree/grow.rs`: growth algorithm → ordered `Vec<CellWrite>`, so animation is a replay of the growth steps
- `tree/palette.rs`: season palettes, truecolor and 256-color fallback
- `forest.rs`: lays trees out on a shared canvas (x offsets, overlap, z-order, ground line)
- `canvas.rs`: a cell grid (char, fg, modifiers) → ratatui `Buffer` or ANSI string
- `ui/app.rs`, `ui/view.rs`: TUI state, input, detail panel, ambient particles
- `print.rs`: static ANSI output

Dependencies: current ratatui, crossterm, clap (derive), reqwest (blocking, rustls), serde/serde_json, dirs, unicode-width, and insta (dev). Versions are resolved with `cargo add` rather than pinned from memory.

## Build order (beauty first)
1. `git init`, save this design to `docs/superpowers/specs/2026-10-01-grove-design.md`, and commit. Then turn it into a detailed task plan with the writing-plans skill.
2. **Tree generator + `--print` from synthetic stats** (a fixed set of fake repos covering every season, blossoms, blight and a seedling).
   - **Visual checkpoint:** you look at the output and approve the look before anything else is built. I'll iterate on the algorithm and palettes until it's genuinely beautiful.
3. GitHub fetch, cache, and the real `--print`.
4. TUI: forest scrolling, selection, detail panel, growth animation, ambient leaves and petals, legend.

## Verification
- `cargo test`:
  - Mapping unit tests (seasons, thresholds, nullable fields)
  - Determinism (same seed gives an identical canvas)
  - insta snapshots of a few trees with color stripped
  - GraphQL parsing from a fixture JSON captured from the real API, including null rollup, language and defaultBranchRef
  - Cache TTL behavior
- `cargo clippy -- -D warnings`
- Manual checks:
  - `cargo run -- --print` in your terminal at several widths
  - `cargo run` to try scrolling, Enter, `o`, `s`, `r`, `?`
  - Run offline with a warm cache, so `--print` still prints quickly
  - Check that the forest looks right against your 36 real repos
