// this file was written by claude for an experiment
#![allow(
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

//! Heuristic + Logistic Regression metadata guesser for Toaq dictionary
//! entries.
//!
//! Writes `data/guesses.txt` with:
//!   - Top discriminative tokens per class (sanity check)
//!   - 10-fold cross-validation accuracy (honest held-out estimate)
//!   - Training-data accuracy for frame/distribution (heuristics)
//!   - Guesses for unannotated entries with calibrated pronoun confidence
//!
//! Frame and distribution use heuristics (~93%/96% accurate).
//! Pronoun and subject use Logistic Regression trained on annotated entries.
//! Subject uses classifier chaining: predicted pronoun is an additional
//! feature. Pronoun confidence is calibrated (raw softmax → actual accuracy
//! estimate). Subject confidence is not reported — it is essentially
//! uncorrelated with accuracy.

use std::{
    collections::HashMap,
    fs,
    io::{self, Write as _},
};

use itertools::Itertools as _;

use crate::toadua::{Toa, split_into_raku};

// ─── feature extraction & tokenization ───────────────────────────────────────

fn extract_features(toa: &Toa) -> String {
    let mut tokens = tokenize(&toa.body);

    if let Some(rakus) = split_into_raku(&toa.head)
        && let Some(raku0) = rakus.last()
    {
        tokens.push(format!("_RAKU_{raku0}"));
        if rakus.len() >= 2
            && let Some(raku1) = rakus.get(rakus.len() - 2)
        {
            tokens.push(format!("_2_RAKU_{raku1}{raku0}"));
        }
    }

    tokens.push(format!("_ARITY_{}", primary_arity(&toa.body).0));

    if toa.head.chars().next().is_some_and(char::is_uppercase) {
        tokens.push("_CAPS".to_string());
    }

    tokens.join(" ")
}

fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    macro_rules! push_lowercase_unless_special {
        () => {
            if current.starts_with('_') {
                tokens.push(current.clone());
            } else {
                tokens.push(current.to_lowercase());
            }
        };
    }
    for ch in text.chars() {
        if ch == '▯' {
            if !current.is_empty() {
                push_lowercase_unless_special!();
                current.clear();
            }
            tokens.push("▯".to_string());
        } else if ch.is_alphanumeric() || ch == '_' {
            current.push(ch);
        } else if !current.is_empty() {
            push_lowercase_unless_special!();
            current.clear();
        }
    }
    if !current.is_empty() {
        push_lowercase_unless_special!();
    }
    tokens.push("_BIAS".to_string());
    tokens
}

/// Append predicted pronoun as a classifier-chaining feature.
fn with_pron_feature(features: &str, pron: &str) -> String { format!("{features} _PRON_{pron}") }

// ─── Logistic Regression (SGD) ───────────────────────────────────────────────

#[derive(Clone)]
struct LogisticRegression {
    weights: Vec<f64>,
    classes: Vec<String>,
    vocab: HashMap<String, usize>,
}

