//! Writes `src/Cases.mw` (and, built without the `unicode` feature,
//! `src/CasesPlain.mw`) for diff.
//!
//! ```text
//! cargo run --release -- <package root>
//! cargo run --release --no-default-features -- <package root>
//! ```
//!
//! Inputs, with what `similar` makes of them: sequences diffed by every
//! algorithm, over whole ranges and parts; operations grouped; texts diffed by
//! lines, words, characters, Unicode words and graphemes, with their ratios,
//! changes, unified diffs and inline changes; close matches; and the `utils`
//! helpers. Without `unicode`, inline changes split lines into plain words
//! rather than Unicode ones, and only those are written.
//!
//! The library is ported by hand into `src/`, and the crate's source is
//! fingerprinted.

#[cfg(feature = "unicode")]
use similar::utils::{diff_chars, diff_lines, diff_slices, diff_words};
use similar::{Algorithm, ChangeTag, TextDiff};
#[cfg(feature = "unicode")]
use similar::{DiffOp, capture_diff_slices, get_close_matches, group_diff_ops};
use std::fmt::Write as _;
use std::path::PathBuf;

/// The crate version pinned in `Cargo.toml`.
const UPSTREAM_VERSION: &str = "2.7.0";

/// The fingerprint of the crate's source, which `src/` ports.
const SOURCES: u64 = 0xdffe_ec1f_8bd7_802e;

fn main() {
    let root = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "../..".into()));

    let print = fingerprint(include_str!(concat!(env!("OUT_DIR"), "/sources.rs.txt")));
    if print != SOURCES {
        eprintln!(
            "error: similar is not the version src/ ports.\n\
             Compare its source in {} with the previous version, carry any change\n\
             into src/, then set SOURCES in scripts/generate/src/main.rs to\n\
             {print:#x}",
            env!("UPSTREAM_DIR")
        );
        std::process::exit(1);
    }

    #[cfg(feature = "unicode")]
    let (name, cases) = ("Cases", cases());
    #[cfg(not(feature = "unicode"))]
    let (name, cases) = ("CasesPlain", plain_cases());
    let path = root.join(format!("src/{name}.mw"));
    std::fs::write(&path, &cases).unwrap();
    eprintln!("wrote {} ({} bytes)", path.display(), cases.len());
}

/// FNV-1a: stable across builds, which `DefaultHasher` does not promise.
fn fingerprint(text: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// --- encoding -----------------------------------------------------------------------

/// A number as `digits` base-64 digits, most significant first, each digit the
/// character `'0' + d`: `'0'` to `'o'`, one contiguous run of ASCII.
fn digits(out: &mut String, value: u64, digits: u32) {
    assert!(
        value < 1 << (6 * digits),
        "{value} does not fit in {digits} digits"
    );
    for k in (0..digits).rev() {
        out.push(char::from(b'0' + ((value >> (6 * k)) & 63) as u8));
    }
}

/// A string, as its length in bytes (3 digits) and then its bytes.
fn text(out: &mut String, s: &str) {
    digits(out, s.len() as u64, 3);
    out.push_str(s);
}

/// A list of strings: a count (3 digits) and the strings.
fn texts<S: AsRef<str>>(out: &mut String, items: impl IntoIterator<Item = S>) {
    let items: Vec<S> = items.into_iter().collect();
    digits(out, items.len() as u64, 3);
    for s in items {
        text(out, s.as_ref());
    }
}

/// `text` as one Meadow string literal, broken with `\`-newline every `width`
/// characters. Only printable ASCII is written raw; a space that would start a
/// line is `\x20`, since a continuation drops leading whitespace.
fn long_literal(text: &str, width: usize) -> String {
    let mut out = String::with_capacity(text.len() + text.len() / width * 4 + 2);
    out.push('"');
    for (i, c) in text.chars().enumerate() {
        let line_start = i > 0 && i % width == 0;
        if line_start {
            out.push_str("\\\n    ");
        }
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '$' => out.push_str("\\$"),
            ' ' if line_start => out.push_str("\\x20"),
            ' '..='~' => out.push(c),
            _ => {
                let _ = write!(out, "\\u{{{:X}}}", u32::from(c));
            }
        }
    }
    out.push('"');
    out
}

// --- what is compared ---------------------------------------------------------------

