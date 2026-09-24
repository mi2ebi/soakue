// Metadata guesser v2.
// disclaimer! mostly authored by gpt 5.6 luna

#![expect(
    clippy::too_many_lines,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "metadata experiments are intentionally data-heavy"
)]

use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    fs,
    io::{self, Write as _},
    sync::LazyLock,
    time::Instant,
};

use rayon::prelude::*;
use regex::Regex;

use crate::toadua::{Toa, split_into_raku};

const VALID_PRONOUNS: &[&str] = &["hó", "máq", "hóq", "tá"];
const VALID_SUBJECTS: &[&str] = &["sA", "sI", "sE", "sP", "sS", "sF"];
const FRAME_SLOT_LETTERS: &str = "ijk";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
enum Field {
    Frame,
    Distribution,
    Pronoun,
    Subject,
}

impl Field {
    const ALL: [Self; 4] = [Self::Frame, Self::Distribution, Self::Pronoun, Self::Subject];

    const fn name(self) -> &'static str {
        match self {
            Self::Frame => "frame",
            Self::Distribution => "distribution",
            Self::Pronoun => "pronoun",
            Self::Subject => "subject",
        }
    }

    fn get(self, toa: &Toa) -> Option<&str> {
        match self {
            Self::Frame => toa.frame.as_deref(),
            Self::Distribution => toa.distribution.as_deref(),
            Self::Pronoun => toa.pronoun.as_deref(),
            Self::Subject => toa.subject.as_deref(),
        }
    }

    fn valid_value(self, value: &str) -> bool {
        match self {
            Self::Frame | Self::Distribution => !value.is_empty(),
            Self::Pronoun => VALID_PRONOUNS.contains(&value),
            Self::Subject => VALID_SUBJECTS.contains(&value),
        }
    }
}

fn tokenize(text: &str) -> Vec<String> {
    fn flush(current: &mut String, tokens: &mut Vec<String>) {
        if !current.is_empty() {
            if current.starts_with('_') {
                tokens.push(current.clone());
            } else {
                tokens.push(current.to_lowercase());
            }
            current.clear();
        }
    }

    let mut tokens = Vec::new();
    let mut current = String::new();

    for ch in text.chars() {
        if ch == '▯' {
            flush(&mut current, &mut tokens);
            tokens.push("▯".to_string());
        } else if ch.is_alphanumeric() || ch == '_' {
            current.push(ch);
        } else {
            flush(&mut current, &mut tokens);
        }
    }
    flush(&mut current, &mut tokens);
    tokens
}

fn body_word_features(body: &str) -> Vec<String> {
    tokenize(body).into_iter().filter(|t| !t.starts_with('_')).map(|t| format!("_W_{t}")).collect()
}

fn char_ngrams(text: &str, min_n: usize, max_n: usize) -> Vec<String> {
    let chars: Vec<char> = text.to_lowercase().chars().collect();
    let mut out = Vec::new();
    for n in min_n ..= max_n {
        if n > chars.len() {
            break;
        }
        for i in 0 ..= chars.len() - n {
            let gram: String = chars[i .. i + n].iter().collect();
            // Combining marks on their own are almost never a useful feature.
            if gram.chars().all(|c| !c.is_control()) {
                out.push(format!("_C{n}_{gram}"));
            }
        }
    }
    out
}

fn metadata_features(toa: &Toa, target: Field) -> Vec<String> {
    let mut out = Vec::new();

    for (field, value) in [
        (Field::Frame, toa.frame.as_deref()),
        (Field::Distribution, toa.distribution.as_deref()),
        (Field::Pronoun, toa.pronoun.as_deref()),
        (Field::Subject, toa.subject.as_deref()),
    ] {
        if field as u8 != target as u8 {
            if let Some(value) = value {
                out.push(format!("_KNOWN_{}_{}", field.name().to_uppercase(), value));
            } else {
                out.push(format!("_MISSING_{}", field.name().to_uppercase()));
            }
        }
    }

    if let Some(typ) = &toa.typ {
        out.push(format!("_TYPE_{typ}"));
    }
    if let Some(gloss) = &toa.gloss {
        for token in tokenize(gloss) {
            out.push(format!("_GLOSS_{token}"));
        }
    }
    if let Some(tags) = &toa.tags {
        for tag in tags.split(',').map(str::trim).filter(|x| !x.is_empty()) {
            out.push(format!("_TAG_{}", tag.to_lowercase()));
        }
    }

    out
}

fn extract_features(toa: &Toa, target: Field) -> Vec<String> {
    extract_features_extra(toa, target, "")
}

fn extract_features_extra(toa: &Toa, target: Field, extra: &str) -> Vec<String> {
    let mut tokens = tokenize(&toa.body);
    let rakus = split_into_raku(&toa.head).unwrap_or_default();

    if let Some(last) = rakus.last() {
        tokens.push(format!("_RAKU_LAST_{last}"));
    }
    if let Some(first) = rakus.first() {
        tokens.push(format!("_RAKU_FIRST_{first}"));
    }
    for (i, raku) in rakus.iter().enumerate().take(3) {
        tokens.push(format!("_RAKU_POS{i}_{raku}"));
    }
    if rakus.len() >= 2 {
        let a = &rakus[rakus.len() - 2];
        let b = &rakus[rakus.len() - 1];
        tokens.push(format!("_RAKU_LAST2_{a}_{b}"));
    }
    tokens.push(format!("_ARITY_{}", primary_arity(&toa.body).0));

    if toa.head.chars().next().is_some_and(char::is_uppercase) {
        tokens.push("_CAPS".to_string());
    }

    for feature in char_ngrams(&toa.head, 2, 5) {
        tokens.push(feature);
    }
    tokens.extend(body_word_features(&toa.body));
    tokens.extend(metadata_features(toa, target));

    if !extra.is_empty() {
        tokens.push(extra.to_string());
    }

    tokens.push("_BIAS".to_string());

    tokens
}

