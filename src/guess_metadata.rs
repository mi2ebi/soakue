// this file was written by claude for an experiment

//! Heuristic + Naive Bayes metadata guesser for Toaq dictionary entries.
//!
//! Writes `data/guesses.txt` with accuracy stats on the already-annotated
//! entries followed by metadata guesses for everything that has ▯ but no
//! existing annotation.
//!
//! Frame and distribution are still rule-based (they're syntactic/structural
//! and 91–94% accurate already). Pronoun and subject use Naive Bayes trained
//! on the annotated entries in the same dictionary.
//!
//! Note: accuracy is evaluated on the training data, so numbers will be
//! optimistic. Held-out experiments suggest ~67% real pronoun accuracy.

use std::{
    collections::HashMap,
    fs,
    io::{self, Write as _},
};

use crate::toadua::Toa;

// ─── tokenizer ────────────────────────────────────────────────────────────────

/// Split a definition body into tokens. Keeps alphabetic chars (including ı
/// and other Unicode letters) and ▯ as a literal token.
fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if ch == '▯' {
            if !current.is_empty() {
                tokens.push(current.to_lowercase());
                current.clear();
            }
            tokens.push("▯".to_string());
        } else if ch.is_alphabetic() {
            current.push(ch);
        } else {
            if !current.is_empty() {
                tokens.push(current.to_lowercase());
                current.clear();
            }
        }
    }
    if !current.is_empty() {
        tokens.push(current.to_lowercase());
    }
    tokens
}

// ─── Naive Bayes ─────────────────────────────────────────────────────────────

struct NaiveBayes {
    /// Number of training examples per class.
    class_counts: HashMap<String, usize>,
    /// token → class → count.
    token_class_counts: HashMap<String, HashMap<String, usize>>,
    /// Total unique tokens (for Laplace smoothing denominator).
    vocab_size: usize,
    /// Ordered list of classes (for deterministic argmax).
    classes: Vec<String>,
}

impl NaiveBayes {
    fn train<'a>(examples: impl Iterator<Item = (&'a str, &'a str)>) -> Self {
        let mut class_counts: HashMap<String, usize> = HashMap::new();
        let mut token_class_counts: HashMap<String, HashMap<String, usize>> = HashMap::new();

        for (text, label) in examples {
            *class_counts.entry(label.to_string()).or_insert(0) += 1;
            for token in tokenize(text) {
                *token_class_counts
                    .entry(token)
                    .or_default()
                    .entry(label.to_string())
                    .or_insert(0) += 1;
            }
        }

        let vocab_size = token_class_counts.len();
        let mut classes: Vec<String> = class_counts.keys().cloned().collect();
        classes.sort();

        NaiveBayes { class_counts, token_class_counts, vocab_size, classes }
    }

    fn predict(&self, text: &str) -> &str {
        let tokens = tokenize(text);
        let total_examples: usize = self.class_counts.values().sum();
        let n_classes = self.classes.len();

        let best = self.classes.iter().max_by(|a, b| {
            let score_for = |c: &str| -> f64 {
                let class_count = *self.class_counts.get(c).unwrap_or(&0);
                // log prior: Laplace-smoothed
                let mut score =
                    ((class_count + 1) as f64 / (total_examples + n_classes) as f64).ln();
                // log likelihood: Laplace-smoothed
                for token in &tokens {
                    let token_count = self
                        .token_class_counts
                        .get(token)
                        .and_then(|m| m.get(c))
                        .copied()
                        .unwrap_or(0);
                    score += ((token_count + 1) as f64
                        / (class_count + self.vocab_size + 1) as f64)
                        .ln();
                }
                score
            };
            score_for(a).partial_cmp(&score_for(b)).unwrap()
        });

        best.map(|s| s.as_str()).unwrap_or("")
    }

    /// Print the most discriminative tokens per class (P(token|class) / P(token)).
    fn print_top_tokens(&self, label: &str, n: usize, out: &mut impl io::Write) -> io::Result<()> {
        let total_examples: usize = self.class_counts.values().sum();
        writeln!(out, "Top discriminative tokens for {label}:")?;
        for class in &self.classes {
            let class_count = *self.class_counts.get(class.as_str()).unwrap_or(&0);
            let mut scores: Vec<(&str, f64)> = self
                .token_class_counts
                .iter()
                .filter(|(_, m)| m.values().sum::<usize>() >= 3) // ignore rare tokens
                .map(|(token, class_map)| {
                    let token_total: usize = class_map.values().sum();
                    let p_t_given_c =
                        (*class_map.get(class.as_str()).unwrap_or(&0) + 1) as f64
                        / (class_count + 1) as f64;
                    let p_t = (token_total + 1) as f64 / (total_examples + 1) as f64;
                    (token.as_str(), p_t_given_c / p_t)
                })
                .collect();
            scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            let top: Vec<&str> = scores.iter().take(n).map(|(t, _)| *t).collect();
            writeln!(out, "  {class}: {top:?}")?;
        }
        Ok(())
    }
}