/// An operation as `Tests.mw` spells it: its tag and four numbers.
#[cfg(feature = "unicode")]
fn op_text(op: &DiffOp) -> String {
    match *op {
        DiffOp::Equal {
            old_index,
            new_index,
            len,
        } => format!("E {old_index} {new_index} {len}"),
        DiffOp::Delete {
            old_index,
            old_len,
            new_index,
        } => format!("D {old_index} {old_len} {new_index}"),
        DiffOp::Insert {
            old_index,
            new_index,
            new_len,
        } => format!("I {old_index} {new_index} {new_len}"),
        DiffOp::Replace {
            old_index,
            old_len,
            new_index,
            new_len,
        } => format!("R {old_index} {old_len} {new_index} {new_len}"),
    }
}

fn tag_char(tag: ChangeTag) -> char {
    match tag {
        ChangeTag::Equal => '=',
        ChangeTag::Delete => '-',
        ChangeTag::Insert => '+',
    }
}

fn index_text(i: Option<usize>) -> String {
    i.map_or("_".to_string(), |i| i.to_string())
}

/// A ratio, exactly: its value times 2^32, rounded down.
#[cfg(feature = "unicode")]
fn ratio_text(r: f32) -> String {
    ((f64::from(r)) * 4_294_967_296.0).floor().to_string()
}

const ALGORITHMS: [Algorithm; 3] = [Algorithm::Myers, Algorithm::Patience, Algorithm::Lcs];

// --- inputs -------------------------------------------------------------------------

/// A small deterministic generator, so that the cases are the same on every run.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }

    fn pick<T: Copy>(&mut self, xs: &[T]) -> T {
        xs[self.below(xs.len())]
    }
}

#[cfg(feature = "unicode")]
fn random_sequence(rng: &mut Rng, len: usize, alphabet: usize) -> Vec<u8> {
    (0..len).map(|_| rng.below(alphabet) as u8).collect()
}

/// `a`, changed here and there.
fn mutate<T: Clone>(rng: &mut Rng, a: &[T], pool: &[T]) -> Vec<T> {
    let mut b = a.to_vec();
    for _ in 0..rng.below(6) {
        let at = rng.below(b.len() + 1);
        match rng.below(4) {
            0 if !b.is_empty() => {
                b.remove(at.min(b.len() - 1));
            }
            1 => b.insert(at, pool[rng.below(pool.len())].clone()),
            2 if !b.is_empty() => {
                let i = at.min(b.len() - 1);
                b[i] = pool[rng.below(pool.len())].clone();
            }
            _ => {
                if b.len() > 2 {
                    let i = rng.below(b.len() - 1);
                    b.swap(i, i + 1);
                }
            }
        }
    }
    b
}

const LINES: &[&str] = &[
    "fn main() {\n",
    "    println!(\"hello\");\n",
    "}\n",
    "\n",
    "let x = 1;\n",
    "let y = 2;\n",
    "return x + y;\n",
    "// a comment\n",
    "a b c\n",
    "a b d\n",
    "the quick brown fox\n",
    "the quick red fox\n",
    "end\r\n",
    "\r\n",
    "tab\there\n",
    "日本語のテキスト\n",
    "e\u{301}l\u{e8}ve \u{1f468}\u{200d}\u{1f469}\n",
];

const WORDS: &[&str] = &[
    "a",
    "b",
    "the",
    "quick",
    "brown",
    "fox",
    "jumps",
    " ",
    "  ",
    "\t",
    "\n",
    ",",
    ".",
    "hello",
    "world",
    "e\u{301}",
    "\u{1f600}",
    "日本",
    "don't",
    "3.14",
];

/// A text of lines, perhaps without its last newline.
fn random_text(rng: &mut Rng) -> String {
    let n = rng.below(14);
    let mut s: String = (0..n).map(|_| rng.pick(LINES)).collect();
    if rng.chance(30) && s.ends_with('\n') {
        s.pop();
        if s.ends_with('\r') && rng.chance(50) {
            s.pop();
        }
    }
    if rng.chance(10) {
        s.push_str(rng.pick(WORDS));
    }
    s
}

fn change_text(text: &str) -> String {
    let mut rng = Rng(fingerprint(text) | 1);
    let lines: Vec<String> = text.split_inclusive('\n').map(String::from).collect();
    let pool: Vec<String> = LINES.iter().map(|s| s.to_string()).collect();
    let mut out: String = mutate(&mut rng, &lines, &pool).concat();
    if rng.chance(20) {
        out = out.replace("quick", "slow").replace('x', "xx");
    }
    if rng.chance(10) {
        out.push_str(rng.pick(WORDS));
    }
    out
}