fn primary_arity(body: &str) -> (usize, &str) {
    body.split([';', '.'])
        .filter(|clause| clause.contains('▯'))
        .map(|clause| (clause.chars().filter(|&c| c == '▯').count(), clause))
        .max_by_key(|(a, _)| *a)
        .unwrap_or_default()
}

fn arity_metadata_consistent(toa: &Toa) -> bool {
    let body_arity = primary_arity(&toa.body).0;

    let frame_arity = toa.frame.as_deref().map(|frame| frame.split_whitespace().count());

    let distribution_arity =
        toa.distribution.as_deref().map(|distribution| distribution.split_whitespace().count());

    frame_arity.is_none_or(|n| n == body_arity)
        && distribution_arity.is_none_or(|n| n == body_arity)
        && match (frame_arity, distribution_arity) {
            (Some(frame), Some(distribution)) => frame == distribution,
            _ => true,
        }
}

fn class_weights(
    labels: impl Iterator<Item = impl AsRef<str>>,
    classes: &[String],
) -> HashMap<String, f64> {
    let mut counts = HashMap::<String, usize>::new();
    let mut n = 0;
    for label in labels {
        *counts.entry(label.as_ref().to_string()).or_insert(0) += 1;
        n += 1;
    }
    let k = classes.len().max(1) as f64;
    counts
        .into_iter()
        .map(|(class, count)| (class, (f64::from(n) / (count as f64 * k)).sqrt()))
        .collect()
}

#[derive(Clone)]
struct LogisticRegression {
    weights: Vec<f64>,
    classes: Vec<String>,
    vocab: HashMap<String, usize>,
}

impl LogisticRegression {
    fn train<'a>(
        examples: impl Iterator<Item = (&'a [String], &'a str)>,
        weights: &HashMap<String, f64>,
        epochs: usize,
        learning_rate: f64,
    ) -> Self {
        let mut class_to_id = HashMap::new();
        let mut vocab = HashMap::new();
        let mut processed_data = Vec::new();

        for (text, label) in examples {
            let next_class_id = class_to_id.len();
            let c_id = *class_to_id.entry(label.to_string()).or_insert(next_class_id);
            let token_ids = text
                .iter()
                .map(|token| {
                    if let Some(&token_id) = vocab.get(token) {
                        token_id
                    } else {
                        let token_id = vocab.len();
                        vocab.insert(token.clone(), token_id);
                        token_id
                    }
                })
                .collect::<Vec<_>>();
            processed_data.push((token_ids, c_id));
        }

        let num_classes = class_to_id.len();
        let num_tokens = vocab.len();
        let mut classes = vec![String::new(); num_classes];
        for (name, id) in class_to_id {
            classes[id] = name;
        }

        let mut model = Self { weights: vec![0.; num_tokens * num_classes], classes, vocab };

        for epoch in 0 .. epochs {
            let lr = learning_rate / 0.05_f64.mul_add(epoch as f64, 1.);
            for (token_ids, label_id) in &processed_data {
                let mut scores = vec![0.; num_classes];
                for (c_idx, score) in scores.iter_mut().enumerate() {
                    for &t_id in token_ids {
                        *score += model.weights[t_id * num_classes + c_idx];
                    }
                }

                let max_score = scores.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                let exps: Vec<f64> = scores.iter().map(|s| (s - max_score).exp()).collect();
                let sum_exps: f64 = exps.iter().sum();

                for (c_idx, e) in exps.iter().enumerate() {
                    let prob = e / sum_exps;
                    let target = if c_idx == *label_id { 1. } else { 0. };
                    let error = target - prob;
                    let class_weight = weights.get(&model.classes[c_idx]).copied().unwrap_or(1.);
                    for &t_id in token_ids {
                        model.weights[t_id * num_classes + c_idx] = (lr * error)
                            .mul_add(class_weight, model.weights[t_id * num_classes + c_idx]);
                    }
                }
            }
        }

        model
    }

    fn probs_from_tokens(&self, tokens: &[String]) -> Vec<f64> {
        let num_classes = self.classes.len();
        if num_classes == 0 {
            return Vec::new();
        }
        let mut scores = vec![0.; num_classes];
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
        exps.into_iter().map(|e| e / sum_exps).collect()
    }

    fn predict_raw_tokens(&self, tokens: &[String]) -> Prediction {
        let probs = self.probs_from_tokens(tokens);
        let mut best = None;
        let mut second = None;

        for (i, &prob) in probs.iter().enumerate() {
            match best {
                None => best = Some((i, prob)),
                Some((_, best_prob)) if prob > best_prob => {
                    second = best;
                    best = Some((i, prob));
                }
                Some(_) => {
                    if second.is_none_or(|(_, second_prob)| prob > second_prob) {
                        second = Some((i, prob));
                    }
                }
            }
        }

        let best = best.map_or(0, |(i, _)| i);
        let second = second.map(|(i, _)| i);
        Prediction {
            label: self.classes.get(best).cloned().unwrap_or_default(),
            raw_conf: probs.get(best).copied().unwrap_or(0.),
            margin: probs.get(best).copied().unwrap_or(0.)
                - second.and_then(|i| probs.get(i).copied()).unwrap_or(0.),
            margin_label: second.and_then(|i| self.classes.get(i)).cloned().unwrap_or_default(),

            probs,
        }
    }

    fn predict_raw_allowed_tokens<F>(&self, tokens: &[String], allowed: F) -> Prediction
    where F: Fn(&str) -> bool {
        let probs = self.probs_from_tokens(tokens);
        let mut allowed_mass = 0.;
        let mut best = None;
        let mut second = None;

        for (i, class) in self.classes.iter().enumerate() {
            if !allowed(class) {
                continue;
            }

            let prob = probs[i];
            allowed_mass += prob;

            match best {
                None => best = Some((i, prob)),
                Some((_, best_prob)) if prob > best_prob => {
                    second = best;
                    best = Some((i, prob));
                }
                Some(_) => {
                    if second.is_none_or(|(_, second_prob)| prob > second_prob) {
                        second = Some((i, prob));
                    }
                }
            }
        }

        let Some((best, best_prob)) = best else {
            // `c` is valid for every arity. This keeps the structural invariant
            // even if a future training set somehow contains no valid model
            // class.
            return Prediction {
                label: "c".to_string(),
                raw_conf: 0.,
                margin: 0.,
                margin_label: String::new(),
                probs,
            };
        };

        let normalizer = if allowed_mass > 0. { allowed_mass } else { 1. };

        let best_prob = best_prob / normalizer;
        let second_prob = second.map_or(0., |(_, prob)| prob / normalizer);

        Prediction {
            label: self.classes[best].clone(),
            raw_conf: best_prob,
            margin: best_prob - second_prob,
            margin_label: second
                .and_then(|(i, _)| self.classes.get(i))
                .cloned()
                .unwrap_or_default(),
            probs,
        }
    }

    fn class_probability_tokens(&self, tokens: &[String], class: &str) -> f64 {
        let probs = self.probs_from_tokens(tokens);
        self.classes
            .iter()
            .position(|c| c == class)
            .and_then(|i| probs.get(i).copied())
            .unwrap_or(0.)
    }

    fn oov_rate_tokens(&self, tokens: &[String]) -> f64 {
        if tokens.is_empty() {
            return 0.;
        }
        let oov = tokens.iter().filter(|t| !self.vocab.contains_key(t.as_str())).count();
        oov as f64 / tokens.len() as f64
    }
}

