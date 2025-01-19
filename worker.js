importScripts("data/toakue.js");
let escapeHTML = s => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
let error = (words, err) => ({ err: words.join(`« <code>${escapeHTML(err)}</code> »`) });
const orders = {
    default: (a, b) => b[1] - a[1],
    random:  (a, b) => Math.random() - 0.5,
    alpha:   (a, b) => dict.indexOf(a[0]) - dict.indexOf(b[0]),
    newest:  (a, b) => new Date(b[0].date) - new Date(a[0].date),
    score:   (a, b) => b[0].score - a[0].score
}
function search(q) {
    let terms = q.split(" ");
    terms = terms.map(term => {
        let [_, operator, query] = term.match(/^(==|[=~@#/$!^-]|[a-z]*:)(.*)/) ?? [];
        if (!operator) return {op: "", orig: term, value: term.toLowerCase()};
        let colon = operator.endsWith(":");
        operator = operator.replace(/:$/, "");
        const operators = ["head", "body", "user", "score", "id", "scope", "arity", "not", "order"];
        if (colon && !operators.includes(operator))
            return error`bu jıq mıjóaıchase ${operator}`;
        if (["/", "arity"].includes(operator) && !/^[0-9]?$/.test(query))
            return error`bu tıozıu mí ${query} (kïo tıao máo kóam kı)`;
        if (["^", "score"].includes(operator) && isNaN(query.replace(/^=/, "")))
            return error`bu zıu mí ${query.replace(/^=/, "")}`;
        if (["head", "=", "~"].includes(operator)) {
            let regex = queryToRegex(query);
            if (regex.err) return regex;
        }
        if (operator == "order") {
            if (terms.length == 1)
                return {err: "sua pó méuq joaıteoq"};
            if (!orders[query])
                return error`bu chase suım mí ${query}`;
        }
        return {
            op: operator,
            orig: query,
            value: query.toLowerCase()
        };
    });
    if (terms.filter(t => t.op == "order").length > 1)
        return error`bu daı gaoshì pó mí ${"order"}`;
    let err = terms.find(t => t.err);
    if (err) return err;
    let excluded = terms
        .filter(t => ["!", "-", "not"].includes(t.op))
        .map(t => search(t.orig));
    err = excluded.find(e => e.err);
    if (err) return err;
    excluded = new Set(excluded.flat().map(e => e[0].id));
    let res = [];
    for (const entry of dict) {
        if (excluded.has(entry.id)) continue;
        let arities = entry.body.split(/[;.?!]/).map(b => b.split("▯").length - 1);
        if (!arities.every(x => x == 0)) {
            arities = arities.filter(x => x != 0);
        }
        let scores = terms.filter(t => t.op != "order").map(({op, orig, value}) => {
            // 6: id
            if (["#", "id"].includes(op) && entry.id == orig) return 6;
            // 5: head
            if (["=", "head", "~", ""].includes(op) && compareish(normalize_query(value), normalize(entry.head))) return 5.2;
            if (!op && compareish(normalizeToneless(value), normalizeToneless(entry.head))) return 5.1;
            // and regex matching
            if (["=", "head", "~"].includes(op)) {
                let regex = queryToRegex(normalize_query(orig, false), op != '~');
                if (regex.test(normalize(entry.head))) return 5;
            }
            // 3: body
            if (["body", ""].includes(op)) {
                const v = normalize_query(value).replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
                const body = normalize(entry.body);
                if (RegExp(`▯ ?(is|are)?( an?)? ([^ /▯]+/)*${v}`, "iu").test(body)) return 3.2
                if (RegExp(`([^'’]\\b|(?!['’])\\W|^)${v}`, "iu").test(body)) return 3.1
                if (body.includes(normalize_query(value))) return 3;
            }
            // 1-2: no op
            if (!op) {
                if (entry.notes.some(n => normalize(n.content).includes(normalize_query(value)))) return 2;
                if (normalize(entry.head).startsWith(normalize_query(value))) return 1.1;
                if (normalizeToneless(entry.head).includes(normalizeToneless(value))) return 1;
            }
            // other
            if (
                ["@", "user"].includes(op) && entry.user.toLowerCase() == value.toLowerCase()
                || ["$", "scope"].includes(op) && entry.scope.toLowerCase() == value.toLowerCase()
                || ["/", "arity"].includes(op) && arities.includes(+value)
                || ["^", "score"].includes(op) && (entry.score >= value || entry.score == value.replace(/^=/, ""))
                || ["!", "-", "not"].includes(op)
            ) return 0.1;
        })
        if (scores.some(s => !s)) continue;
        let bonus = entry.user == "official" ? 0.3 :
            entry.user == "oldofficial" || /^(old)?(countries|examples)$/.test(entry.user) ? -0.3 : 0;
        bonus += entry.score / 20;
        res.push([entry, Math.max(...scores) + bonus]);
    }
    return res.sort(orders[(terms.find(t => t.op == "order") || {value: "default"}).value]);
}
const tones = `\u0300\u0301\u0308\u0302`;
const underdot = `\u0323`;
const vowels = `aeıou`;
const char_match = `(?:.[${tones}]?${underdot}?)`;
const vowel_match = `(?:[${vowels}][${tones}]?${underdot}?)`;
const init_consonants = `(?:[mpbfntdczsrljꝡkg'h]|[ncs]h)`;
const letter = `(?:${vowel_match}|${init_consonants}|q)`;
const finals = `[mq]`;
const diphthongs = `([aeo]ı|ao)`;
const raku = `((?<= |^)|${init_consonants})${vowel_match}?(${diphthongs}|${vowel_match}${finals}?)`;
let substitutions = {
    '*': '.*',
    '?': letter,
    'C': init_consonants,
    'V': vowel_match,
    'F': diphthongs,
    'Q': finals,
    'R': raku,
    '_': ' ',
}
// If a tone is present in the query, it's required in the word; if not present any tone(s) are allowed.
// Underdots are dealt with separately, so query nạbie matches word nạ́bıe
for (let vowel of vowels) {
    substitutions[vowel] = `${vowel}[${tones}]?${underdot}?`
    substitutions[vowel + underdot] = `${vowel}[${tones}]?${underdot}`
    for (let tone of tones) {
        substitutions[vowel + tone] = `${vowel}${tone}${underdot}?`
    }
}
const word_diacritic_regex = new RegExp(`(${letter}+)([1234])`, "iug");
const diacritic_tones = {
    '1': '\u0300',
    '2': '\u0301',
    '3': '\u0308',
    '4': '\u0302',
}
const vowel_regex = new RegExp(`${vowel_match}`, "iu");
const underdot_regex = new RegExp(`(${raku})([\.])`, "iug");
const isTone = c => /^[\u0300\u0301\u0308\u0302\u0323]$/.test(c);
// attach a cache to a function, so that it doesn't recalculate the same values
const memoize = fn => {
    const cache = new Map();
    return (...args) => {
        let hash = args.join("\x00");
        if (cache.has(hash)) return cache.get(hash);
        let res = fn(...args);
        cache.set(hash, res);
        return res;
    }
}
const normalizeToneless = memoize(w => [...normalize(w)].filter(c => !isTone(c)).join(""));
// for regex search purposes, we don't want to convert to lowercase since C/F/Q/R/V exist
const normalize = memoize((w, lowercase = true) =>
    (lowercase ? w.toLowerCase() : w)
        .normalize("NFD")
        .replace(/i/g, "ı")
        .replace(/[vw]/g, "ꝡ")
        .replace(/[x‘’]/g, "'")
        .replace(/\u0323([\u0301\u0308\u0302])/, "$1\u0323"))
// queries also have underdot and number replacements, which can be dealt with separately (and are somewhat expensive)
const normalize_query = memoize((w, lowercase = true) =>
    normalize(w, lowercase).replace(word_diacritic_regex, (_, word, number) =>
        word.replace(vowel_regex, c => c + diacritic_tones[number])
    ).replace(underdot_regex, (_, word) =>
        word.replace(vowel_regex, c => c + underdot)
    )
);
// handle prefix hyphens
const compareish = (query, word) => query == word || query == word.replace(/-$/, "");
const char_regex = new RegExp(`${char_match}`, "iug");
const char_brackets_regex = new RegExp(`\\[${char_match}*?\\]`, "iug");
const queryToRegex = memoize((query, anchored = true) => {
    // due to [...] not being true character classes, we can't directly substitute them
    // and instead have to turn [abc] into (a|b|c)
    let compiled = query
        .replace(char_brackets_regex, c => `(${c.slice(1, -1).match(char_regex)?.join("|") ?? ''})`)
        .replace(char_regex, c => substitutions[c] ?? c)
    // Rather than attempting to deal with invalid regexes manually, just let javascript barf if something goes wrong
    // -? is added to the end to allow for prefix hyphens
    try {
        let regex = new RegExp(anchored ? `^(${compiled})-?$` : `(${compiled})-?`, "ui");
        return regex;
    } catch (e) {
        return error`bu sekogeq mí ${query}`;
    }
})
onmessage = e => {
    var q = e.data.q;
    var res = search(q);
    postMessage(res);
}
