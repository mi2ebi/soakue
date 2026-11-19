"use strict";

// tables

const ONSETS = {
    p: "pʰ", t: "tʰ", k: "kʰ", b: "b", d: "d", g: "ɡ",
    c: "ʦʰ", z: "ʣ", ch: "ʨʰ", j: "ʥ",
    f: "f", s: "s", sh: "ɕ", h: "h",
    m: "m", n: "n", nh: "ɲ", r: "ɾ", l: "l",
    "ꝡ": "ꝡ", // filled in later
    "'": "ʔ",
};
const DIGRAPH_ONSETS = ["ch", "sh", "nh"];

const VOWELS = "aeıou";
const DIPHTHONGS = { aı: "aj", ao: "aw", oı: "ɔj", eı: "ej" };

const GRAVE = "\u0300";
const UNDERDOT = "\u0323"; // marks the last prefix (official spelling)
const MACRON = "\u0304"; // marks a prefix (reform++)
const BREVE = "\u0306"; // extra-short

const TONE_MARKS = { [GRAVE]: "fall", "\u0301": "rise", "\u0308": "glottal", "\u0302": "hiatus" };

// contours as pitch heights, 1 (bottom) to 5 (top)
const CONTOURS = { fall: [5, 1], rise: [1, 5], glottal: [3, 1], hiatus: [5, 1] };
const TONE_GLYPHS = {
    letters: [, "˩", "˨", "˧", "˦", "˥"],
    numbers: [, "¹", "²", "³", "⁴", "⁵"],
};

// what can sit between the previous word and a ꝡ without hiding the sound before it
const NOT_A_SEGMENT = /[\s.ˈ\u032f\u0306ː˥˦˧˨˩¹²³⁴⁵]+$/;
const NON_LETTER = "[^\\p{L}\\p{M}']";
const PUNCTUATION_SPLIT = new RegExp(`^(${NON_LETTER}*)([\\s\\S]*?)(${NON_LETTER}*)$`, "u");

// input