#[derive(Clone)]
struct Prediction {
    label: String,
    raw_conf: f64,
    margin: f64,
    margin_label: String,
    probs: Vec<f64>,
}

#[derive(Clone)]
struct SubjectModel {
    is_i: LogisticRegression,
    non_i: LogisticRegression,
}

impl SubjectModel {
    fn train(examples: &[(Vec<String>, String)], epochs: usize) -> Self {
        let classes = vec!["sI".to_string(), "NON_I".to_string()];
        let weights = class_weights(
            examples.iter().map(|(_, label)| if label == "sI" { "sI" } else { "NON_I" }),
            &classes,
        );
        let is_i = LogisticRegression::train(
            examples.iter().map(|(features, label)| {
                (features.as_slice(), if label == "sI" { "sI" } else { "NON_I" })
            }),
            &weights,
            epochs,
            0.1,
        );

        let non_i_classes = examples
            .iter()
            .filter(|(_, label)| label != "sI")
            .map(|(_, label)| label.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let non_i_weights = class_weights(
            examples.iter().filter(|(_, label)| label != "sI").map(|(_, label)| label.as_str()),
            &non_i_classes,
        );
        let non_i = LogisticRegression::train(
            examples
                .iter()
                .filter(|(_, label)| label != "sI")
                .map(|(features, label)| (features.as_slice(), label.as_str())),
            &non_i_weights,
            epochs,
            0.1,
        );

        Self { is_i, non_i }
    }

    fn predict_raw_tokens(&self, tokens: &[String]) -> Prediction {
        let p_i = self.is_i.class_probability_tokens(tokens, "sI");
        let non_i = self.non_i.predict_raw_tokens(tokens);
        let mut probs = Vec::new();
        let mut classes = vec!["sI".to_string()];
        probs.push(p_i);
        for (class, p) in self.non_i.classes.iter().zip(non_i.probs.iter()) {
            classes.push(class.clone());
            probs.push((1. - p_i) * p);
        }

        let mut best = None;
        let mut second = None;

        for (i, &prob) in probs.iter().enumerate() {
            match best {
                None => best = Some((i, prob)),
                Some((_, best_prob)) if prob > best_prob => {
                    second = best;
                    best = Some((i, prob));
                }
                Some(_) => {
                    if second.is_none_or(|(_, second_prob)| prob > second_prob) {
                        second = Some((i, prob));
                    }
                }
            }
        }

        let best = best.map_or(0, |(i, _)| i);
        let second = second.map(|(i, _)| i);

        Prediction {
            label: classes.get(best).cloned().unwrap_or_default(),
            raw_conf: probs.get(best).copied().unwrap_or(0.),
            margin: probs.get(best).copied().unwrap_or(0.)
                - second.and_then(|i| probs.get(i).copied()).unwrap_or(0.),
            margin_label: second.and_then(|i| classes.get(i)).cloned().unwrap_or_default(),
            probs,
        }
    }
}

#[derive(Default, Clone)]
struct Calibration {
    breakpoints: Vec<(f64, f64)>,
}

impl Calibration {
    fn fit(mut results: Vec<(f64, bool)>, buckets: usize) -> Self {
        if results.is_empty() {
            return Self::default();
        }
        results.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal));
        let buckets = buckets.max(1);
        let step = 1. / buckets as f64;
        let mut grouped: Vec<Vec<(f64, bool)>> = vec![Vec::new(); buckets];
        for pair in results {
            let idx = ((pair.0 / step).floor() as usize).min(buckets - 1);
            grouped[idx].push(pair);
        }

        // PAVA with weights, not the old pairwise midpoint approximation.
        #[derive(Clone, Copy)]
        #[allow(clippy::items_after_statements, reason = "")]
        struct Bin {
            x: f64,
            y: f64,
            n: usize,
        }
        let mut bins = Vec::new();
        for group in grouped.into_iter().filter(|g| !g.is_empty()) {
            let n = group.len();
            let x = group.iter().map(|(c, _)| *c).sum::<f64>() / n as f64;
            let y = group.iter().filter(|(_, ok)| *ok).count() as f64 / n as f64;
            bins.push(Bin { x, y, n });
        }

        let mut pooled: Vec<Bin> = Vec::new();
        for bin in bins {
            pooled.push(bin);
            while pooled.len() >= 2 {
                let b = pooled[pooled.len() - 1];
                let a = pooled[pooled.len() - 2];
                if a.y <= b.y {
                    break;
                }
                let n = a.n + b.n;
                let merged = Bin {
                    x: b.x.mul_add(b.n as f64, a.x * a.n as f64) / n as f64,
                    y: b.y.mul_add(b.n as f64, a.y * a.n as f64) / n as f64,
                    n,
                };
                pooled.pop();
                pooled.pop();
                pooled.push(merged);
            }
        }