fn random_words(rng: &mut Rng) -> String {
    (0..rng.below(16)).map(|_| rng.pick(WORDS)).collect()
}

// --- cases --------------------------------------------------------------------------

/// Everything about one text diff, as the lines `Tests.mw` compares.
#[cfg(feature = "unicode")]
fn text_diff_lines<'a>(diff: &'a TextDiff<'a, 'a, 'a, str>, rng: &mut Rng) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(format!(
        "ops {} ratio {} slices {} {} nl {}",
        diff.ops()
            .iter()
            .map(op_text)
            .collect::<Vec<_>>()
            .join(", "),
        ratio_text(diff.ratio()),
        diff.old_slices().len(),
        diff.new_slices().len(),
        diff.newline_terminated()
    ));
    for change in diff.iter_all_changes() {
        lines.push(format!(
            "{} {} {} {}",
            tag_char(change.tag()),
            index_text(change.old_index()),
            index_text(change.new_index()),
            change.value()
        ));
    }
    let n = rng.below(5);
    for group in diff.grouped_ops(n) {
        lines.push(format!(
            "group {n}: {}",
            group.iter().map(op_text).collect::<Vec<_>>().join(", ")
        ));
    }
    let radius = rng.below(5);
    let hint = rng.chance(70);
    let header = rng.chance(40);
    let mut u = diff.unified_diff();
    u.context_radius(radius).missing_newline_hint(hint);
    if header {
        u.header("a/old.txt", "b/new.txt");
    }
    lines.push(format!("unified {radius} {hint} {header}\n{u}"));
    lines
}

fn inline_lines<'a>(diff: &'a TextDiff<'a, 'a, 'a, str>) -> Vec<String> {
    let mut lines = Vec::new();
    for op in diff.ops() {
        for change in diff.iter_inline_changes(op) {
            let values: Vec<String> = change
                .iter_strings_lossy()
                .map(|(e, s)| format!("{}{s}", if e { "*" } else { "." }))
                .collect();
            lines.push(format!(
                "inline {} {} {} [{}] {}",
                tag_char(change.tag()),
                index_text(change.old_index()),
                index_text(change.new_index()),
                values.join("|"),
                change
            ));
        }
    }
    lines
}

/// The tokenizers, in the order `Tests.mw` numbers them.
fn tokenized<'a>(
    kind: usize,
    config: &similar::TextDiffConfig,
    old: &'a str,
    new: &'a str,
) -> TextDiff<'a, 'a, 'a, str> {
    match kind {
        0 => config.diff_lines(old, new),
        1 => config.diff_words(old, new),
        2 => config.diff_chars(old, new),
        #[cfg(feature = "unicode")]
        3 => config.diff_unicode_words(old, new),
        #[cfg(feature = "unicode")]
        _ => config.diff_graphemes(old, new),
        #[cfg(not(feature = "unicode"))]
        _ => unreachable!(),
    }
}

/// The text diffs both builds write: the same inputs, from the same seed.
fn text_inputs() -> Vec<(usize, usize, Option<bool>, String, String)> {
    let mut rng = Rng(0xd1ff_7a4a_c0ff_ee42);
    let mut out = Vec::new();
    for _ in 0..900 {
        let kind = rng.below(5);
        let alg = rng.below(3);
        let nl = [None, None, Some(true), Some(false)][rng.below(4)];
        let (old, new) = if kind == 0 || rng.chance(40) {
            let old = random_text(&mut rng);
            let new = if rng.chance(10) {
                random_text(&mut rng)
            } else {
                change_text(&old)
            };
            (old, new)
        } else {
            let old = random_words(&mut rng);
            let pool: Vec<String> = WORDS.iter().map(|s| s.to_string()).collect();
            let words: Vec<String> = old.split_inclusive(' ').map(String::from).collect();
            (old.clone(), mutate(&mut rng, &words, &pool).concat())
        };
        out.push((kind, alg, nl, old, new));
    }
    // Long texts, which the crate diffs by ids.
    for k in 0..6 {
        let mut r = Rng(k + 1);
        let old: String = (0..120 + k * 15).map(|_| r.pick(LINES)).collect();
        let new = change_text(&old) + &change_text(&old);
        out.push((0, (k as usize) % 3, None, old, new));
    }
    out
}