// ─── frame heuristics (unchanged from previous version) ──────────────────────

fn primary_arity(body: &str) -> usize {
    body.split(';').next().unwrap_or(body).chars().filter(|&c| c == '▯').count()
}

fn normalize_body(s: &str) -> String {
    s.chars().filter(|&c| c != '*' && c != '†' && c != '‡').collect::<String>().to_lowercase()
}

fn classify_slot(body: &str, slot_idx: usize) -> &'static str {
    let parts: Vec<&str> = body.split('▯').collect();
    let before = normalize_body(parts.get(slot_idx).copied().unwrap_or(""));
    let after = normalize_body(parts.get(slot_idx + 1).copied().unwrap_or(""));
    let after_trim = after.trim_start_matches(|c: char| " ;,./†*".contains(c));

    if after_trim.starts_with("is the case")
        || after_trim.starts_with("is not the case")
        || after_trim.starts_with("is true")
        || after_trim.starts_with("is false")
        || after_trim.starts_with("begins")
        || after_trim.starts_with("occurs")
        || after_trim.starts_with("takes place")
        || after_trim.starts_with("holds")
        || after_trim.starts_with("happens")
        || before.ends_with("that ")
        || before.ends_with("whether ")
        || before.ends_with("if ")
    {
        return "0";
    }

    let is_property = before.ends_with("property ")
        || before.ends_with("satisfies ")
        || before.ends_with("satisfying ")
        || before.ends_with("has property ")
        || before.contains("satisfies property")
        || before.contains("satisfy property")
        || before.ends_with("for doing ")
        || before.ends_with("for starting ")
        || before.ends_with("concept of satisfying ")
        || before.ends_with("way of ");

    if is_property {
        if before.contains("concept of")
            || before.contains("community of")
            || before.ends_with("instructions for ")
            || before.ends_with("recipe for ")
            || before.ends_with("energy for ")
            || before.ends_with("idea of ")
            || before.ends_with("notion of ")
        {
            return "1x";
        }
        if slot_idx >= 2 {
            let b = body.to_lowercase();
            if ["making it satisfy", "into satisfying", "to satisfy",
                "entrusts", "tricks", "shoves", "thrusts", "pushes",
                "jams", "dunks", "hurls", "forces", "compels",
                "manipulat", "persuad", "teaches"]
                .iter().any(|v| b.contains(v))
            {
                return "1j";
            }
        }
        return "1i";
    }

    if before.ends_with("relation ")
        || before.ends_with("in relation ")
        || before.ends_with("satisfies relation ")
        || before.ends_with("relationship ")
        || before.ends_with("connected by relation ")
    {
        let b = body.to_lowercase();
        if b.contains("each other") || b.contains("reciprocal")
            || b.contains("consecutive") || b.contains("both sides")
        {
            return "2xx";
        }
        if slot_idx >= 2 { return "2ij"; }
        return "2ix";
    }

    "c"
}

fn guess_frame(body: &str, n: usize) -> String {
    (0..n).map(|i| classify_slot(body, i)).collect::<Vec<_>>().join(" ")
}

fn guess_distribution(body: &str, n: usize) -> String {
    let b = body.to_lowercase();
    let collective = b.contains("each other") || b.contains("mutual")
        || b.contains("collectively") || b.contains("reciprocal")
        || b.contains("annihilat") || b.contains("both sides");
    (0..n).map(|i| if collective && i == 0 { "n" } else { "d" })
        .collect::<Vec<_>>().join(" ")
}

// ─── valid label sets ─────────────────────────────────────────────────────────

const VALID_PRONOUNS: &[&str] = &["hó", "máq", "hóq", "tá"];
const VALID_SUBJECTS: &[&str] = &["I", "F", "A", "E", "P", "S"];

fn is_valid_pronoun(s: &str) -> bool { VALID_PRONOUNS.contains(&s) }
fn is_valid_subject(s: &str) -> bool { VALID_SUBJECTS.contains(&s) }

// ─── top-level ────────────────────────────────────────────────────────────────