        Self { breakpoints: pooled.into_iter().map(|b| (b.x, b.y)).collect() }
    }

    fn calibrate(&self, raw: f64) -> f64 {
        if self.breakpoints.is_empty() {
            return raw;
        }
        if raw <= self.breakpoints[0].0 {
            return self.breakpoints[0].1;
        }
        for window in self.breakpoints.windows(2) {
            assert_eq!(window.len(), 2, "universe broke {}", line!());
            let (x0, y0) = window[0];
            let (x1, y1) = window[1];
            if raw <= x1 {
                let t = {
                    let t = (raw - x0) / (x1 - x0);
                    if t.is_finite() { t } else { 1. }
                };
                return f64::mul_add(t, y1 - y0, y0);
            }
        }
        self.breakpoints.last().map_or(raw, |(_, y)| *y)
    }
}

#[derive(Default)]
struct FieldReport {
    correct: usize,
    total: usize,
    calibration_data: Vec<(f64, bool)>,
    per_class: HashMap<String, (usize, usize)>,
    // For factorized fields such as distribution, measure each component
    // independently in addition to measuring exact whole-value correctness.
    slot_correct: usize,
    slot_total: usize,
    per_slot: HashMap<usize, (usize, usize)>,
    // Frame accuracy broken down by arity.
    per_arity: HashMap<usize, (usize, usize)>,
}
impl FieldReport {
    fn merge(mut self, other: Self) -> Self {
        self.correct += other.correct;
        self.total += other.total;
        self.calibration_data.extend(other.calibration_data);
        for (k, v) in other.per_class {
            let entry = self.per_class.entry(k).or_default();
            entry.0 += v.0;
            entry.1 += v.1;
        }
        self.slot_correct += other.slot_correct;
        self.slot_total += other.slot_total;
        for (k, v) in other.per_slot {
            let entry = self.per_slot.entry(k).or_default();
            entry.0 += v.0;
            entry.1 += v.1;
        }
        for (k, v) in other.per_arity {
            let entry = self.per_arity.entry(k).or_default();
            entry.0 += v.0;
            entry.1 += v.1;
        }
        self
    }
}

fn deterministic_hash(text: &str) -> u64 {
    // Stable FNV-1a; no random dependency needed for a reproducible experiment.
    let mut h = 0xcbf2_9ce4_8422_2325_u64;
    for b in text.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

fn grouped_stratified_folds(examples: &[&Toa], field: Field, k: usize) -> Vec<Vec<usize>> {
    let k = k.max(2).min(examples.len().max(2));
    let mut groups: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, example) in examples.iter().enumerate() {
        groups.entry(example.head.as_str()).or_default().push(i);
    }

    let mut groups = groups.into_iter().collect::<Vec<_>>();
    groups.sort_by(|a, b| {
        let sa = deterministic_hash(a.0);
        let sb = deterministic_hash(b.0);
        sb.cmp(&sa)
    });

    let mut folds = vec![Vec::new(); k];
    let mut fold_class_counts: Vec<HashMap<String, usize>> = vec![HashMap::new(); k];
    let mut fold_sizes = vec![0; k];

    for (_, indices) in groups {
        let mut group_counts = HashMap::<String, usize>::new();
        for &i in &indices {
            if let Some(label) = field.get(examples[i]) {
                *group_counts.entry(label.to_string()).or_insert(0) += 1;
            }
        }

        let mut best_fold = 0;
        let mut best_score = f64::INFINITY;
        for fold in 0 .. k {
            let mut score = fold_sizes[fold] as f64;
            for (label, count) in &group_counts {
                let current = *fold_class_counts[fold].get(label).unwrap_or(&0) as f64;
                score = current.mul_add(1. + *count as f64, score);
            }
            if score < best_score {
                best_score = score;
                best_fold = fold;
            }
        }

        for &i in &indices {
            folds[best_fold].push(i);
        }
        fold_sizes[best_fold] += indices.len();
        for (label, count) in group_counts {
            *fold_class_counts[best_fold].entry(label).or_insert(0) += count;
        }
    }

    folds
}

fn field_examples(dict: &[Toa], field: Field) -> Vec<&Toa> {
    dict.iter()
        .filter(|t| {
            !t.warn
                && t.scope == "en"
                && !t.head.ends_with('-')
                && (1 ..= 3).contains(&primary_arity(&t.body).0)
                && arity_metadata_consistent(t)
                && field.get(t).is_some_and(|value| match field {
                    Field::Frame => valid_frame_value(value, primary_arity(&t.body).0),
                    _ => field.valid_value(value),
                })
        })
        .collect()
}

fn frame_last_slot(value: &str) -> Option<&str> { value.split_whitespace().last() }

fn valid_frame_value(value: &str, n: usize) -> bool {
    let slots = value.split_whitespace().collect::<Vec<_>>();

    if slots.len() != n || n == 0 {
        return false;
    }

    slots[.. n - 1].iter().all(|&slot| slot == "c") && valid_frame_last_slot(slots[n - 1], n)
}

fn valid_frame_last_slot(label: &str, n: usize) -> bool {
    if n == 0 {
        return false;
    }

    // The ordinary all-canonical frame is represented by `c`.
    if label == "c" {
        return true;
    }

    if n.saturating_sub(1) > FRAME_SLOT_LETTERS.chars().count() {
        return false;
    }

    let digit_count = label.chars().take_while(char::is_ascii_digit).count();
    if digit_count == 0 {
        return false;
    }

    let Some((number, suffix)) = label.get(.. digit_count).zip(label.get(digit_count ..)) else {
        return false;
    };
    let Ok(k) = number.parse::<usize>() else {
        return false;
    };

    if suffix.chars().count() != k {
        return false;
    }

    let allowed_letters = FRAME_SLOT_LETTERS
        .chars()
        .take(n.saturating_sub(1))
        .chain(std::iter::once('x'))
        .collect::<HashSet<_>>();

    suffix.chars().all(|ch| allowed_letters.contains(&ch))
}

