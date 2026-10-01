# grove

Your GitHub repos, grown as a forest of bonsai trees in the terminal.

![grove growing a forest of bonsai trees in the terminal](assets/grove.gif)

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

## Connect your account

grove shows the repos of whichever GitHub account it is signed in as. There is no
separate login; it reuses one of these:

**GitHub CLI (easiest).** If you use [`gh`](https://cli.github.com):

    gh auth login
    grove

**Personal access token.** Create one under GitHub → Settings → Developer settings →
Personal access tokens, then add it to your shell config (e.g. `~/.zshrc`):

    export GITHUB_TOKEN=ghp_...

- Public repos only: a token with no extra permissions is enough.
- To include private repos, give the token read access to them (fine-grained token:
  Metadata, Contents, Commit statuses and Checks, all read-only).

If both are set, `GITHUB_TOKEN` wins. A token is needed even for viewing someone
else's public repos with `--user` / `--org`, because GitHub's API requires one.

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