fn config(alg: usize, nl: Option<bool>) -> similar::TextDiffConfig {
    let mut config = TextDiff::configure();
    config.algorithm(ALGORITHMS[alg]);
    if let Some(nl) = nl {
        config.newline_terminated(nl);
    }
    config
}

fn nl_digit(nl: Option<bool>) -> u64 {
    match nl {
        None => 0,
        Some(false) => 1,
        Some(true) => 2,
    }
}

#[cfg(feature = "unicode")]
fn cases() -> String {
    let mut rng = Rng(0x5157_7a4a_c0ff_ee42);
    let mut body = String::new();
    let mut counts = [0usize; 5];

    // Kind 0: sequences, diffed whole and over ranges.
    for _ in 0..1500 {
        let alphabet = rng.pick(&[2, 3, 5, 10]);
        let len = rng.below(24);
        let old = random_sequence(&mut rng, len, alphabet);
        let pool: Vec<u8> = (0..alphabet as u8).collect();
        let new = if rng.chance(20) {
            let n = rng.below(24);
            random_sequence(&mut rng, n, alphabet)
        } else {
            mutate(&mut rng, &old, &pool)
        };
        let alg = rng.below(3);
        let (oa, ob) = if rng.chance(30) {
            let a = rng.below(old.len() + 1);
            (a, a + rng.below(old.len() - a + 1))
        } else {
            (0, old.len())
        };
        let (na, nb) = if rng.chance(30) {
            let a = rng.below(new.len() + 1);
            (a, a + rng.below(new.len() - a + 1))
        } else {
            (0, new.len())
        };
        let ops = similar::capture_diff(ALGORITHMS[alg], &old, oa..ob, &new, na..nb);
        counts[0] += 1;
        digits(&mut body, 0, 1);
        digits(&mut body, alg as u64, 1);
        text(
            &mut body,
            &old.iter().map(|d| char::from(b'0' + d)).collect::<String>(),
        );
        text(
            &mut body,
            &new.iter().map(|d| char::from(b'0' + d)).collect::<String>(),
        );
        for n in [oa, ob, na, nb] {
            digits(&mut body, n as u64, 2);
        }
        let mut lines: Vec<String> = ops.iter().map(op_text).collect();
        let n = rng.below(4);
        digits(&mut body, n as u64, 1);
        for group in group_diff_ops(ops.clone(), n) {
            lines.push(format!(
                "group: {}",
                group.iter().map(op_text).collect::<Vec<_>>().join(", ")
            ));
        }
        if oa == 0 && ob == old.len() && na == 0 && nb == new.len() {
            let whole = capture_diff_slices(ALGORITHMS[alg], &old, &new);
            assert_eq!(whole, ops);
            for (tag, slice) in diff_slices(ALGORITHMS[alg], &old, &new) {
                lines.push(format!(
                    "slice {} {}",
                    tag_char(tag),
                    slice
                        .iter()
                        .map(|d| char::from(b'0' + d))
                        .collect::<String>()
                ));
            }
        }
        texts(&mut body, lines);
    }

    // Kind 1: text diffs.
    for (kind, alg, nl, old, new) in text_inputs() {
        let config = config(alg, nl);
        let diff = tokenized(kind, &config, &old, &new);
        counts[1] += 1;
        digits(&mut body, 1, 1);
        digits(&mut body, kind as u64, 1);
        digits(&mut body, alg as u64, 1);
        digits(&mut body, nl_digit(nl), 1);
        text(&mut body, &old);
        text(&mut body, &new);
        let mut lines = text_diff_lines(&diff, &mut rng);
        lines.extend(inline_lines(&diff));
        texts(&mut body, lines);
    }

    // Kind 2: close matches.
    let vocabulary = [
        "apple",
        "ape",
        "apply",
        "appeal",
        "peach",
        "puppy",
        "pineapple",
        "app",
        "",
        "a",
        "banana",
        "bandana",
        "cabana",
        "applet",
        "maple",
        "日本",
        "日本語",
    ];
    for _ in 0..600 {
        let word = if rng.chance(20) {
            random_words(&mut rng)
        } else {
            rng.pick(&vocabulary).to_string()
        };
        let possibilities: Vec<&str> = (0..rng.below(10)).map(|_| rng.pick(&vocabulary)).collect();
        let n = rng.below(5);
        let cutoff = rng.pick(&[0.0f32, 0.3, 0.5, 0.6, 0.8, 1.0]);
        counts[2] += 1;
        digits(&mut body, 2, 1);
        text(&mut body, &word);
        texts(&mut body, &possibilities);
        digits(&mut body, n as u64, 1);
        digits(&mut body, (cutoff * 10.0).round() as u64, 2);
        texts(
            &mut body,
            get_close_matches(word.as_str(), &possibilities, n, cutoff),
        );
    }

    // Kind 3: the `utils` helpers.
    for _ in 0..400 {
        let helper = rng.below(3);
        let alg = rng.below(3);
        let (old, new) = if helper == 2 {
            let old = random_text(&mut rng);
            let new = change_text(&old);
            (old, new)
        } else {
            let old = random_words(&mut rng);
            let new = random_words(&mut rng);
            (old, new)
        };
        let pairs = match helper {
            0 => diff_chars(ALGORITHMS[alg], &old, &new),
            1 => diff_words(ALGORITHMS[alg], &old, &new),
            _ => diff_lines(ALGORITHMS[alg], &old, &new),
        };
        counts[3] += 1;
        digits(&mut body, 3, 1);
        digits(&mut body, helper as u64, 1);
        digits(&mut body, alg as u64, 1);
        text(&mut body, &old);
        text(&mut body, &new);
        texts(
            &mut body,
            pairs.iter().map(|(t, s)| format!("{}{s}", tag_char(*t))),
        );
    }

    // Kind 4: the Unicode helpers.
    for _ in 0..300 {
        let helper = rng.below(2);
        let alg = rng.below(3);
        let old = random_words(&mut rng);
        let new = random_words(&mut rng);
        let pairs = if helper == 0 {
            similar::utils::diff_unicode_words(ALGORITHMS[alg], &old, &new)
        } else {
            similar::utils::diff_graphemes(ALGORITHMS[alg], &old, &new)
        };
        counts[4] += 1;
        digits(&mut body, 4, 1);
        digits(&mut body, helper as u64, 1);
        digits(&mut body, alg as u64, 1);
        text(&mut body, &old);
        text(&mut body, &new);
        texts(
            &mut body,
            pairs.iter().map(|(t, s)| format!("{}{s}", tag_char(*t))),
        );
    }

    write_cases(
        "Cases",
        &format!(
            "{} sequence diffs, {} text diffs, {} close-match searches, {}\n\
             -- `utils` diffs and {} Unicode `utils` diffs",
            counts[0], counts[1], counts[2], counts[3], counts[4]
        ),
        &body,
    )
}