static RE_THE_CASE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\S+\s+){0,4}the case\b").unwrap());
static RE_IS_TRUE_FALSE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(is true|is false)\b").unwrap());
static RE_THAT_WHETHER_IF: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(that|whether|if)\s*$").unwrap());
static RE_PROPERTY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(property|satisf\w+|to do|doing)\s*$").unwrap());
static RE_MAKING_IT_THEM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("making (it|them)").unwrap());
static RE_RELATION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\brelation\w*\s*$").unwrap());

fn field_training_examples(train: &[&Toa], field: Field) -> Vec<(Vec<String>, String)> {
    let mut examples = Vec::new();

    for toa in train {
        let Some(value) = field.get(toa) else {
            continue;
        };

        match field {
            Field::Frame => {
                let Some(label) = frame_last_slot(value) else {
                    continue;
                };

                examples.push((extract_features(toa, field), label.to_string()));
            }

            Field::Distribution => {
                let n = primary_arity(&toa.body).0;

                for (slot, label) in value.split_whitespace().enumerate() {
                    let extra = format!("_DIST_SLOT_{slot}_OF_{n}");

                    examples.push((extract_features_extra(toa, field, &extra), label.to_string()));
                }
            }

            Field::Pronoun | Field::Subject => {
                examples.push((extract_features(toa, field), value.to_string()));
            }
        }
    }

    examples
}