const normalize = (text) =>
      text.normalize("NFD").toLowerCase()
      .replace(/[’‘`x]/g, "'")
      .replace(/i/g, "ı")
      .replace(/[vwy]/g, "ꝡ");

/** a word as base letters, each with the combining marks that follow it */
function splitLetters(word) {
    const letters = [];
    for (const ch of word) {
        if (/\p{M}/u.test(ch) && letters.length) letters.at(-1).marks.push(ch);
        else letters.push({ char: ch, marks: [] });
    }
    return letters;
}

class Cursor {
    constructor(word) {
        this.letters = splitLetters(word);
        this.pos = 0;
    }
    get done() {
        return this.pos >= this.letters.length;
    }
    /** the base letter `ahead` places from here */
    char(ahead = 0) {
        return this.letters[this.pos + ahead]?.char;
    }
    isVowel(ahead = 0) {
        const c = this.char(ahead);
        return c !== undefined && VOWELS.includes(c);
    }
    next() {
        return this.letters[this.pos++];
    }
}

// parsing

/**
 * A raku is onset + nucleus (+ second nucleus) (+ final), e.g. ma, kuaı, buq, chıeq.
 * `vowelsRead` counts vowel letters so far, to check marks sit on a raku's first vowel.
 */
function newRaku() {
    return {
        onset: "", nucleus: "", second: "", coda: "",
        underdot: false, boundary: false, macron: false,
        graveOnSecond: false, vowelsRead: 0,
    };
}

/**
 * Parses one word. Accepts every spelling at once:
 *   official  tone on the word's first vowel, underdot (or - : .) after the last prefix
 *   reform    tone on the stem's first vowel, no underdots
 *   reform++  every prefix raku carries a macron
 *   &'s       also a grave on a raku's second vowel (tıà) as the hiatus tone
 * Returns { rakus, tone, stem (index of the stem's first raku), errs }.
 */
function parseWord(word, hiatusReform) {
    const cursor = new Cursor(word);
    const rakus = [], errs = [], tones = [];
    const ctx = { index: 0, hiatusReform, errs, tones, skipped: false };
    while (!cursor.done) {
        ctx.index = rakus.length;
        const raku = readRaku(cursor, ctx);
        if (raku) rakus.push(raku);
    }
    const { tone, stem } = resolveStem(rakus, tones, errs);
    return { rakus, tone, stem, errs: [...new Set(errs)] };
}

/** reads one raku, or returns null when the letter can't start one */
function readRaku(cursor, ctx) {
    const raku = newRaku();
    if (!readOnset(cursor, ctx, raku)) return null;

    if (cursor.isVowel()) {
        raku.nucleus = readNucleus(cursor, ctx, raku);
        // two nuclei only when the first is a plain vowel: CVV, CVF
        if (raku.nucleus.length === 1 && cursor.isVowel()) raku.second = readNucleus(cursor, ctx, raku);
        readFinal(cursor, ctx, raku);
    } else {
        ctx.errs.push(`no vowel after “${raku.onset}”`);
        // a final right after the missing vowel belongs to it, so it doesn't get reported again
        if (isFinal(cursor)) cursor.next();
    }

    // a hyphen, colon or period after a raku: the stem starts next
    const separator = cursor.char();
    if (separator && ":-.".includes(separator)) {
        raku.boundary = true;
        cursor.next();
    }

    // hiatus reform: a grave on the second vowel is the hiatus tone, if there is one
    if (raku.graveOnSecond) {
        if (raku.second && raku.nucleus.length === 1) ctx.tones.push({ name: "hiatus", raku: ctx.index });
        else ctx.errs.push("the tone mark goes on the first vowel of a raku");
    }
    checkPhonotactics(raku, ctx.errs);
    ctx.skipped = false;
    return raku;
}

function readOnset(cursor, ctx, raku) {
    const c = cursor.char();
    const digraph = c + (cursor.char(1) ?? "");
    if (DIGRAPH_ONSETS.includes(digraph)) {
        raku.onset = digraph;
        cursor.next();
        cursor.next();
    } else if (Object.hasOwn(ONSETS, c)) {
        raku.onset = c;
        cursor.next();
    } else if (VOWELS.includes(c)) {
        // vowel-initial: only the word's first raku can drop its onset
        if (ctx.index > 0 && !ctx.skipped) ctx.errs.push("only the first raku can start with a vowel (use ' for ʔ)");
        raku.onset = "'";
    } else {
        ctx.errs.push(c === "q" ? "q can only be a final" : `unknown letter “${c}”`);
        ctx.skipped = true; // whatever raku comes next isn't really "vowel-initial", it's after junk
        cursor.next();
        return false;
    }
    return true;
}

/** a vowel or a falling diphthong */
function readNucleus(cursor, ctx, raku) {
    let nucleus = readVowel(cursor, ctx, raku);
    if (cursor.isVowel() && DIPHTHONGS[nucleus + cursor.char()]) nucleus += readVowel(cursor, ctx, raku);
    return nucleus;
}

/** consumes one vowel letter and interprets its marks */
function readVowel(cursor, ctx, raku) {
    const letter = cursor.next();
    for (const mark of letter.marks) {
        if (mark === UNDERDOT) {
            if (raku.vowelsRead === 0) raku.underdot = true;
            else ctx.errs.push("the underdot goes on the first vowel of a raku");
        } else if (mark === MACRON) {
            if (raku.vowelsRead === 0) raku.macron = true;
            else ctx.errs.push("the macron goes on the first vowel of a raku");
        } else if (TONE_MARKS[mark]) {
            if (ctx.hiatusReform && mark === GRAVE && raku.vowelsRead === 1) raku.graveOnSecond = true;
            else {
                ctx.tones.push({ name: TONE_MARKS[mark], raku: ctx.index });
                if (raku.vowelsRead) ctx.errs.push("the tone mark goes on the first vowel of a raku");
            }
        } else ctx.errs.push(`unexpected mark on “${letter.char}”`);
    }
    raku.vowelsRead++;
    return letter.char;
}

/** q, or m when it isn't the next raku's onset */
function isFinal(cursor) {
    const c = cursor.char();
    return c === "q" || (c === "m" && !cursor.isVowel(1));
}

function readFinal(cursor, ctx, raku) {
    if (!isFinal(cursor)) return;
    const c = cursor.char();
    if ((raku.second || raku.nucleus).length === 1) raku.coda = c;
    else ctx.errs.push("a diphthong can't take a final");
    cursor.next();
}

function checkPhonotactics(raku, errs) {
    const { nucleus: a, second: b } = raku;
    if (b) {
        if (a === b[0]) errs.push(`repeated vowel “${a}${b[0]}”`);
        else if (a === "a") errs.push("a can only be followed by ı or o, as a diphthong");
        else if (a === "o" && b[0] === "u") errs.push("o can't be followed by u");
    }
    if (raku.onset === "ꝡ" && /^[ıu]/.test(a)) errs.push("ꝡ can't precede ı or u");
    if (raku.onset === "nh" && a[0] === "ı") errs.push("nh can't precede ı");
}

/**
 * Where does the stem start, and what is the tone?
 * Prefix boundaries come from an underdot/hyphen or from ā marks, otherwise from the tone
 * mark itself (reform: the tone sits on the stem's first vowel). No marks at all: no prefix.
 */
function resolveStem(rakus, tones, errs) {
    const tone = tones[0]?.name ?? "fall";
    const toneRaku = tones.at(-1)?.raku ?? -1;
    if (tones.length > 1) errs.push("words can't have multiple tones");

    const hasMacrons = rakus.some((r) => r.macron);
    // indices of rakus followed by an underdot or hyphen (ignored when ā is used)
    const boundaries = hasMacrons ? [] : rakus.flatMap((r, k) => (r.underdot || r.boundary ? [k] : []));
    let stem = 0;
    // once the prefix structure is wrong there is no stem to measure the tone against,
    // so the checks below would only report side effects
    let broken = false;

    if (boundaries.length > 1) {
        errs.push("words can have at most one prefix boundary");
        broken = true;
    } else if (boundaries.length) {
        stem = boundaries[0] + 1;
        if (stem >= rakus.length) {
            errs.push("a prefix needs a stem after it");
            broken = true;
        }
    }
    if (hasMacrons) {
        // reform++: the stem is the first raku without a macron
        stem = rakus.findIndex((r) => !r.macron);
        if (stem < 0) {
            errs.push("a prefix needs a stem after it");
            stem = 0;
            broken = true;
        } else if (rakus.slice(stem).some((r) => r.macron)) {
            errs.push("prefixes (ā) have to come before the stem");
            broken = true;
        }
    }

    if (tones.length === 1 && !broken) {
        if (hasMacrons) {
            if (toneRaku !== stem) errs.push("the tone mark goes on the stem's first vowel");
        } else if (!boundaries.length) stem = toneRaku;
        else if (toneRaku !== 0 && toneRaku !== stem)
            errs.push("the tone mark has to be on the word's first vowel or on the stem's");
        if (toneRaku > 0 && rakus.some((r) => r.underdot)) errs.push("the reform spelling has no underdots");
    }
    return { tone, stem };
}

// syllables

/** rakus → syllables (a CVV raku is two). `closed` means ŋ */
function toSyllables(rakus) {
    const syllables = [];
    rakus.forEach((raku, index) => {
        const two = Boolean(raku.second);
        syllables.push({
            raku: index,
            onset: raku.onset,
            nucleus: raku.nucleus,
            coda: two ? "" : raku.coda,
            closed: !two && raku.coda === "q",
            // ɛ is [e] before a following vowel other than ı
            eBeforeVowel: two && raku.nucleus === "e" && raku.second[0] !== "ı",
            chop: raku.chop,
            endsRaku: !two,
        });
        if (two) {
            syllables.push({
                raku: index,
                onset: "",
                nucleus: raku.second,
                coda: raku.coda,
                closed: raku.coda === "q",
                endsRaku: true,
            });
        }
    });
    return syllables;
}

// transcription

/** writes tone contours as Chao letters or superscript numbers (or nothing) */
function toneWriter(mode) {
    const glyphs = TONE_GLYPHS[mode];
    const level = (height) => (glyphs ? glyphs[height] : "");
    const contour = (from, to) => level(from) + level(to);
    return {
        level,
        contour,
        /** a chop splits the contour at its midpoint: 51 → 53 ʔ 31 */
        split([from, to], chopped) {
            if (!chopped) return { mid: "", end: contour(from, to) };
            const middle = Math.round((from + to) / 2);
            return { mid: contour(from, middle), end: contour(middle, to) };
        },
    };
}

/** closed syllables, and prefixes in the pronunciation reform, take the lax vowels */
function vowelIPA(letter, syllable) {
    const lax = syllable.closed || syllable.prefix;
    return {
        a: "a",
        u: lax ? "ʊ" : "u",
        ı: lax ? "ɪ" : "i",
        o: lax ? "ɔ" : "o",
        e: syllable.eBeforeVowel ? "e" : "ɛ",
    }[letter];
}

/**
 * Quality and length come from the underlying syllable; the chop (ʔ in the first vowel)
 * then divides the length: long → two normal halves, short → two breve halves.
 * `tone.mid` is the tone on the first half of a chopped vowel.
 */
function nucleusIPA(syllable, stressed, tone) {
    const { nucleus } = syllable;
    if (!nucleus) return "";
    if (nucleus.length === 2) {
        const ipa = DIPHTHONGS[nucleus];
        const diphthong = stressed ? ipa.slice(0,1) + "ː" + ipa.slice(1) : ipa;
        return syllable.chop ? vowelIPA(nucleus[0], syllable) + tone.mid + "ʔ" + diphthong : diphthong;
    }
    const vowel = vowelIPA(nucleus, syllable);
    const long = stressed && !syllable.closed;
    if (syllable.chop) {
        const half = long ? "" : BREVE;
        return vowel + half + tone.mid + "ʔ" + vowel + half;
    }
    return vowel + (long ? "ː" : "");
}

/**
 * Syllables → IPA pieces. The stressed syllable carries the tone contour; in the
 * pronunciation reform the prefixes before it get a level mid tone instead.
 */
function renderParts(syllables, { tone, stem, stressedIndex }, style) {
    const { tones } = style;
    syllables.forEach((syllable, k) => {
        syllable.prefix = style.prefixReform && syllable.raku < stem;
        if (k === stressedIndex && tone === "glottal") syllable.chop = true; // the glottal tone is a chop too
    });

    const parts = [];
    const hiatusSecondHalves = new Set();
    syllables.forEach((syllable, k) => {
        const stressed = k === stressedIndex;
        const onset = syllable.onset ? ONSETS[syllable.onset] : "";
        const coda = syllable.coda === "q" ? "ŋ" : syllable.coda;

        if (hiatusSecondHalves.has(syllable)) {
            // &'s: ti35 a51, the hiatus is just the syllable break
            const t = tones.split(CONTOURS.hiatus, syllable.chop);
            parts.push({ str: "ˈ" + nucleusIPA(syllable, true, t) + coda + t.end, onset: "", coda: syllable.coda });
            return;
        }

        const next = syllables[k + 1];
        const t = stressed
              ? tones.split(CONTOURS[tone], syllable.chop)
              : { mid: "", end: syllable.prefix && syllable.endsRaku ? tones.level(3) : "" };

        if (style.hiatusReform && stressed && tone === "hiatus" && next?.raku === syllable.raku) {
            next.chop = syllable.chop;
            syllable.chop = false;
            hiatusSecondHalves.add(next);
            parts.push({
                str: onset + vowelIPA(syllable.nucleus, syllable) + tones.contour(3, 5),
                onset: syllable.onset,
                coda: ""
            });
        } else if (stressed && tone === "hiatus" && syllable.nucleus) {
            // the hiatus tone: a plain copy of the vowel (rising), then the stressed vowel
            parts.push({
                str: onset + vowelIPA(syllable.nucleus[0], syllable) + tones.contour(3, 5),
                onset: syllable.onset,
                coda: "" });
            parts.push({
                str: "ˈ" + nucleusIPA(syllable, true, t) + coda + t.end,
                onset: "",
                coda: syllable.coda
            });
        } else {
            parts.push({
                str: (stressed ? "ˈ" : "") + onset + nucleusIPA(syllable, stressed, t) + coda + t.end,
                onset: syllable.onset,
                coda: syllable.coda,
            });
        }
    });
    return parts;
}

/** joins the pieces: a schwa between two m's, and optionally a dot between syllables */
function joinParts(parts) {
    return parts
        .map((part, k) => {
            if (!k) return part.str;
            const schwa = parts[k - 1].coda === "m" && part.onset === "m" ? "ə" : "";
            return schwa + part.str;
        })
        .join("");
}

/** one word → { errs } or { errs: [], str, firstOnset, lastCoda } */
function transcribeWord(word, style) {
    const parsed = parseWord(word, style.hiatusReform);
    const { errs, rakus, tone, stem } = parsed;
    if (!style.prefixReform && !errs.length && tone === "glottal" && stem > 0)
        errs.push("the official pronunciation can't combine the glottal tone with a prefix");
    if (!rakus.length && !errs.length) errs.push("nothing to transcribe");
    if (errs.length) return { errs };

    // official pronunciation: the underdot chops the last prefix, and raku 0 is stressed.
    // reform pronunciation: no chop, and the stem is stressed
    rakus.forEach((raku, k) => {
        raku.chop = !style.prefixReform && stem > 0 && k === stem - 1;
    });
    const syllables = toSyllables(rakus);
    const stressedIndex = style.prefixReform ? syllables.findIndex((s) => s.raku === stem) : 0;

    const parts = renderParts(syllables, { tone, stem, stressedIndex }, style);
    return {
        errs: [],
        str: joinParts(parts),
        firstOnset: rakus[0].onset,
        lastCoda: rakus.at(-1).coda,
    };
}

// whole text

/** ꝡ is [j] after u/o and [w] otherwise */
function glideAfter(previousText) {
    const last = previousText.replace(NOT_A_SEGMENT, "").slice(-1);
    return last && "uoɔʊw".includes(last)
        ? "j" : last && "iɪeɛj".includes(last) ? "w" : "ꝡ";
}

/** [leading punctuation, the word, trailing punctuation] */
const splitPunctuation = (chunk) => chunk.match(PUNCTUATION_SPLIT).slice(1);

/** a word made only of ā prefixes joins the word after it, so "bō tâo" is one word */
function mergePrefixWords(chunks, hiatusReform) {
    const isPrefixWord = (chunk) => {
        const word = splitPunctuation(chunk)[1];
        if (!word) return false;
        const { rakus } = parseWord(word, hiatusReform);
        return rakus.length > 0 && rakus.every((r) => r.macron);
    };
    const merged = [];
    for (let i = 0; i < chunks.length; i++) {
        let chunk = chunks[i];
        if (!/^\s+$/.test(chunk)) {
            while (/^\s+$/.test(chunks[i + 1] ?? "") && chunks[i + 2] && isPrefixWord(chunk)) {
                chunk += chunks[i + 2];
                i += 2;
            }
        }
        merged.push(chunk);
    }
    return merged;
}

/**
 * options: { tones: "letters" | "numbers" | "none",
 *            pron: "official" | "reform" | "hiatus" | "reform hiatus" }
 * returns { tokens: [{ text, bad? }], warnings }
 */
function convert(text, options) {
    const style = {
        tones: toneWriter(options.tones),
        prefixReform: options.pron.includes("prefix"),
        hiatusReform: options.pron.includes("hiatus"),
    };
    const tokens = [], warnings = [];
    const textSoFar = () => tokens.map((t) => t.text).join("");
    let previousWord = null; // the word just before, if only whitespace separates it from here

    const chunks = normalize(text).split(/(\s+)/).filter(Boolean);
    for (const chunk of mergePrefixWords(chunks, style.hiatusReform)) {
        if (/^\s+$/.test(chunk)) {
            tokens.push({ text: chunk });
            continue;
        }
        const [lead, word, trail] = splitPunctuation(chunk);
        if (lead) {
            tokens.push({ text: lead });
            previousWord = null;
        }
        if (word) {
            const result = transcribeWord(word, style);
            if (result.errs.length) {
                const shown = word.normalize("NFC");
                tokens.push({ text: shown, bad: true });
                warnings.push(shown + ":");
                for (const err of result.errs)
                    warnings.push("• " + err);
                previousWord = null;
            } else {
                // two m's meeting across words also get a schwa: tam moi
                if (previousWord?.lastCoda === "m" && result.firstOnset === "m") previousWord.token.text += "ə";
                const ipa = result.str.replaceAll("ꝡ", (_, offset, whole) => glideAfter(textSoFar() + whole.slice(0, offset)));
                const token = { text: ipa };
                tokens.push(token);
                previousWord = { token, lastCoda: result.lastCoda };
            }
        }
        if (trail) {
            tokens.push({ text: trail });
            previousWord = null;
        }
    }
    return { tokens, warnings };
}

// page

const ui = Object.fromEntries(
    ["src", "brk", "tones", "pron", "out", "warn", "copy"].map((id) => [id, document.getElementById(id)]),
);

function renderOutput(tokens, brackets) {
    ui.out.textContent = "";
    if (brackets) ui.out.append("[");
    for (const token of tokens) {
        if (token.bad) {
            const span = document.createElement("span");
            span.className = "bad";
            span.textContent = token.text;
            ui.out.append(span);
        } else ui.out.append(token.text);
    }
    if (brackets) ui.out.append("]");
}

/** keeps ?q= and ?pron= in the address bar so a conversion can be linked */
function syncUrl(hasText) {
    const url = new URL(location.href);
    if (hasText) url.searchParams.set("q", ui.src.value);
    else url.searchParams.delete("q");
    if (ui.pron.value !== "official") url.searchParams.set("pron", ui.pron.value);
    else url.searchParams.delete("pron");
    history.replaceState(null, "", url);
}

function update() {
    const text = ui.src.value.trim();
    const { tokens, warnings } = convert(text, {
        tones: ui.tones.value,
        pron: ui.pron.value,
    });
    if (text) renderOutput(tokens, ui.brk.checked);
    else ui.out.textContent = "";
    ui.warn.textContent = warnings.join("\n");
    syncUrl(Boolean(text));
}

function init() {
    ui.src.addEventListener("input", update);
    for (const id of ["brk", "tones", "pron"]) ui[id].addEventListener("change", update);
    let copytimer;
    ui.copy.addEventListener("click", () => {
        navigator.clipboard.writeText(ui.out.textContent);
        ui.copy.innerText = "copied";
        ui.copy.style.color = "var(--teal)";
        clearTimeout(copytimer);
        copytimer = setTimeout(() => {
            ui.copy.innerText = "copy";
            ui.copy.style.color = "";
        }, 1000)
    });

    const params = new URLSearchParams(location.search);
    const pron = params.get("pron");
    if ([...ui.pron.options].some((option) => option.value === pron)) ui.pron.value = pron;
    ui.src.value = params.get("q") ?? "";
    update();
    ui.src.focus();
}

init();
