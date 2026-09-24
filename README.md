# diff

Find the differences between two sequences or texts, for
[Meadow](https://github.com/mcdearman/meadow). It offers Myers, patience and
LCS diffs, unified diffs like `diff -u` and `git diff` print, and inline
(word-level) changes.

This package is a port of Rust's [`similar`](https://github.com/mitsuhiko/similar)
2.7.0 with its `text`, `inline` and `unicode` features. It finds the same
operations, groups them into the same hunks and prints the same text.

## Install

```sh
meadow add mcdearman/Diff
```

## Use

```meadow
use Diff (fromLines, unifiedDiff, unifiedDiffText, diffWordParts, Myers)

def main =
  ( unifiedDiffText (unifiedDiff (fromLines "a\nb\nc\n" "a\nB\nc\n")),
    -- "@@ -1,3 +1,3 @@\n a\n-b\n+B\n c\n"
    diffWordParts Myers "one two" "one three"
    -- [(Equal, "one "), (Delete, "two"), (Insert, "three")]
  )
```

### Sequences

`captureDiffSlices alg old new` diffs two arrays of any type with equality and
returns a vector of `DiffOp`s:

| operation | meaning |
|---|---|
| `Equal oldIndex newIndex len` | `len` elements are the same on both sides |
| `Delete oldIndex oldLen newIndex` | elements were removed from the old side |
| `Insert oldIndex newIndex newLen` | elements were added on the new side |
| `Replace oldIndex oldLen newIndex newLen` | old elements were replaced by new ones |

`alg` is `Myers`, `Patience` or `Lcs`. `captureDiff alg old oa ob new na nb`
diffs just the ranges `oa..ob` and `na..nb`. `groupDiffOps ops n` splits the
operations into hunks with `n` elements of context. `getDiffRatio ops oldLen
newLen` says how alike the two sides are, from 0 to 1.

For each operation:

- `opTag`, `oldRange` and `newRange` describe it;
- `changesOf op old new` lists its elements one by one, as `Change` records
  with a `tag`, `oldIndex`, `newIndex` and `value`;
- `slicesOf op old new` gives them as runs.

### Text

A `TextDiff` is two texts cut into tokens, together with the operations
between them.

- `fromLines`, `fromWords`, `fromChars`, `fromUnicodeWords` and
  `fromGraphemes` diff two strings with the default settings.
- `diffLines config old new` and its siblings take a `TextDiffConfig`: start
  from `textDiffConfig` and set `algorithm` or `newlineTerminated`.
- `fromSlices` diffs two arrays of strings as they are.

On a `TextDiff`, `ratio`, `iterAllChanges`, `iterChanges d op` and
`groupedOps d n` work as above.

`unifiedDiff d` prints a text diff as a unified diff. It is a record whose
`contextRadius` (3), `missingNewlineHint` (`True`) and `header`
(`Just ("a/file", "b/file")`) can be changed, and `unifiedDiffText` renders it.
`unifiedDiffOf alg old new n header` does all of that in one call.

`iterInlineChanges d op` gives the lines of an operation with the changed
words marked:

- each `InlineChange` holds `values`, a vector of `(emphasized, text)`;
- `inlineChangeText` renders one the way the crate displays it, wrapping each
  changed word in `-…-` or `+…+`;
- `iterInlineChangesByWords` splits on whitespace instead of Unicode word
  bounds, as the crate does when built without `unicode`.

`diffCharParts`, `diffWordParts`, `diffUnicodeWordParts`, `diffGraphemeParts`
and `diffLineParts` return a diff as tagged runs of text. `diffSliceParts` does
the same for arrays.

`getCloseMatches word possibilities n cutoff` returns up to `n` of the
possibilities that look most like `word` and score at least `cutoff`, best
first.

## How it's made

The files in `src/` are hand translations of the crate's source:

- `Algorithms.mw`: the algorithms, and the compaction and replace hooks that
  run on their output;
- `Text.mw`: the tokenizers and close matches;
- `Unified.mw`: unified diffs;
- `Inline.mw`: inline changes.

The crate's quirks are kept, so that the port gives the same results. One
example: its LCS reports an unchanged range as starting at 0.

**`src/Cases.mw`** and **`src/CasesPlain.mw`** are generated test data:

- 1,500 sequence diffs over whole and partial ranges, with their groups;
- 906 text diffs, each with its:
  - operations and ratio;
  - changes and groups;
  - unified diff;
  - inline changes;
- 600 close-match searches;
- 700 `utils` diffs;
- the inline changes of 518 text diffs, from a build of the crate without
  `unicode`.

Every expected result comes from calling the crate. Run `scripts/generate.sh`
to regenerate; it needs a Rust toolchain.

## Licence

Apache-2.0, like the crate: see [LICENSE](LICENSE) and
[COPYRIGHT](COPYRIGHT).
