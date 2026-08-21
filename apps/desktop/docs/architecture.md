# Architecture

The architecture lives on the web, one page per subsystem, at
**[recast.li/architecture](https://recast.li/architecture)**.

The source is markdown in this repo, at
[`apps/web/content/architecture/`](../../web/content/architecture/), so it is
readable as text and rendered as pages from the same file. There is no second
copy to drift.

Each page opens with what goes in, what comes out, which files to start at, and
the invariants that subsystem cannot break. Those facts are frontmatter, so they
are machine-readable as well as scannable.

For background on the browser APIs the editor is built on, see
[webcodecs-webgl-primer.md](webcodecs-webgl-primer.md).

## Keeping it current

When you change a module a page describes, update that page. Two things matter
more than the prose:

1. The `entrypoints` list, which is how anyone finds the code.
2. The `invariants` list, which is what stops the next change breaking it.

A page whose frontmatter is missing or malformed fails the web build, so those
two lists cannot silently rot into nothing.