impl LogisticRegression {
    fn train<'a>(
        examples: impl Iterator<Item = (&'a str, &'a str)>,
        weights: &HashMap<String, f64>,
    ) -> Self {
        let mut class_to_id = HashMap::new();
        let mut vocab = HashMap::new();
        let mut processed_data = Vec::new();

        for (text, label) in examples {
            let next_class_id = class_to_id.len();
            let c_id = *class_to_id.entry(label.to_string()).or_insert(next_class_id);

            let mut token_ids = Vec::new();
            for token in tokenize(text) {
                let next_vocab_id = vocab.len();
                token_ids.push(*vocab.entry(token).or_insert(next_vocab_id));
            }
            processed_data.push((token_ids, c_id));
        }

        let num_classes = class_to_id.len();
        let num_tokens = vocab.len();

        let mut classes = vec![String::new(); num_classes];
        for (name, &id) in &class_to_id {
            classes[id].clone_from(name);
        }

        let mut model = Self { weights: vec![0.0; num_tokens * num_classes], classes, vocab };

        let epochs = 50;
        let learning_rate = 0.1;

        for epoch in 0..epochs {
            let lr = learning_rate / 0.1_f64.mul_add(f64::from(epoch), 1.0);
            for (token_ids, label_id) in &processed_data {
                // Get probabilities
                let mut scores = vec![0.0; num_classes];
                for (c_idx, score) in scores.iter_mut().enumerate() {
                    for &t_id in token_ids {
                        *score += model.weights[t_id * num_classes + c_idx];
                    }
                }

                // Softmax
                let max_score = scores.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                let exps: Vec<f64> = scores.iter().map(|s| (s - max_score).exp()).collect();
                let sum_exps: f64 = exps.iter().sum();

                // Update weights
                for (c_idx, e) in exps.iter().enumerate().take(num_classes) {
                    let prob = e / sum_exps;
                    let target = if c_idx == *label_id { 1.0 } else { 0.0 };
                    let error = target - prob;

                    let class_name = &model.classes[c_idx];
                    let class_weight = weights.get(class_name).unwrap_or(&1.0);

                    for &t_id in token_ids {
                        model.weights[t_id * num_classes + c_idx] += lr * error * class_weight;
                    }
                }
            }
        }
        model
    }

    fn get_probs_from_tokens(&self, tokens: &[String]) -> Vec<f64> {
        let num_classes = self.classes.len();
        let mut scores = vec![0.0; num_classes];

        for token in tokens {
            if let Some(&t_id) = self.vocab.get(token) {
                let offset = t_id * num_classes;
                for (i, score) in scores.iter_mut().enumerate() {
                    *score += self.weights[offset + i];
                }
            }
        }

        let max_score = scores.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let exps: Vec<f64> = scores.iter().map(|s| (s - max_score).exp()).collect();
        let sum_exps: f64 = exps.iter().sum();
        exps.iter().map(|e| e / sum_exps).collect()
    }

    fn predict_raw(&self, text: &str) -> (String, f64) {
        let tokens = tokenize(text);
        let probs = self.get_probs_from_tokens(&tokens);
        let (best_idx, &max_prob) =
            probs.iter().enumerate().max_by(|a, b| a.1.partial_cmp(b.1).unwrap()).unwrap();

        (self.classes[best_idx].clone(), max_prob)
    }

    fn predict(&self, text: &str) -> String { self.predict_raw(text).0 }

    fn print_top_tokens(
        &self,
        label_type: &str,
        n: usize,
        out: &mut impl io::Write,
    ) -> io::Result<()> {
        writeln!(out, "Top discriminative tokens (LogReg Weights) for {label_type}:")?;

        let num_classes = self.classes.len();

        let mut id_to_token = vec![String::new(); self.vocab.len()];
        for (token, &id) in &self.vocab {
            id_to_token[id].clone_from(token);
        }

        let order = if label_type == "pronoun" { VALID_PRONOUNS } else { VALID_SUBJECTS };

        for class_name in order {
            if let Some(c_idx) = self.classes.iter().position(|c| c == class_name) {
                let mut class_weights: Vec<(&String, f64)> = self
                    .vocab
                    .values()
                    .map(|&t_id| {
                        let weight = self.weights[t_id * num_classes + c_idx];
                        (&id_to_token[t_id], weight)
                    })
                    .collect();

                class_weights
                    .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

                let top: Vec<String> =
                    class_weights.iter().take(n).map(|(t, w)| format!("{t}({w:.2})")).collect();

                writeln!(out, "  {class_name}: {top:?}")?;
            }
        }
        Ok(())
    }
}

// ─── calibration ─────────────────────────────────────────────────────────────

/// Monotone calibration mapping raw softmax confidence → estimated actual
/// accuracy. Fitted from CV data via pool-adjacent-violators isotonic
/// regression.
struct Calibration {
    /// Sorted breakpoints: (`raw_conf`, `actual_accuracy`).
    breakpoints: Vec<(f64, f64)>,
}