/// Inline changes by plain words, from a build without `unicode`.
#[cfg(not(feature = "unicode"))]
fn plain_cases() -> String {
    let mut body = String::new();
    let mut count = 0;
    for (kind, alg, nl, old, new) in text_inputs() {
        // Only the tokenizers this build has.
        if kind > 2 {
            continue;
        }
        let config = config(alg, nl);
        let diff = tokenized(kind, &config, &old, &new);
        count += 1;
        digits(&mut body, 5, 1);
        digits(&mut body, kind as u64, 1);
        digits(&mut body, alg as u64, 1);
        digits(&mut body, nl_digit(nl), 1);
        text(&mut body, &old);
        text(&mut body, &new);
        texts(&mut body, inline_lines(&diff));
    }
    write_cases(
        "casesPlain",
        &format!("{count} text diffs' inline changes by plain words"),
        &body,
    )
}

fn write_cases(name: &str, what: &str, body: &str) -> String {
    let def = if name == "Cases" { "cases" } else { name };
    let mut out = String::new();
    let _ = writeln!(
        out,
        "-- GENERATED by scripts/generate.sh from similar {UPSTREAM_VERSION}.
-- Do not edit: run the script again instead.
--
-- Inputs, with what the crate makes of them, for `Tests.mw`: {what}.
--
-- Copyright Armin Ronacher, Pierre-Étienne Meunier and Brandon Williams, and
-- the Meadow port's authors. Licensed under Apache-2.0: see LICENSE and
-- COPYRIGHT.

-- Each case starts with its kind (1 base-64 digit), and its fields follow in
-- the order `Tests.mw` reads them. A string is its length in bytes (3 digits)
-- and then its bytes; a list is a count (3) and its strings. What is compared
-- is the list of lines at the end of each case.
@cfg(test)
@pub(pkg) def {def} =
  {}",
        long_literal(body, 96)
    );
    out
}
