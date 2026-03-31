// this file was written by claude for an experiment

//! Heuristic metadata guesser for Toaq dictionary entries.
//!
//! Writes `data/guesses.txt` with accuracy stats on the already-annotated
//! entries followed by metadata guesses for everything that has ▯ but no
//! existing annotation.
//!
//! The four metadata fields and their stored representations:
//!   frame        — space-separated slots: "c", "c c", "c 1i", "0", …
//!   distribution — space-separated: "d", "d d", "n d", …
//!   animacy      — stored un-toned: "ho", "maq", "hoq", "ta", "rai"
//!                  (Display adds the acute: hó máq hóq tá ráı)
//!   subject      — single capital: "I" "F" "A" "E" "P" "S"

use std::{
    fs,
    io::{self, Write as _},
};

use crate::toadua::Toa;

// ─── helpers ──────────────────────────────────────────────────────────────────

fn count_blanks(body: &str) -> usize {
    body.chars().filter(|&c| c == '▯').count()
}

// ─── frame ────────────────────────────────────────────────────────────────────

/// Classify one argument slot.
/// Returns a &'static str like "c", "0", "1i", "1j", "1x", "2xx", "2ix", "2ij".
fn classify_slot(body: &str, slot_idx: usize) -> &'static str {
    let parts: Vec<&str> = body.split('▯').collect();
    let before = parts.get(slot_idx).copied().unwrap_or("").to_lowercase();
    let after_raw = parts.get(slot_idx + 1).copied().unwrap_or("");
    let after = after_raw.to_lowercase();
    let after_trim = after.trim_start_matches(|c: char| " ;,./".contains(c));

    // ── Proposition slot (0) ──────────────────────────────────────────────
    // Evidence: "▯ is the case", "▯ begins/occurs/takes place",
    //           "that ▯", "whether ▯"
    if after_trim.starts_with("is the case")
        || after_trim.starts_with("is not the case")
        || after_trim.starts_with("is true")
        || after_trim.starts_with("begins")
        || after_trim.starts_with("occurs")
        || after_trim.starts_with("takes place")
        || after_trim.starts_with("holds")
        || before.ends_with("that ")
        || before.ends_with("whether ")
        || before.ends_with("if ")
    {
        return "0";
    }

    // ── Property slot (1*) ────────────────────────────────────────────────
    // Evidence: text surrounding the blank mentions "property" or "satisfies"
    let is_property_slot = before.ends_with("property ")
        || before.ends_with("satisfies ")
        || before.ends_with("satisfying ")
        || before.ends_with("has property ")
        // definitions like "▯ is enough; ▯ sufficiently satisfies property ▯"
        // — the blank comes *after* "satisfies property", not before
        || before.contains("satisfies property")
        || before.contains("satisfy property")
        // "for doing ▯" / "instructions for ▯" / "concept of satisfying ▯"
        || before.ends_with("for doing ")
        || before.ends_with("for starting ")
        || before.ends_with("concept of satisfying ")
        || before.ends_with("way of ");

    if is_property_slot {
        // 1x: the property is reified (turned into an object), not applied to any arg.
        //   "concept of satisfying ▯", "community of ▯-ers",
        //   "instructions for ▯", "energy for ▯", "recipe for ▯"
        if before.contains("concept of")
            || before.contains("community of")
            || before.ends_with("instructions for ")
            || before.ends_with("recipe for ")
            || before.ends_with("energy for ")
            || before.ends_with("idea of ")
            || before.ends_with("notion of ")
            || before.ends_with("knowledge of ")
            || before.ends_with("art of ")
            || before.ends_with("skill ")
            || before.ends_with("ability to ")
        {
            return "1x";
        }

        // 1j: the *object* (a later arg) satisfies the property.
        //   Indicates by causative / manipulation context when the property
        //   slot is the third or later argument.
        let body_lower = body.to_lowercase();
        if slot_idx >= 2 {
            let causative = [
                "making it satisfy",
                "into satisfying",
                "to satisfy",
                "entrusts",
                "tricks",
                "shoves",
                "thrusts",
                "pushes",
                "jams",
                "dunks",
                "hurls",
                "forces",
                "compels",
                "manipulat",
                "persuad",
                "teaches",
            ];
            if causative.iter().any(|v| body_lower.contains(v)) {
                return "1j";
            }
        }

        return "1i";
    }

    // ── Relation slot (2*) ────────────────────────────────────────────────
    // Evidence: "in relation ▯", "satisfies relation ▯", "relationship ▯"
    if before.ends_with("relation ")
        || before.ends_with("in relation ")
        || before.ends_with("satisfies relation ")
        || before.ends_with("relationship ")
        || before.ends_with("connected by relation ")
    {
        let body_lower = body.to_lowercase();
        // 2xx: the relation holds between two non-specified things
        //   "connected by relation ▯ with each other", "reciprocal relationship ▯"
        if body_lower.contains("each other")
            || body_lower.contains("reciprocal")
            || body_lower.contains("consecutive")
            || body_lower.contains("both sides")
        {
            return "2xx";
        }
        // 2ij: relates subject to object
        if slot_idx >= 2 {
            return "2ij";
        }
        // 2ix: applies between subject and some unspecified x
        return "2ix";
    }

    "c"
}