fn train_field_models(train: &[&Toa], field: Field) -> (LogisticRegression, Option<SubjectModel>) {
    let examples = field_training_examples(train, field);

    if matches!(field, Field::Subject) {
        // Prediction uses the hierarchical model. Its `is_i` model sees the
        // same examples/features as the old plain subject model did, so it
        // also supplies the identical vocabulary for OOV reporting without
        // requiring another training pass.
        let hierarchical = SubjectModel::train(&examples, 55);
        let plain = hierarchical.is_i.clone();
        (plain, Some(hierarchical))
    } else {
        let classes = examples
            .iter()
            .map(|(_, label)| label.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let weights = class_weights(examples.iter().map(|(_, label)| label.as_str()), &classes);
        let model = LogisticRegression::train(
            examples.iter().map(|(features, label)| (features.as_slice(), label.as_str())),
            &weights,
            55,
            0.08,
        );
        (model, None)
    }
}

fn field_feature_tokens(toa: &Toa, field: Field) -> Vec<Vec<String>> {
    match field {
        Field::Distribution => {
            let n = primary_arity(&toa.body).0;

            (0 .. n)
                .map(|slot| {
                    let extra = format!("_DIST_SLOT_{slot}_OF_{n}");
                    extract_features_extra(toa, field, &extra)
                })
                .collect()
        }

        _ => vec![extract_features(toa, field)],
    }
}

fn field_oov_rate(feature_sets: &[Vec<String>], model: &LogisticRegression) -> f64 {
    if feature_sets.is_empty() {
        return 0.;
    }

    feature_sets.iter().map(|tokens| model.oov_rate_tokens(tokens)).sum::<f64>()
        / feature_sets.len() as f64
}

fn predict_field_from_features(
    toa: &Toa,
    field: Field,
    feature_sets: &[Vec<String>],
    model: &LogisticRegression,
    subject: Option<&SubjectModel>,
) -> Prediction {
    match field {
        Field::Frame => {
            let n = primary_arity(&toa.body).0;
            let tokens = feature_sets.first().map_or_else(|| &[], Vec::as_slice);

            let last =
                model.predict_raw_allowed_tokens(tokens, |label| valid_frame_last_slot(label, n));

            let mut slots = Vec::with_capacity(n);
            slots.resize(n.saturating_sub(1), "c");
            slots.push(last.label.as_str());

            Prediction {
                label: slots.join(" "),
                raw_conf: last.raw_conf,
                margin: last.margin,
                margin_label: last.margin_label,
                probs: last.probs,
            }
        }

        Field::Distribution => {
            let n = primary_arity(&toa.body).0;
            let mut slots = Vec::with_capacity(n);
            let mut raw_conf = 1.;
            let mut margin = 1_f64;
            let margin_label = String::new();

            for tokens in feature_sets.iter().take(n) {
                let prediction = model.predict_raw_tokens(tokens);

                slots.push(prediction.label);
                raw_conf *= prediction.raw_conf;

                if prediction.margin < margin {
                    margin = prediction.margin;
                }
            }

            Prediction { label: slots.join(" "), raw_conf, margin, margin_label, probs: Vec::new() }
        }

        Field::Subject => {
            let tokens = feature_sets.first().map_or_else(|| &[], Vec::as_slice);

            subject.map_or_else(
                || model.predict_raw_tokens(tokens),
                |subject| subject.predict_raw_tokens(tokens),
            )
        }

        Field::Pronoun => {
            let tokens = feature_sets.first().map_or_else(|| &[], Vec::as_slice);
            model.predict_raw_tokens(tokens)
        }
    }
}

fn predict_field(
    toa: &Toa,
    field: Field,
    model: &LogisticRegression,
    subject: Option<&SubjectModel>,
) -> Prediction {
    let feature_sets = field_feature_tokens(toa, field);

    predict_field_from_features(toa, field, &feature_sets, model, subject)
}

fn guess_frame_heuristic(body: &str, n: usize) -> String {
    if n == 0 {
        return String::new();
    }
    let mut frame = vec!["c"; n];
    let last_pos = body.rfind('▯').unwrap_or(0);
    let before = &body[.. last_pos];
    let after = &body[last_pos + '▯'.len_utf8() ..];
    let after_trimmed = after.trim_start();
    let lower = body.to_lowercase();

    let last = if RE_THE_CASE.is_match(after_trimmed)
        || RE_IS_TRUE_FALSE.is_match(after_trimmed)
        || RE_THAT_WHETHER_IF.is_match(before)
    {
        "0"
    } else if RE_PROPERTY.is_match(before) {
        match n.cmp(&2) {
            Ordering::Greater => {
                let is_manip = RE_MAKING_IT_THEM.is_match(&lower)
                    || lower.contains("gets")
                    || lower.contains("into")
                    || lower.contains("to do");
                if is_manip { "1j" } else { "1i" }
            }
            Ordering::Equal => "1i",
            Ordering::Less => "1x",
        }
    } else if RE_RELATION.is_match(before) {
        match n.cmp(&2) {
            Ordering::Greater => "2ij",
            Ordering::Equal => "2ix",
            Ordering::Less => "2xx",
        }
    } else {
        "c"
    };
    frame[n - 1] = last;
    frame.join(" ")
}

fn guess_distribution_heuristic(entry: &Toa, n: usize) -> String {
    let b = entry.body.to_lowercase();
    let n_collective = if b.starts_with("▯ and ▯") {
        2
    } else {
        usize::from(
            b.contains("each other")
                || b.contains("mutual")
                || b.contains("collectively")
                || b.contains("reciprocal")
                || b.contains("both sides")
                || entry.head.ends_with("gua"),
        )
    };
    (0 .. n).map(|i| if i < n_collective { "n" } else { "d" }).collect::<Vec<_>>().join(" ")
}

fn per_field_cv(annotated: &[&Toa], field: Field, k: usize) -> FieldReport {
    let folds = grouped_stratified_folds(annotated, field, k);

    folds
        .into_par_iter()
        .filter(|test_indices| !test_indices.is_empty())
        .map(|test_indices| {
            let mut report = FieldReport::default();
            let test_set: HashSet<usize> = test_indices.iter().copied().collect();
            let train = annotated
                .iter()
                .enumerate()
                .filter_map(|(i, t)| (!test_set.contains(&i)).then_some(*t))
                .collect::<Vec<_>>();

            if train.is_empty() {
                return report;
            }

            let mut models =
                HashMap::<Option<usize>, (LogisticRegression, Option<SubjectModel>)>::new();

            models.insert(None, train_field_models(&train, field));

            if matches!(field, Field::Frame) {
                let arities =
                    train.iter().map(|toa| primary_arity(&toa.body).0).collect::<HashSet<_>>();

                for arity in arities {
                    let arity_train = train
                        .iter()
                        .copied()
                        .filter(|toa| primary_arity(&toa.body).0 == arity)
                        .collect::<Vec<_>>();

                    if !arity_train.is_empty() {
                        models.insert(Some(arity), train_field_models(&arity_train, field));
                    }
                }
            }

            for &i in &test_indices {
                let toa = annotated[i];
                let Some(actual) = field.get(toa) else {
                    continue;
                };

                let key = matches!(field, Field::Frame).then_some(primary_arity(&toa.body).0);
                let (plain, subject) =
                    models.get(&key).or_else(|| models.get(&None)).expect("fold model exists");

                let prediction = predict_field(toa, field, plain, subject.as_ref());

                let correct = match field {
                    Field::Distribution => {
                        prediction.label.split_whitespace().eq(actual.split_whitespace())
                    }
                    _ => prediction.label == actual,
                };

                report.correct += usize::from(correct);
                report.total += 1;
                report.calibration_data.push((prediction.raw_conf, correct));
                if matches!(field, Field::Frame) {
                    let arity = primary_arity(&toa.body).0;
                    let entry = report.per_arity.entry(arity).or_insert((0, 0));
                    entry.0 += usize::from(correct);
                    entry.1 += 1;
                }

                let entry = report.per_class.entry(actual.to_string()).or_insert((0, 0));
                entry.0 += usize::from(correct);
                entry.1 += 1;

                if matches!(field, Field::Distribution) {
                    let actual_slots = actual.split_whitespace().collect::<Vec<_>>();
                    let predicted_slots = prediction.label.split_whitespace().collect::<Vec<_>>();
                    let slot_count = actual_slots.len().max(predicted_slots.len());

                    for slot in 0 .. slot_count {
                        let slot_correct = actual_slots.get(slot) == predicted_slots.get(slot);

                        report.slot_correct += usize::from(slot_correct);
                        report.slot_total += 1;

                        let entry = report.per_slot.entry(slot).or_insert((0, 0));
                        entry.0 += usize::from(slot_correct);
                        entry.1 += 1;
                    }
                }
            }
            report
        })
        .reduce(FieldReport::default, FieldReport::merge)
}
fn fit_final_models(
    dict: &[Toa],
    reports: &HashMap<Field, FieldReport>,
) -> (HashMap<FieldKey, LogisticRegression>, SubjectModel, HashMap<FieldKey, Calibration>) {
    let results: Vec<_> = Field::ALL
        .into_par_iter()
        .map(|field| {
            let mut models = HashMap::new();
            let mut calibrations = HashMap::new();

            let report = reports.get(&field).expect("CV report exists");
            let examples = field_examples(dict, field);

            let global_key = FieldKey { field, arity: None };

            let calibration = Calibration::fit(report.calibration_data.clone(), 20);
            calibrations.insert(global_key, calibration.clone());

            let (plain, subject) = train_field_models(&examples, field);
            models.insert(global_key, plain);

            if matches!(field, Field::Frame) {
                let arities =
                    examples.iter().map(|toa| primary_arity(&toa.body).0).collect::<HashSet<_>>();

                for arity in arities {
                    let arity_examples = examples
                        .iter()
                        .copied()
                        .filter(|toa| primary_arity(&toa.body).0 == arity)
                        .collect::<Vec<_>>();

                    if !arity_examples.is_empty() {
                        let (arity_plain, _) = train_field_models(&arity_examples, field);
                        let key = FieldKey { field, arity: Some(arity) };

                        models.insert(key, arity_plain);
                        calibrations.insert(key, calibration.clone());
                    }
                }
            }
            (models, subject, calibrations)
        })
        .collect();

    let mut all_models = HashMap::new();
    let mut all_calibrations = HashMap::new();
    let mut subject_model = None;

    for (models, subject, calibrations) in results {
        all_models.extend(models);
        all_calibrations.extend(calibrations);
        if let Some(s) = subject {
            subject_model = Some(s);
        }
    }

    (all_models, subject_model.expect("subject model trained"), all_calibrations)
}

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
struct FieldKey {
    field: Field,
    arity: Option<usize>,
}

fn field_key(toa: &Toa, field: Field) -> FieldKey {
    FieldKey { field, arity: matches!(field, Field::Frame).then_some(primary_arity(&toa.body).0) }
}

fn model_for_toa<'a>(
    models: &'a HashMap<FieldKey, LogisticRegression>,
    toa: &Toa,
    field: Field,
) -> &'a LogisticRegression {
    let key = field_key(toa, field);

    models
        .get(&key)
        // A frame arity with no training data falls back to the global frame model.
        .or_else(|| models.get(&FieldKey { field, arity: None }))
        .expect("final model exists")
}