impl Calibration {
    fn fit(mut results: Vec<(f64, bool)>, n_buckets: usize) -> Self {
        results.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

        // Bucket by quantised confidence value (steps of 1/`n_buckets`) rather
        // than equal count, so entries with identical raw conf (e.g. 1.0) all
        // land in the same bucket instead of spilling across several.
        let step = 1.0 / n_buckets as f64;
        let bucket_idx = |c: f64| ((c / step).floor() as usize).min(n_buckets - 1);

        let mut buckets: Vec<Vec<(f64, bool)>> = vec![Vec::new(); n_buckets];
        for &(c, ok) in &results {
            buckets[bucket_idx(c)].push((c, ok));
        }

        let mut breakpoints: Vec<(f64, f64)> = buckets
            .into_iter()
            .filter(|b| !b.is_empty())
            .map(|bucket| {
                let mean_conf = bucket.iter().map(|(c, _)| c).sum::<f64>() / bucket.len() as f64;
                let actual_acc =
                    bucket.iter().filter(|(_, ok)| *ok).count() as f64 / bucket.len() as f64;
                (mean_conf, actual_acc)
            })
            .collect();

        // pool-adjacent-violators: merge any pair where acc decreases
        loop {
            let mut merged = false;
            let mut i = 0;
            let mut next = Vec::new();
            while i < breakpoints.len() {
                if i + 1 < breakpoints.len() && breakpoints[i].1 > breakpoints[i + 1].1 {
                    // merge: weighted average
                    next.push((
                        f64::midpoint(breakpoints[i].0, breakpoints[i + 1].0),
                        f64::midpoint(breakpoints[i].1, breakpoints[i + 1].1),
                    ));
                    i += 2;
                    merged = true;
                } else {
                    next.push(breakpoints[i]);
                    i += 1;
                }
            }
            breakpoints = next;
            if !merged {
                break;
            }
        }

        Self { breakpoints }
    }

    fn calibrate(&self, raw_conf: f64) -> f64 {
        let bp = &self.breakpoints;
        if bp.is_empty() {
            return raw_conf;
        }
        if raw_conf <= bp[0].0 {
            return bp[0].1;
        }
        if raw_conf >= bp[bp.len() - 1].0 {
            return bp[bp.len() - 1].1;
        }
        for i in 0..bp.len() - 1 {
            if bp[i].0 <= raw_conf && raw_conf <= bp[i + 1].0 {
                let t = (raw_conf - bp[i].0) / (bp[i + 1].0 - bp[i].0);
                return t.mul_add(bp[i + 1].1 - bp[i].1, bp[i].1);
            }
        }
        bp[bp.len() - 1].1
    }
}

// ─── k-fold CV with chaining + calibration ───────────────────────────────────

struct CvResult {
    pron_acc: f64,
    subj_acc: f64,
    /// (`raw_conf`, `correct`) pairs for pronoun, used to fit calibration.
    pron_calibration_data: Vec<(f64, bool)>,
    /// class → (correct, total) for pronoun.
    pron_per_class: HashMap<String, (usize, usize)>,
    /// class → (correct, total) for subject.
    subj_per_class: HashMap<String, (usize, usize)>,
}

fn kfold_cv(examples: &[(String, &str, &str, usize)], k: usize) -> CvResult {
    let n = examples.len();
    let mut correct_pron = 0;
    let mut correct_subj = 0;
    let mut pron_calibration_data: Vec<(f64, bool)> = Vec::new();
    let mut pron_per_class: HashMap<String, (usize, usize)> = HashMap::new();
    let mut subj_per_class: HashMap<String, (usize, usize)> = HashMap::new();

    for fold in 0..k {
        let test_start = fold * n / k;
        let test_end = (fold + 1) * n / k;

        let train: Vec<_> =
            examples[..test_start].iter().chain(examples[test_end..].iter()).collect();
        let test = &examples[test_start..test_end];

        let mut pron_weights = HashMap::new();
        let mut subj_weights = HashMap::new();
        let mut p_counts = HashMap::new();
        let mut s_counts = HashMap::new();

        for (_, p, s, _) in &train {
            *p_counts.entry(p.to_string()).or_insert(0) += 1;
            *s_counts.entry(s.to_string()).or_insert(0) += 1;
        }

        let train_n = train.len() as f64;
        for (name, count) in p_counts {
            pron_weights.insert(name, train_n / (f64::from(count) * 4.0)); // 4 main prons
        }
        for (name, count) in s_counts {
            subj_weights.insert(name, train_n / (f64::from(count) * 6.0)); // A, I, E, F, P, S
        }

        // train pronoun model on raw features
        let pron_model = LogisticRegression::train(
            train.iter().map(|(f, p, _, _)| (f.as_str(), *p)),
            &pron_weights,
        );

        // train subject model with chained pronoun feature:
        // for each training example, predict pronoun and append as feature
        let subj_train_examples: Vec<(String, &str)> =
            train.iter().map(|(f, p, s, _)| (with_pron_feature(f, p), *s)).collect();
        let subj_model = LogisticRegression::train(
            subj_train_examples.iter().map(|(f, s)| (f.as_str(), *s)),
            &subj_weights,
        );

        for (features, pron, subj, _) in test {
            let (pred_pron, raw_conf) = pron_model.predict_raw(features);
            let correct_p = pred_pron == *pron;
            if correct_p {
                correct_pron += 1;
            }
            pron_calibration_data.push((raw_conf, correct_p));
            let e = pron_per_class.entry((*pron).to_string()).or_insert((0, 0));
            if correct_p {
                e.0 += 1;
            }
            e.1 += 1;

            let subj_features = with_pron_feature(features, &pred_pron);
            let correct_s = subj_model.predict(&subj_features) == *subj;
            if correct_s {
                correct_subj += 1;
            }
            let e = subj_per_class.entry((*subj).to_string()).or_insert((0, 0));
            if correct_s {
                e.0 += 1;
            }
            e.1 += 1;
        }
    }

    CvResult {
        pron_acc: f64::from(correct_pron) / n as f64,
        subj_acc: f64::from(correct_subj) / n as f64,
        pron_calibration_data,
        pron_per_class,
        subj_per_class,
    }
}