fn guess_frame(body: &str, n: usize) -> String {
    (0..n)
        .map(|i| classify_slot(body, i))
        .collect::<Vec<_>>()
        .join(" ")
}

// ─── distribution ─────────────────────────────────────────────────────────────

fn guess_distribution(body: &str, n: usize, frame: &str) -> String {
    let b = body.to_lowercase();
    let frame_slots: Vec<&str> = frame.split_whitespace().collect();

    // The subject slot is non-distributive when the predicate holds of a group
    // collectively rather than each member individually.
    let collective_subject = b.contains("each other")
        || b.contains("mutual")
        || b.contains("collectively")
        || b.contains("reciprocal")
        || b.contains("annihilat")
        || b.contains("both sides");

    (0..n)
        .map(|i| {
            // Distribution of a relation/property slot over multiple values is
            // almost always fine → "d".  Only mark "n" for the subject when we
            // have strong collective evidence.
            let _slot = frame_slots.get(i).copied().unwrap_or("c");
            if collective_subject && i == 0 {
                "n"
            } else {
                "d"
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// ─── animacy ──────────────────────────────────────────────────────────────────
//
// Stored without tone marks:
//   "ho"  → hó   animate (person / animal)
//   "maq" → máq  inanimate concrete / geographic
//   "hoq" → hóq  abstract / linguistic / propositional
//   "ta"  → tá   neutral (adjective / property)
//   "rai" → ráı  only used for the word raı itself

fn guess_animacy(body: &str, frame: &str) -> &'static str {
    let b = body.to_lowercase();
    let frame_slots: Vec<&str> = frame.split_whitespace().collect();

    // If the subject *is* a proposition or event (0-arity subject slot) → hóq
    if frame_slots.first().copied() == Some("0") && frame_slots.len() == 1 {
        return "hóq";
    }

    // Propositional / logical language → hóq
    if b.contains("is the case")
        || b.contains("is not the case")
        || b.contains("entails")
        || b.contains("implies")
        || b.contains("necessarily")
        || b.contains("possibly")
        || b.contains("proposition")
        || b.contains("is true")
    {
        return "hóq";
    }

    // Temporal predicates whose subject is an event → hóq
    if b.contains("takes place")
        || b.contains("begins to occur")
        || b.contains("starts to occur")
    {
        return "hóq";
    }

    // Linguistic / abstract cultural artifacts → hóq
    if b.contains("language")
        || b.contains(" word")
        || b.contains("letter ")
        || b.contains("conlang")
        || b.contains("loglang")
        || b.contains(" story")
        || b.contains("fiction")
        || b.contains(" game")
        || b.contains("software")
        || b.contains("technology")
        || b.contains("sentence")
        || b.contains(" text ")
        || b.contains("symbol")
        || b.contains("concept")
        || b.contains("video game")
        || b.contains("alphabet")
        || b.contains("anecdote")
        || b.contains("information")
        || b.contains("knowledge")
    {
        return "hóq";
    }

    // Deliberate actions / animate behaviour → hó
    let agent_verbs = [
        " noms", "chomps", " eats", " runs", " walks", " speaks",
        "sneezes", "bumps ", " hits ", "utters", "gargles", "pokes ",
        "blesses", "leeches", "uses up", "pays back", "promises",
        "refrains", "holds off", " sings", " dances", " jumps",
        " builds", " writes", "decides", "chooses", " attempts",
        "entrusts", "tricks", "shoves", "pushes", "hurls", "teaches",
        "struts", "alieves", "is a father", "is a mother", "is a parent",
        "is a child", "is a weeb",
    ];
    if agent_verbs.iter().any(|v| b.contains(v)) {
        return "hó";
    }

    // Generic people / animal nouns → hó
    if b.contains("person")
        || b.contains("human")
        || b.contains("people")
        || b.contains(" agent")
        || b.contains("creature")
        || b.contains(" being")
        || b.contains("animal")
        || b.contains("insect")
        || b.contains("arthropod")
        || b.contains("organism")
        || b.contains("lifeform")
    {
        return "hó";
    }

    // Geographic / physical place or object → máq
    if b.contains("country")
        || b.contains(" city")
        || b.contains(" place")
        || b.contains("location")
        || b.contains("region")
        || b.contains("continent")
        || b.contains("pertains to")      // country/culture predicates
        || b.contains("is the country")
    {
        return "máq";
    }

    // Default: tá — a neutral adjective/property with no animacy constraint
    "tá"
}

// ─── subject type ─────────────────────────────────────────────────────────────
//
// From the subject-type table:
//   A (agent)       deliberate action, subject must be capable of intent
//   I (individual)  concrete thing or creature; not necessarily deliberate
//   S (shape)       spatial / dimensional; can be thing, creature, or event
//   F (free)        anything — the broadest possible
//   E (event)       specifically an event
//   P (proposition) specifically a proposition

fn guess_subject(body: &str, frame: &str) -> &'static str {
    let b = body.to_lowercase();
    let frame_slots: Vec<&str> = frame.split_whitespace().collect();

    // 0-arity subject slot → either P or E depending on body language
    if frame_slots.first().copied() == Some("0") && frame_slots.len() == 1 {
        if b.contains("is the case")
            || b.contains("is not the case")
            || b.contains("is true")
            || b.contains("in retrospect")
            || b.contains("possibly")
            || b.contains("necessarily")
            || b.contains("proposition")
            || b.contains("entails")
        {
            return "P";
        }
        return "E"; // "begins", "occurs", "takes place", etc.
    }

    // Explicit proposition language
    if b.contains("is the case")
        || b.contains("is not the case")
        || b.contains("is true")
        || b.contains("entails")
        || b.contains("implies")
        || b.contains("proposition")
        || b.contains("necessarily")
        || b.contains("possibly")
    {
        return "P";
    }

    // Explicit event language
    if b.contains("takes place")
        || b.contains("begins to occur")
        || b.contains("starts to occur")
        || b.contains("(event)")
        || b.contains("is an event")
        || b.contains("happens")
        || b.contains("occurs")
    {
        return "E";
    }

    // Agent: deliberate, volitional action
    let agent_indicators = [
        " noms", "chomps", " eats", " runs", " walks", " speaks", " says ",
        " writes", " builds", "decides", "chooses", "sneezes", " hits ",
        "bumps into", "uses up", "pays back", "blesses", "leeches",
        "gargles", "pokes ", "utters", " sings", " dances", " jumps",
        "promises ", "entrusts", "tricks", "shoves", "pushes", "hurls",
        "refrains", "holds off", " attempts", "teaches", "struts",
        "alieves",
    ];
    if agent_indicators.iter().any(|v| b.contains(v)) {
        return "A";
    }

    // Shape: spatial, dimensional, orientational predicates
    // (applies to things, creatures, and events alike)
    let shape_indicators = [
        " long", " tall", " wide", " big ", " short", " broad", " narrow",
        " deep", "shallow", " thick", " thin", " high", " low",
        " large", " small", " far ", " near",
        "north of", "south of", "east of", "west of",
        " above", " below", " inside", "outside",
        "intersects", "crosses", "traverses",
    ];
    if shape_indicators.iter().any(|w| b.contains(w)) {
        return "S";
    }

    // Individual: concrete physical properties that work for things / animals
    let individual_indicators = [
        " hard", " soft", " hot", " cold", " bright", " dark",
        " wet", " dry", " sharp", " round",
        "white", "black", " red", " blue", "green", "yellow",
        " color", "thousand", " active", "awake", " flat",
        " acid", "plant", " tree", "insect", "arthropod", "fungus",
        " heavy", " light", " fast", " slow",
        "is a father", "is a mother", "is a parent",
        "is a child", "is a weeb",
        " is a bee", " is a worm", " is a bear", " is a cat",
    ];
    if individual_indicators.iter().any(|w| b.contains(w)) {
        return "I";
    }

    // Default: F — no restriction on what the subject can be
    "F"
}

// ─── top-level ────────────────────────────────────────────────────────────────

struct Guess {
    frame: String,
    distribution: String,
    animacy: &'static str,
    subject: &'static str,
}

fn guess(toa: &Toa) -> Option<Guess> {
    let n = count_blanks(&toa.body);
    if n == 0 {
        return None;
    }
    let frame = guess_frame(&toa.body, n);
    let distribution = guess_distribution(&toa.body, n, &frame);
    let animacy = guess_animacy(&toa.body, &frame);
    let subject = guess_subject(&toa.body, &frame);
    Some(Guess { frame, distribution, animacy, subject })
}

pub fn run(dict: &[Toa]) -> io::Result<()> {
    fs::create_dir_all("data")?;
    let mut out = fs::File::create("data/guesses.txt")?;

    // ── accuracy pass ─────────────────────────────────────────────────────
    writeln!(out, "=== MISMATCHES ON ANNOTATED ENTRIES ===")?;
    writeln!(out)?;

    let mut total = 0usize;
    let mut n_frame = 0usize;
    let mut n_dist  = 0usize;
    let mut n_anim  = 0usize;
    let mut n_subj  = 0usize;
    let mut ok_frame = 0usize;
    let mut ok_dist  = 0usize;
    let mut ok_anim  = 0usize;
    let mut ok_subj  = 0usize;
    let mut ok_all   = 0usize;

    for toa in dict.iter().filter(|t| t.has_metadata()) {
        let Some(g) = guess(toa) else { continue };
        total += 1;

        let af = toa.frame.as_deref();
        let ad = toa.distribution.as_deref();
        let aa = toa.pronoun.as_deref();
        let as_ = toa.subject.as_deref();

        let fm = af.map(|v| g.frame        == v);
        let dm = ad.map(|v| g.distribution == v);
        let am = aa.map(|v| g.animacy      == v);
        let sm = as_.map(|v| g.subject     == v);

        if let Some(b) = fm { n_frame += 1; if b { ok_frame += 1; } }
        if let Some(b) = dm { n_dist  += 1; if b { ok_dist  += 1; } }
        if let Some(b) = am { n_anim  += 1; if b { ok_anim  += 1; } }
        if let Some(b) = sm { n_subj  += 1; if b { ok_subj  += 1; } }

        if fm.unwrap_or(true) && dm.unwrap_or(true)
            && am.unwrap_or(true) && sm.unwrap_or(true)
        {
            ok_all += 1;
        }

        let any_wrong = matches!(fm, Some(false))
            || matches!(dm, Some(false))
            || matches!(am, Some(false))
            || matches!(sm, Some(false));

        if any_wrong {
            writeln!(out, "✗ {} #{}", toa.head, &toa.id)?;
            writeln!(
                out,
                "  actual:  [({}) ({}) {} {}]",
                af.unwrap_or("?"), ad.unwrap_or("?"),
                aa.unwrap_or("?"), as_.unwrap_or("?")
            )?;
            writeln!(
                out,
                "  guessed: [({}) ({}) {} {}]",
                g.frame, g.distribution, g.animacy, g.subject
            )?;
        }
    }

    let pct = |ok: usize, n: usize| {
        if n == 0 { 0.0_f64 } else { 100.0 * ok as f64 / n as f64 }
    };

    writeln!(out)?;
    writeln!(out, "=== ACCURACY ({total} annotated entries with ▯) ===")?;
    writeln!(out, "  frame:        {ok_frame}/{n_frame} ({:.1}%)", pct(ok_frame, n_frame))?;
    writeln!(out, "  distribution: {ok_dist}/{n_dist} ({:.1}%)",   pct(ok_dist,  n_dist))?;
    writeln!(out, "  animacy:      {ok_anim}/{n_anim} ({:.1}%)",   pct(ok_anim,  n_anim))?;
    writeln!(out, "  subject:      {ok_subj}/{n_subj} ({:.1}%)",   pct(ok_subj,  n_subj))?;
    writeln!(out, "  all fields:   {ok_all}/{total} ({:.1}%)",      pct(ok_all,   total))?;

    // ── guess pass ────────────────────────────────────────────────────────
    writeln!(out)?;
    writeln!(out, "=== GUESSES FOR UNANNOTATED ENTRIES ===")?;
    writeln!(out)?;

    let mut n_guessed = 0usize;
    for toa in dict.iter().filter(|t| !t.has_metadata()) {
        let Some(g) = guess(toa) else { continue };
        n_guessed += 1;
        writeln!(
            out,
            "{} #{} → [({}) ({}) {} {}]",
            toa.head,
            &toa.id[..toa.id.len().min(8)],
            g.frame, g.distribution, g.animacy, g.subject
        )?;
    }

    println!(
        "data/guesses.txt: {total} annotated checked, {n_guessed} unannotated guessed"
    );
    Ok(())
}