fn calibration_for_toa<'a>(
    calibrations: &'a HashMap<FieldKey, Calibration>,
    toa: &Toa,
    field: Field,
) -> Option<&'a Calibration> {
    let key = field_key(toa, field);

    calibrations.get(&key).or_else(|| calibrations.get(&FieldKey { field, arity: None }))
}

fn al_item_priority(
    toa: &Toa,
    models: &HashMap<FieldKey, LogisticRegression>,
    subject_model: &SubjectModel,
    calibrations: &HashMap<FieldKey, Calibration>,
    annotated_heads: &HashSet<&str>,
) -> f64 {
    let mut priority = 0.;

    for field in Field::ALL {
        if field.get(toa).is_some() {
            continue;
        }
        let model = model_for_toa(models, toa, field);
        let features = field_feature_tokens(toa, field);
        let pred = predict_field_from_features(
            toa,
            field,
            &features,
            model,
            matches!(field, Field::Subject).then_some(subject_model),
        );
        let oov = field_oov_rate(&features, model);
        let cal = calibration_for_toa(calibrations, toa, field)
            .map_or(pred.raw_conf, |c| c.calibrate(pred.raw_conf));
        let uncertainty = 1. - cal;
        let disagreement = match field {
            Field::Frame => f64::from(
                pred.label != guess_frame_heuristic(&toa.body, primary_arity(&toa.body).0),
            ),
            Field::Distribution => f64::from(
                pred.label != guess_distribution_heuristic(toa, primary_arity(&toa.body).0),
            ),
            _ => 0.,
        };
        let class_rarity = if pred.label == "sI" || pred.label == "hóq" { 0.02 } else { 0.08 };
        let field_priority =
            0.20_f64.mul_add(disagreement, 0.35_f64.mul_add(oov, uncertainty)) + class_rarity;
        priority += field_priority;
    }

    if !annotated_heads.contains(toa.head.as_str()) {
        priority += 0.10;
    }

    priority
}