pub fn run(dict: &[Toa]) -> io::Result<()> {
    fs::create_dir_all("data")?;
    let mut out = fs::File::create("data/guesses.txt")?;

    // ── train ─────────────────────────────────────────────────────────────
    let annotated: Vec<&Toa> = dict
        .iter()
        .filter(|t| t.has_metadata() && primary_arity(&t.body) > 0)
        .filter(|t| {
            t.pronoun.as_deref().is_some_and(is_valid_pronoun)
                && t.subject.as_deref().is_some_and(is_valid_subject)
        })
        .collect();

    let pronoun_model = NaiveBayes::train(
        annotated.iter().map(|t| (t.body.as_str(), t.pronoun.as_deref().unwrap()))
    );
    let subject_model = NaiveBayes::train(
        annotated.iter().map(|t| (t.body.as_str(), t.subject.as_deref().unwrap()))
    );

    pronoun_model.print_top_tokens("pronoun", 10, &mut out)?;
    writeln!(out)?;
    subject_model.print_top_tokens("subject", 10, &mut out)?;
    writeln!(out)?;

    // ── accuracy pass (on training data — optimistic) ─────────────────────
    writeln!(out, "=== MISMATCHES ON ANNOTATED ENTRIES (evaluated on training data) ===")?;
    writeln!(out)?;

    let mut total = 0usize;
    let mut n_frame = 0usize; let mut ok_frame = 0usize;
    let mut n_dist  = 0usize; let mut ok_dist  = 0usize;
    let mut n_pron  = 0usize; let mut ok_pron  = 0usize;
    let mut n_subj  = 0usize; let mut ok_subj  = 0usize;
    let mut ok_all  = 0usize;

    for toa in dict.iter().filter(|t| t.has_metadata()) {
        let n = primary_arity(&toa.body);
        if n == 0 { continue; }
        total += 1;

        let frame = guess_frame(&toa.body, n);
        let dist  = guess_distribution(&toa.body, n);
        let pron  = pronoun_model.predict(&toa.body).to_string();
        let subj  = subject_model.predict(&toa.body).to_string();

        let af = toa.frame.as_deref();
        let ad = toa.distribution.as_deref();
        let ap = toa.pronoun.as_deref();
        let as_ = toa.subject.as_deref();

        let fm = af.map(|v| frame == v);
        let dm = ad.map(|v| dist  == v);
        let pm = ap.map(|v| pron  == v);
        let sm = as_.map(|v| subj  == v);

        if let Some(b) = fm { n_frame += 1; if b { ok_frame += 1; } }
        if let Some(b) = dm { n_dist  += 1; if b { ok_dist  += 1; } }
        if let Some(b) = pm { n_pron  += 1; if b { ok_pron  += 1; } }
        if let Some(b) = sm { n_subj  += 1; if b { ok_subj  += 1; } }

        if fm.unwrap_or(true) && dm.unwrap_or(true)
            && pm.unwrap_or(true) && sm.unwrap_or(true)
        {
            ok_all += 1;
        }

        let any_wrong = matches!(fm, Some(false)) || matches!(dm, Some(false))
            || matches!(pm, Some(false)) || matches!(sm, Some(false));

        if any_wrong {
            writeln!(out, "✗ {} #{}", toa.head, &toa.id)?;
            writeln!(out, "  actual:  [({}) ({}) {} {}]",
                af.unwrap_or("?"), ad.unwrap_or("?"),
                ap.unwrap_or("?"), as_.unwrap_or("?"))?;
            writeln!(out, "  guessed: [({}) ({}) {} {}]",
                frame, dist, pron, subj)?;
        }
    }

    let pct = |ok: usize, n: usize| {
        if n == 0 { 0.0_f64 } else { 100.0 * ok as f64 / n as f64 }
    };

    writeln!(out)?;
    writeln!(out, "=== ACCURACY ({total} annotated entries, evaluated on training data) ===")?;
    writeln!(out, "  (held-out experiment suggests ~67% real pronoun accuracy)")?;
    writeln!(out, "  frame:        {ok_frame}/{n_frame} ({:.1}%)", pct(ok_frame, n_frame))?;
    writeln!(out, "  distribution: {ok_dist}/{n_dist} ({:.1}%)",   pct(ok_dist,  n_dist))?;
    writeln!(out, "  pronoun:      {ok_pron}/{n_pron} ({:.1}%)",   pct(ok_pron,  n_pron))?;
    writeln!(out, "  subject:      {ok_subj}/{n_subj} ({:.1}%)",   pct(ok_subj,  n_subj))?;
    writeln!(out, "  all fields:   {ok_all}/{total} ({:.1}%)",      pct(ok_all,   total))?;

    // ── guess pass ────────────────────────────────────────────────────────
    writeln!(out)?;
    writeln!(out, "=== GUESSES FOR UNANNOTATED ENTRIES ===")?;
    writeln!(out)?;

    let mut n_guessed = 0usize;
    for toa in dict.iter().filter(|t| !t.has_metadata()) {
        let n = primary_arity(&toa.body);
        if n == 0 { continue; }
        let frame = guess_frame(&toa.body, n);
        let dist  = guess_distribution(&toa.body, n);
        let pron  = pronoun_model.predict(&toa.body);
        let subj  = subject_model.predict(&toa.body);
        n_guessed += 1;
        writeln!(out, "{} #{} → [({}) ({}) {} {}]",
            toa.head, toa.id, frame, dist, pron, subj)?;
    }

    println!("data/guesses.txt: {total} annotated checked, {n_guessed} unannotated guessed");
    Ok(())
}