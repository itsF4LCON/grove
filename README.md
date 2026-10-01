# grove

Your GitHub repos, grown as a forest of bonsai trees in the terminal.

Every repo becomes a procedurally grown tree, always the same shape for the same repo:

| you see | it means |
|---|---|
| tree size | total commits and repo age |
| leaf density | commits in the last 90 days |
| season | days since last push: spring/summer → autumn (2 mo) → late autumn (6 mo) → bare winter (1 yr or archived) |
| ❀ ✿ blossoms | stars |
| red `x` blight | failing CI |
| `\ /` seedling | empty repo |

## Install

    cargo install --path .

Auth: uses `GITHUB_TOKEN`, or falls back to `gh auth token`.

## Use

    grove                      # interactive forest (←/→, ⏎ details, o open, s sort, r refresh, ? legend, q quit)
    grove --print              # one snapshot, great in ~/.zshrc
    grove --org ratatui        # someone else's forest (or --user NAME)
    grove --include-archived   # show archived repos as winter trees
    grove --demo               # sample forest, no network
    grove --still              # no animation

Data is cached for 15 minutes in `~/.cache/grove/repos.json`; `--refresh` bypasses it.
`--print` uses a 3-second network timeout when a cache exists, so it never stalls your shell.

The growth algorithm is an independent implementation inspired by cbonsai.