// ─── frame heuristics ────────────────────────────────────────────────────────

fn primary_arity(body: &str) -> (usize, &str) {
    body.split(';')
        .filter(|clause| clause.contains('▯'))
        .map(|clause| (clause.chars().filter(|&c| c == '▯').count(), clause))
        .max_by_key(|(a, _)| *a)
        .unwrap_or_default()
}

fn normalize_body(s: &str) -> String { s.to_lowercase() }

fn classify_slot(body: &str, slot_idx: usize) -> &'static str {
    let parts: Vec<&str> = body.split('▯').collect();
    let before = normalize_body(parts.get(slot_idx).copied().unwrap_or(""));
    let after = normalize_body(parts.get(slot_idx + 1).copied().unwrap_or(""));
    let after = after.trim_start_matches(|c: char| " ;,./".contains(c));

    if after.contains("is the case")
        || (1..=2).any(|n| after.split_whitespace().skip(n).join(" ").starts_with("the case"))
        || after.starts_with("is true")
        || after.starts_with("is false")
        || after.starts_with("begins")
        || after.starts_with("occurs")
        || after.starts_with("takes place")
        || before.ends_with("that ")
        || before.ends_with("whether ")
        || before.ends_with("if ")
    {
        return "0";
    }

    if before.ends_with("for doing ") {
        return "1x";
    }

    let is_property = before.ends_with("property ")
        || before.contains("satisfies ")
        || before.contains("satisfying ")
        || before.ends_with("to do ");

    if is_property {
        if before.ends_with("instructions for ") || before.ends_with("recipe for ") {
            return "1x";
        }
        if slot_idx >= 2 {
            let b = body.to_lowercase();
            if [
                "making it satisfy",
                "into satisfying",
                "to satisfy",
                "forces",
                "compels",
                "manipulat",
                "persuad",
                "to do",
            ]
            .iter()
            .any(|v| b.contains(v))
            {
                return "1j";
            }
        }
        return "1i";
    }

    if before.ends_with("relation ") || before.ends_with("relationship ") {
        let b = body.to_lowercase();
        if b.contains("each other") || b.contains("reciprocal") || b.contains("both sides") {
            return "2xx";
        }
        if slot_idx >= 2 {
            return "2ij";
        }
        return "2ix";
    }

    "c"
}

fn guess_frame(body: &str, n: usize) -> String {
    (0..n)
        .map(|i| if i == n - 1 { classify_slot(body, i) } else { "c" })
        .collect::<Vec<_>>()
        .join(" ")
}

fn guess_distribution(body: &str, n: usize) -> String {
    let b = normalize_body(body);
    let n_collective = if b.starts_with("▯ and ▯") {
        2
    } else {
        usize::from(
            b.contains("each other")
                || b.contains("mutual")
                || b.contains("collectively")
                || b.contains("reciprocal")
                || b.contains("both sides"),
        )
    };
    (0..n).map(|i| if i < n_collective { "n" } else { "d" }).collect::<Vec<_>>().join(" ")
}

// ─── valid label sets
// ─────────────────────────────────────────────────────────