pub fn run(dict: &[Toa]) -> io::Result<()> {
    println!("\nguessing");
    let start = Instant::now();
    fs::create_dir_all("data")?;
    let mut out = fs::File::create("data/guesses.txt")?;

    let complete_annotated = dict
        .iter()
        .filter(|t| {
            !t.warn
                && t.scope == "en"
                && !t.head.ends_with('-')
                && (1 ..= 3).contains(&primary_arity(&t.body).0)
                && arity_metadata_consistent(t)
                && Field::ALL.iter().all(|field| {
                    field.get(t).is_some_and(|value| match field {
                        Field::Frame => valid_frame_value(value, primary_arity(&t.body).0),
                        _ => field.valid_value(value),
                    })
                })
        })
        .collect::<Vec<_>>();

    writeln!(out, "=== PRODUCTION-SHAPED 10-FOLD CV ===")?;
    writeln!(
        out,
        "Each fold groups identical heads together; the target field is hidden, while other \
         metadata remains available as features."
    )?;
    writeln!(out, "complete examples (all four fields present): {}", complete_annotated.len())?;
    writeln!(out)?;

    println!("- 10fold cv for everything");
    let mut reports = HashMap::new();
    for field in Field::ALL {
        let examples = field_examples(dict, field);
        let report = per_field_cv(&examples, field, 10);
        let pct =
            if report.total == 0 { 0. } else { 100. * report.correct as f64 / report.total as f64 };

        if matches!(field, Field::Distribution) {
            writeln!(
                out,
                "{}: exact {:5.1}% ({}/{}) [training examples: {}]",
                field.name(),
                pct,
                report.correct,
                report.total,
                examples.len()
            )?;

            let slot_pct = if report.slot_total == 0 {
                0.
            } else {
                100. * report.slot_correct as f64 / report.slot_total as f64
            };

            writeln!(
                out,
                "  per-slot: {:5.1}% ({}/{})",
                slot_pct, report.slot_correct, report.slot_total
            )?;

            let mut slots = report.per_slot.iter().collect::<Vec<_>>();
            slots.sort_by_key(|&(slot, _)| *slot);

            for (slot, (correct, total)) in slots {
                writeln!(
                    out,
                    "  slot {:<2}  {:5.1}% ({correct}/{total})",
                    slot + 1,
                    100. * *correct as f64 / *total as f64
                )?;
            }
        } else {
            writeln!(
                out,
                "{}: {:5.1}% ({}/{}) [training examples: {}]",
                field.name(),
                pct,
                report.correct,
                report.total,
                examples.len()
            )?;
            if matches!(field, Field::Frame) {
                let mut arities = report.per_arity.iter().collect::<Vec<_>>();
                arities.sort_by_key(|&(arity, _)| *arity);

                for (arity, (correct, total)) in arities {
                    writeln!(
                        out,
                        "  arity {arity:<2} {:5.1}% ({correct}/{total})",
                        100. * *correct as f64 / *total as f64
                    )?;
                }
            }
        }

        let mut classes = report.per_class.iter().collect::<Vec<_>>();
        classes.sort_by_key(|&(class, _)| class.clone());

        for (class, (correct, total)) in classes {
            writeln!(
                out,
                "  {class:12} {:5.1}% ({correct}/{total})",
                100. * *correct as f64 / *total as f64
            )?;
        }

        writeln!(out)?;

        reports.insert(field, report);
    }

    let heuristic_frame = complete_annotated
        .iter()
        .filter(|t| {
            let guessed = guess_frame_heuristic(&t.body, primary_arity(&t.body).0);
            t.frame.as_deref() == Some(guessed.as_str())
        })
        .count();
    let heuristic_dist = complete_annotated
        .iter()
        .filter(|t| {
            let guessed = guess_distribution_heuristic(t, primary_arity(&t.body).0);
            t.distribution.as_deref() == Some(guessed.as_str())
        })
        .count();
    println!("- heuristic frames/distributions");
    writeln!(out, "=== RULE-BASED BASELINES ===")?;
    writeln!(
        out,
        "frame heuristic: {:5.1}% ({}/{})",
        100. * heuristic_frame as f64 / complete_annotated.len() as f64,
        heuristic_frame,
        complete_annotated.len()
    )?;
    writeln!(
        out,
        "distribution heuristic: {:5.1}% ({}/{})",
        100. * heuristic_dist as f64 / complete_annotated.len() as f64,
        heuristic_dist,
        complete_annotated.len()
    )?;
    writeln!(out)?;

    let (models, subject_model, calibrations) = fit_final_models(dict, &reports);

    writeln!(out, "=== CALIBRATION ===")?;
    for field in Field::ALL {
        let key = FieldKey { field, arity: None };
        let calibration = &calibrations[&key];

        if matches!(field, Field::Frame) {
            writeln!(out, "frame (pooled across arities):")?;
        } else {
            writeln!(out, "{}:", field.name())?;
        }

        for &(raw, cal) in &calibration.breakpoints {
            writeln!(out, "  raw {:.0}% -> {:.0}%", 100. * raw, 100. * cal)?;
        }
    }
    writeln!(out)?;

    println!("- training data sanity check");
    writeln!(out, "=== TRAINING-DATA SANITY CHECK ===")?;
    for field in Field::ALL {
        let examples = field_examples(dict, field);

        let (correct, total) = examples
            .par_iter()
            .map(|toa| {
                let actual = field.get(toa).unwrap();
                let model = model_for_toa(&models, toa, field);
                let prediction = predict_field(
                    toa,
                    field,
                    model,
                    matches!(field, Field::Subject).then_some(&subject_model),
                );

                let is_correct = match field {
                    Field::Distribution => {
                        prediction.label.split_whitespace().eq(actual.split_whitespace())
                    }
                    _ => prediction.label == actual,
                };

                (usize::from(is_correct), 1)
            })
            .reduce(|| (0, 0), |a, b| (a.0 + b.0, a.1 + b.1));

        writeln!(
            out,
            "{}: {:5.1}% ({}/{})",
            field.name(),
            100. * correct as f64 / f64::from(total),
            correct,
            total
        )?;
    }
    writeln!(out)?;

    println!("- writing guesses");
    writeln!(out, "=== GUESSES FOR ENTRIES NEEDING METADATA ===")?;

    let mut guesses_to_write: Vec<_> = dict
        .par_iter()
        .filter(|t| {
            !t.warn
                && t.scope == "en"
                && !t.head.ends_with('-')
                && (1 ..= 3).contains(&primary_arity(&t.body).0)
                && arity_metadata_consistent(t)
                && Field::ALL.iter().any(|field| field.get(t).is_none())
        })
        .filter_map(|toa| {
            let mut fields_str = Vec::new();
            let mut missing_fields = Vec::new();

            for field in Field::ALL {
                if field.get(toa).is_some() {
                    continue;
                }
                let model = model_for_toa(&models, toa, field);
                let feature_tokens = field_feature_tokens(toa, field);
                let prediction = predict_field_from_features(
                    toa,
                    field,
                    &feature_tokens,
                    model,
                    matches!(field, Field::Subject).then_some(&subject_model),
                );
                let oov = field_oov_rate(&feature_tokens, model);
                let confidence = calibration_for_toa(&calibrations, toa, field)
                    .map_or(prediction.raw_conf, |c| c.calibrate(prediction.raw_conf));

                fields_str.push(format!(
                    "  {} = {} ({:.0}%, margin {:.0}%{}{}, oov {:.0}%)",
                    field.name(),
                    prediction.label,
                    confidence * 100.,
                    prediction.margin * 100.,
                    if prediction.margin_label.is_empty() { "" } else { " " },
                    prediction.margin_label,
                    oov * 100.
                ));
                missing_fields.push(field);
            }
            if fields_str.is_empty() {
                None
            } else {
                let priority = al_item_priority(
                    toa,
                    &models,
                    &subject_model,
                    &calibrations,
                    &HashSet::from_iter(complete_annotated.iter().map(|t| t.head.as_str())),
                );
                Some((priority, toa, fields_str, missing_fields))
            }
        })
        .collect();

    // Sort descending by priority score (highest priority / most uncertain
    // first)
    guesses_to_write.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    let mut guessed = 0;
    let mut per_field_count = HashMap::<Field, usize>::new();

    for (priority, toa, fields_str, missing_fields) in guesses_to_write {
        guessed += 1;
        for field in missing_fields {
            *per_field_count.entry(field).or_insert(0) += 1;
        }
        writeln!(
            out,
            "{} #{} priority={:.2} ->\n{}\n  {}",
            toa.head,
            toa.id,
            priority,
            fields_str.join("\n"),
            toa.body
        )?;
    }

    writeln!(out)?;
    writeln!(out, "entries needing metadata: {guessed}")?;
    for field in Field::ALL {
        writeln!(
            out,
            "  {} missing: {}",
            field.name(),
            per_field_count.get(&field).copied().unwrap_or(0)
        )?;
    }

    eprintln!("- done with {} in {:?}", complete_annotated.len(), start.elapsed());
    Ok(())
}