const VALID_PRONOUNS: &[&str] = &["hó", "máq", "hóq", "tá"];
const VALID_SUBJECTS: &[&str] = &["A", "I", "E", "P", "S", "F"];

fn is_valid_pronoun(s: &str) -> bool { VALID_PRONOUNS.contains(&s) }
fn is_valid_subject(s: &str) -> bool { VALID_SUBJECTS.contains(&s) }

fn oov_rate(text: &str, model: &LogisticRegression) -> f64 {
    let tokens = tokenize(text);
    if tokens.is_empty() {
        return 0.0;
    }
    let oov = tokens.iter().filter(|t| !model.vocab.contains_key(t.as_str())).count();
    oov as f64 / tokens.len() as f64
}

// ─── top-level
// ────────────────────────────────────────────────────────────────

struct Mismatch {
    line: String,
    max_ml_conf: f64,
}

pub fn run(dict: &[Toa]) -> io::Result<()> {
    fs::create_dir_all("data")?;
    let mut out = fs::File::create("data/guesses.txt")?;

    // ── collect annotated examples ────────────────────────────────────────
    let annotated: Vec<&Toa> = dict
        .iter()
        .filter(|t| {
            t.has_metadata()
                && primary_arity(&t.body).0 > 0
                && t.pronoun.as_deref().is_some_and(is_valid_pronoun)
                && t.subject.as_deref().is_some_and(is_valid_subject)
                && t.scope == "en"
        })
        .collect();

    let cv_examples: Vec<(String, &str, &str, usize)> = annotated
        .iter()
        .map(|t| {
            (
                extract_features(t),
                t.pronoun.as_deref().unwrap(),
                t.subject.as_deref().unwrap(),
                primary_arity(&t.body).0,
            )
        })
        .collect();

    let mut subj_counts = std::collections::HashMap::new();
    for (_, _, s, _) in &cv_examples {
        *subj_counts.entry(s.to_string()).or_insert(0) += 1;
    }

    let total_n = cv_examples.len() as f64;
    let num_subjs = subj_counts.len() as f64;
    let subj_weights: std::collections::HashMap<String, f64> = subj_counts
        .into_iter()
        .map(|(name, count)| (name, total_n / (f64::from(count) * num_subjs)))
        .collect();

    // ── export NB training data ──
    let mut f = fs::File::create("data/nb_export.tsv")?;
    writeln!(f, "features\tpron\tsubj\tarity")?;

    for (features, pron, subj, arity) in &cv_examples {
        let features = features.replace(['\t', '\n'], " ");
        writeln!(f, "{features}\t{pron}\t{subj}\t{arity}")?;
    }

    // ── 10-fold CV with chaining + calibration data ───────────────────────
    eprint!("Running 10-fold CV... ");
    let cv = kfold_cv(&cv_examples, 10);
    eprintln!("done");

    let calibration = Calibration::fit(cv.pron_calibration_data.clone(), 20);

    // ── train final models on all annotated data ──────────────────────────
    let mut pron_counts = std::collections::HashMap::new();
    for (_, p, _, _) in &cv_examples {
        *pron_counts.entry(p.to_string()).or_insert(0) += 1;
    }
    let num_prons = pron_counts.len() as f64;
    let pron_weights: std::collections::HashMap<String, f64> = pron_counts
        .into_iter()
        .map(|(name, count)| (name, total_n / (f64::from(count) * num_prons)))
        .collect();
    let pronoun_model = LogisticRegression::train(
        cv_examples.iter().map(|(f, p, _, _)| (f.as_str(), *p)),
        &pron_weights,
    );
    // subject model: use predicted pronoun as chained feature
    let subj_train: Vec<(String, &str)> = cv_examples
        .iter()
        .map(|(f, _, s, _)| {
            let pred_pron = pronoun_model.predict(f);
            (with_pron_feature(f, &pred_pron), *s)
        })
        .collect();
    let subject_model =
        LogisticRegression::train(subj_train.iter().map(|(f, s)| (f.as_str(), *s)), &subj_weights);

    let mut total = 0;
    let mut n_frame = 0;
    let mut ok_frame = 0;
    let mut n_dist = 0;
    let mut ok_dist = 0;

    let mut mismatches: Vec<Mismatch> = Vec::new();

    for toa in annotated {
        let n = primary_arity(&toa.body);
        if n.0 == 0 {
            continue;
        }
        total += 1;

        // Guesses
        let frame = guess_frame(n.1, n.0);
        let dist = guess_distribution(n.1, n.0);
        let features = extract_features(toa);
        let (pron, p_raw_conf) = pronoun_model.predict_raw(&features);
        let p_cal_conf = calibration.calibrate(p_raw_conf);

        let subj_features = with_pron_feature(&features, &pron);
        let (subj, s_conf) = subject_model.predict_raw(&subj_features);

        // Actuals
        let af = toa.frame.as_deref().unwrap_or("?");
        let ad = toa.distribution.as_deref().unwrap_or("?");
        let ap = toa.pronoun.as_deref().unwrap_or("?");
        let as_ = toa.subject.as_deref().unwrap_or("?");

        let fm = frame == af;
        let dm = dist == ad;
        let pm = pron == ap;
        let sm = subj == as_;

        if fm {
            ok_frame += 1;
        }
        n_frame += 1;
        if dm {
            ok_dist += 1;
        }
        n_dist += 1;

        if !fm || !dm || !pm || !sm {
            let mut tags = Vec::new();
            if !fm {
                tags.push("FRAME");
            }
            if !dm {
                tags.push("DIST");
            }
            if !pm {
                tags.push("PRON");
            }
            if !sm {
                tags.push("SUBJ");
            }

            let tag_str = tags.join("/");
            let line = format!(
                "✗ [{tag_str}] {} #{}\n  actual:  [({af}) ({ad}) {ap} {as_}]\n  guessed: \
                 [({frame}) ({dist}) {pron} {subj}] (conf: p={:.0}%, s={:.0}%)",
                toa.head,
                toa.id,
                p_cal_conf * 100.0,
                s_conf * 100.0
            );

            let max_ml_conf = if !pm {
                1. + p_cal_conf
            } else if !sm {
                s_conf
            } else {
                0.0
            };

            mismatches.push(Mismatch { line, max_ml_conf });
        }
    }

    // Sort: High confidence errors first (likely annotation typos)
    mismatches.sort_by(|a, b| b.max_ml_conf.partial_cmp(&a.max_ml_conf).unwrap());

    let pct = |ok: usize, n: usize| {
        if n == 0 { 0. } else { 100.0 * ok as f64 / n as f64 }
    };

    // ── write summary + discriminative tokens ─────────────────────────────
    pronoun_model.print_top_tokens("pronoun", 10, &mut out)?;
    writeln!(out)?;
    subject_model.print_top_tokens("subject", 10, &mut out)?;
    writeln!(out)?;

    writeln!(out, "=== ACCURACY (n={}) ===", cv_examples.len())?;
    writeln!(out, "  frame:        {:.1}% (heuristic, training data)", pct(ok_frame, n_frame))?;
    writeln!(out, "  distribution: {:.1}% (heuristic, training data)", pct(ok_dist, n_dist))?;
    writeln!(out, "  pronoun:      {:4.1}% (10-fold CV)", cv.pron_acc * 100.0)?;
    let mut pron_classes: Vec<_> = cv.pron_per_class.iter().collect();
    pron_classes.sort_by_key(|(k, _)| k.as_str());
    for &(class, &(correct, total)) in &pron_classes {
        writeln!(
            out,
            "    {class:4} {:4.1}%  ({correct}/{total})",
            100.0 * correct as f64 / total as f64
        )?;
    }
    writeln!(out, "  subject:      {:4.1}% (10-fold CV, chained)", cv.subj_acc * 100.0)?;
    let mut subj_classes: Vec<_> = cv.subj_per_class.iter().collect();
    subj_classes.sort_by_key(|(k, _)| k.as_str());
    for &(class, &(correct, total)) in &subj_classes {
        writeln!(
            out,
            "    {class:4} {:4.1}%  ({correct}/{total})",
            100.0 * correct as f64 / total as f64
        )?;
    }
    writeln!(out)?;

    writeln!(out, "=== PRONOUN CONFIDENCE CALIBRATION ===")?;
    writeln!(out, "  (raw softmax → estimated actual accuracy)")?;
    for &(raw, cal) in &calibration.breakpoints {
        writeln!(out, "  raw {:.0}% → {:.0}%", raw * 100.0, cal * 100.0)?;
    }
    writeln!(out)?;

    // ── annotation QC: mismatches ─────────────────────────────────────────
    writeln!(out, "=== MISMATCHES ON ANNOTATED ENTRIES ({}; annotation QC) ===", mismatches.len())?;
    writeln!(out, "  Sorted by confidence: high % likely indicates a typo in the training data.")?;
    writeln!(out)?;

    for m in &mismatches {
        writeln!(out, "{}", m.line)?;
    }
    writeln!(out)?;

    // ── guess pass ────────────────────────────────────────────────────────
    writeln!(out, "=== GUESSES FOR UNANNOTATED ENTRIES ===")?;
    writeln!(out, "  conf = calibrated pronoun accuracy estimate")?;
    writeln!(out, "  oov = fraction of tokens unseen in training")?;
    writeln!(out)?;

    let mut guesses = vec![];
    let mut n_guessed = 0;
    let mut confidence_sum = 0.;
    let mut oov_sum = 0.;
    let mut high_conf_count = 0;

    for toa in dict.iter().filter(|t| !t.has_metadata() && !t.warn && t.scope == "en") {
        let n = primary_arity(&toa.body);
        if n.0 == 0 {
            continue;
        }

        let frame = guess_frame(n.1, n.0);
        let dist = guess_distribution(n.1, n.0);
        let features = extract_features(toa);
        let (pron, raw_conf) = pronoun_model.predict_raw(&features);
        let cal_conf = calibration.calibrate(raw_conf);
        let subj_features = with_pron_feature(&features, &pron);
        let subj = subject_model.predict(&subj_features);
        let oov = oov_rate(&features, &pronoun_model);

        n_guessed += 1;
        confidence_sum += cal_conf;
        oov_sum += oov;
        if cal_conf >= 0.8 {
            high_conf_count += 1;
        }

        let conf_str = format!("{:.0}%", cal_conf * 100.0);
        let oov_str = if oov > 0.0 { format!(" oov={:.0}%", oov * 100.0) } else { String::new() };

        guesses.push((toa, frame, dist, pron, subj, conf_str, oov_str));
    }
    guesses.sort_by(|a, b| b.5.partial_cmp(&a.5).unwrap_or(std::cmp::Ordering::Equal));
    for (toa, frame, dist, pron, subj, conf_str, oov_str) in guesses {
        writeln!(
            out,
            "{} #{} → [({}) ({}) {} {}] conf={}{}\n  {}",
            toa.head, toa.id, frame, dist, pron, subj, conf_str, oov_str, toa.body
        )?;
    }

    writeln!(out)?;
    writeln!(out, "=== SUMMARY ===")?;
    writeln!(out, "  unannotated entries guessed: {n_guessed}")?;
    writeln!(
        out,
        "  mean calibrated pronoun conf: {:.1}%",
        100.0 * confidence_sum / f64::from(n_guessed)
    )?;
    writeln!(
        out,
        "  mean oov rate:                {:.1}%",
        100.0 * oov_sum / f64::from(n_guessed)
    )?;
    writeln!(out, "  high-confidence (cal≥80%):    {high_conf_count}")?;

    println!("data/guesses.txt: {total} annotated checked, {n_guessed} unannotated guessed");
    println!(
        "10-fold CV accuracy: pronoun {}{:.1}%{RESET}, subject {}{:.1}%{RESET} (chained)",
        color(cv.pron_acc),
        cv.pron_acc * 100.0,
        color(cv.subj_acc),
        cv.subj_acc * 100.0
    );
    Ok(())
}

const RED: &str = "\x1b[91m";
const YELLOW: &str = "\x1b[93m";
const GREEN: &str = "\x1b[92m";
const CYAN: &str = "\x1b[96m";
const BLUE: &str = "\x1b[94m";
const PURPLE: &str = "\x1b[95m";
const RESET: &str = "\x1b[m";
fn color(p: f64) -> String {
    assert!((0.0..=1.0).contains(&p), "uh oh how is p not between 0 and 1");
    let k = 12_f64;
    let i = (6. / (k - 1.) * (k.powf(p) - 1.)).floor() as usize;
    let c = match i {
        0 => RED,
        1 => YELLOW,
        2 => GREEN,
        3 => CYAN,
        4 => BLUE,
        _ => PURPLE,
    };
    c.to_string()
}
