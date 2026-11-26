importScripts("phonology.js", "data/toakue.js");
const { normalize, normalize_query, normalizeToneless, compareish, queryToRegex } = phonology;

const escapeHTML = (s) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
const bad = (query, why) => ({ err: `<code>${escapeHTML(query)}</code> ${why}` });

const SCORE = {
    id: 6,
    headExact: 5.2,
    headToneless: 5.1,
    headRegex: 5,
    glossExact: 4.1,
    gloss: 4,
    bodySlot: 3.2, // the query starts a ▯ slot's definition
    bodyWord: 3.1, // the query starts a word in the body
    bodyPart: 3, // the query is somewhere in the body
    bodyMeta: 2.5, // ...but only in the "type: 'gloss';" part at the front
    note: 2,
    headPrefix: 1.1,
    headPart: 1,
    filter: 0.1, // yes/no operators (@, $, score:, ...): they don't rank anything
};

// orderings. each sorts [entry, score] pairs; random is special-cased in `search`
const dict_index = new Map(dict.map((entry, i) => [entry, i]));
const timestamps = new Map(dict.map((entry) => [entry, Date.parse(entry.date)]));
const ORDERS = {
    default: (a, b) => b[1] - a[1],
    random: null,
    alpha: (a, b) => dict_index.get(a[0]) - dict_index.get(b[0]),
    best: (a, b) => b[0].score - a[0].score,
    worst: (a, b) => a[0].score - b[0].score,
    new: (a, b) => timestamps.get(b[0]) - timestamps.get(a[0]),
    old: (a, b) => timestamps.get(a[0]) - timestamps.get(b[0]),
};

function shuffle(a) {
    for (let i = a.length - 1; i > 0; i--) {
        const j = Math.floor(Math.random() * (i + 1));
        [a[i], a[j]] = [a[j], a[i]];
    }
    return a;
}

// matchers: (entry, term) => score, or 0 for no match
const first = (...matchers) => (entry, term) => {
    for (const matcher of matchers) {
        const score = matcher(entry, term);
        if (score) return score;
    }
    return 0;
};

// the entry has this field, and either the term is empty (meaning "has any") or compare() agrees
const withField = (get, compare) => (entry, term) => {
    const field = get(entry);
    return field && (!term.value || compare(field, term)) ? SCORE.filter : 0;
};

const headExact = (entry, t) => {
    const head = normalize(entry.head);
    return t.value == entry.head || compareish(t.norm, head) || compareish(t.nq, head)
        ? SCORE.headExact
        : 0;
};
const headToneless = (entry, t) =>
      compareish(t.toneless, normalizeToneless(entry.head)) ? SCORE.headToneless : 0;
const headRegex = (entry, t) =>
      t.regex.test(normalize(entry.head)) ? SCORE.headRegex : 0;
const headPart = (entry, t) =>
      normalizeToneless(entry.head).includes(t.toneless) ? SCORE.headPart : 0;
const headPrefix = (entry, t) =>
      normalize(entry.head).startsWith(t.nq) ? SCORE.headPrefix : 0;

const gloss = (entry, t) => {
    if (!entry.gloss) return 0;
    const g = entry.gloss.toLowerCase();
    return g == t.value ? SCORE.glossExact : g.includes(t.value) ? SCORE.gloss : 0;
};

const body = (entry, t) => {
    const text = normalize(entry.body);
    if (t.bodySlot.test(text)) return SCORE.bodySlot;
    if (t.bodyWord.test(text)) return SCORE.bodyWord;
    if (text.includes(t.nq)) return t.bodyMeta.test(text) ? SCORE.bodyMeta : SCORE.bodyPart;
    return 0;
};

const notes = (entry, t) => entry.notes.some((n) => normalize(n.content).includes(t.nq)) ? SCORE.note : 0;

// number of ▯ in each clause of the body (ignoring clauses without any, unless none have any)
function arities(entry) {
    const counts = entry.body.split(/[;.?!；]/).map((clause) => clause.split("▯").length - 1);
    return counts.every((n) => n == 0) ? counts : counts.filter((n) => n != 0);
}

// extra fields on a term, worked out once per search instead of once per entry
const textFields = (query) => {
    const value = query.toLowerCase();
    return {
        norm: normalize(value),
        nq: normalize_query(value),
        toneless: normalizeToneless(value),
    };
};
const bodyFields = (nq) => {
    const v = nq.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    return {
        bodySlot: RegExp(`▯ ?(is|are)?( an?)? ([^ /▯]+/)*${v}`, "iu"),
        bodyWord: RegExp(`([^'’]\\b|(?!['’])\\W|^)${v}`, "iu"),
        bodyMeta: RegExp(`^[^▯:]*${v}[^▯]*:`, "iu"),
    };
};
const regexFields = (query, anchored) => {
    if (!queryToRegex(query)) return bad(query, "is not a regex");
    const regex = queryToRegex(normalize_query(query, false), anchored);
    if (!regex) return bad(query, "is not a regex");
    return { ...textFields(query), regex };
};

const unsignedInt = (query) => /^[0-9]*$/.test(query) ? undefined : bad(query, "is not an unsigned integer");

// ">=3" → [">=", "3"]; a bare "3" means "=3"
const splitComparison = (query) => {
    const m = query.match(/^(>=?|<=?|=)?(.+)$/);
    return m && [m[1] ?? "=", m[2]];
};
const compare = {
    "=": (a, b) => a === b,
    ">": (a, b) => a > b,
    ">=": (a, b) => a >= b,
    "<": (a, b) => a < b,
    "<=": (a, b) => a <= b,
};

// "2025" and "2025-03" stand for a whole year/month: the start of it for >= and <, the end for > and <=
function dateBound(value, comparison) {
    const late = comparison == ">" || comparison == "<=";
    if (value.length == 4) return value + (late ? "-12-31" : "-01-01");
    if (value.length == 7) {
        const [year, month] = value.split("-").map(Number);
        const last = String(new Date(year, month, 0).getDate()).padStart(2, "0");
        return value + (late ? `-${last}` : "-01");
    }
    return value;
}

const stripAcute = (s) => s.normalize("NFD").replace(/\u0301/g, "");

// THE OPERATORS
// one object per operator, so adding one means adding one object (and a row in the help table).
//   name     what terms call it; the scoring code never sees the aliases
//   tokens   what people type: a symbol like "@", or a word with its colon like "user:"
//   parse    (query) => { err } or extra fields to put on the term. optional.
//            terms always get `orig` (the query as typed) and `value` (lowercased)
//   match    (entry, term) => score, 0 meaning no match. every term has to match
//   modifier changes the results rather than filtering them (order:, count:)
const BARE = {
    name: "",
    parse: (query) => {
        const fields = textFields(query);
        return { ...fields, ...bodyFields(fields.nq) };
    },
    match: first(headExact, headToneless, gloss, body, headPart, notes, headPrefix),
};

const OPERATORS = [
    {
        name: "head",
        tokens: ["=", "head:"],
        parse: (query) => regexFields(query, true),
        match: first(headExact, headRegex),
    },
    {
        name: "within",
        tokens: ["~"],
        parse: (query) => regexFields(query, false),
        match: first(headExact, headRegex),
    },
    {
        name: "body",
        tokens: ["body:"],
        parse: BARE.parse,
        match: body,
    },
    {
        name: "gloss",
        tokens: ["gloss:"],
        match: gloss,
    },
    {
        name: "note",
        tokens: ["note:"],
        parse: (query) => ({ nq: normalize_query(query.toLowerCase()) }),
        match: notes,
    },
    {
        name: "noteby",
        tokens: ["noteby:"],
        match: (entry, t) =>
        (t.value
         ? entry.notes.some((n) => n.user.toLowerCase() == t.value)
         : entry.notes.length > 0)
            ? SCORE.filter
            : 0,
    },
    {
        name: "id",
        tokens: ["#", "id:"],
        match: (entry, t) => (entry.id == t.orig ? SCORE.id : 0),
    },
    {
        name: "user",
        tokens: ["@", "user:"],
        match: (entry, t) => (entry.user.toLowerCase() == t.value ? SCORE.filter : 0),
    },
    {
        name: "scope",
        tokens: ["$", "scope:"],
        match: (entry, t) => (entry.scope.toLowerCase() == t.value ? SCORE.filter : 0),
    },
    {
        name: "arity",
        tokens: ["/", "arity:"],
        parse: unsignedInt,
        match: (entry, t) => (arities(entry).includes(+t.value) ? SCORE.filter : 0),
    },
    {
        name: "tags",
        tokens: ["%", "tags:"],
        match: withField(
            (entry) => entry.tags,
            (tags, t) => tags.split(" ").includes(t.value),
        ),
    },
    {
        name: "score",
        tokens: ["score:"],
        parse: (query) => {
            const split = splitComparison(query);
            if (!split) return bad(query, "is nothing");
            const [comparison, value] = split;
            if (isNaN(value)) return bad(value, "is not a number");
            return { comparison, value: value.toLowerCase() };
        },
        match: (entry, t) => (compare[t.comparison](entry.score, parseFloat(t.value)) ? SCORE.filter : 0),
    },
    {
        name: "date",
        tokens: ["date:"],
        parse: (query) => {
            const split = splitComparison(query);
            if (!split) return bad(query, "is nothing");
            const [comparison, value] = split;
            if (!/^\d{4}(-\d{2}(-\d{2})?)?$/.test(value))
                return bad(value, "is not a valid iso-8601 date");
            return { comparison, value: value.toLowerCase(), bound: dateBound(value.toLowerCase(), comparison) };
        },
        match: (entry, t) => {
            const date = entry.date.slice(0, 10);
            if (t.comparison == "=" && date.startsWith(t.value)) return SCORE.filter;
            return compare[t.comparison](date, t.bound) ? SCORE.filter : 0;
        },
    },
    {
        name: "warn",
        tokens: ["w:", "warn:"],
        parse: (query) => (query ? bad("w", "does not accept arguments, it's a flag") : undefined),
        match: (entry) => (entry.warn ? SCORE.filter : 0),
    },
    {
        name: "frame",
        tokens: ["frame:"],
        parse: (query) =>
            /^(0|1x?|2(xx)?|c(0|1[ix]?|2([ix][ix])?|c(0|1[ijx]?|2([ijx][ijx])?|c)?)?)?$/.test(query)
            ? undefined
            : bad(query.replace(/(?=[ce\d])/g, " "), "isn't a valid frame"),
        match: withField(
            (entry) => entry.frame,
            (frame, t) =>
            t.value == frame.replace(/ /g, "") || t.value == frame.replace(/ |[ijx]+$/g, ""),
        ),
    },
    {
        name: "pron",
        tokens: ["pron:"],
        parse: (query) =>
            /^(ho\u0301?q?|ta\u0301?|ma\u0301?q|particle|phrase)?$/.test(query.normalize("NFD"))
            ? undefined
            : bad(query, "isn't a valid pronominal class"),
        match: withField(
            (entry) => entry.pronoun,
            (pronoun, t) => stripAcute(t.value) == stripAcute(pronoun),
        ),
    },
    {
        name: "dist",
        tokens: ["dist:"],
        parse: (query) =>
            /^[nd]{0,3}$/.test(query) ? undefined : bad(query, "isn't a valid distribution"),
        match: withField(
            (entry) => entry.distribution,
            (dist, t) => t.value == dist.replace(/ /g, ""),
        ),
    },
    {
        name: "subj",
        tokens: ["subj:"],
        parse: (query) =>
            /^(s[FIPEAS]|[fipeasFIPEAS])$/.test(query) ? undefined : bad(query, "isn't a valid subject type"),
        match: withField(
            (entry) => entry.subject,
            (subject, t) => "s" + t.value.toUpperCase() == subject || t.orig == subject,
        ),
    },
    {
        name: "type",
        tokens: ["type:"],
        match: withField(
            (entry) => entry.type,
            (type, t) => type.toLowerCase().replace(/ /g, "").includes(t.value.replace(/ /g, "")),
        ),
    },
    {
        name: "order",
        tokens: ["order:", "ord:"],
        modifier: true,
        parse: (query) =>
        Object.hasOwn(ORDERS, query)
            ? undefined
            : bad(
                query,
                "is not an ordering. available orderings: "
                    + Object.keys(ORDERS)
                    .map((name) => `<code>${name}</code>`)
                    .join(", ")
            ),
    },
    {
        name: "count",
        tokens: ["count:"],
        modifier: true,
        parse: (query) => {
            const err = unsignedInt(query);
            if (err) return err;
            if (+query > dict.length) return { err: `there are only ${dict.length} entries!` };
        },
    },
];

const OPERATORS_BY_TOKEN = new Map(
    OPERATORS.flatMap((def) => def.tokens.map((token) => [token, def])),
);

const fail = (err) => {
    throw err;
};

// a term is { def, op, orig, value, ...whatever def.parse adds }
// `literal` terms (the ones that started with a quote) are always plain text, never operators
function parseTerm(token, literal = false) {
    const [, opToken, rest] = (!literal && token.match(/^([=~@#/$%]|[a-z]*:)(.*)/)) || [];
    // the word forms of ! and |, for old links
    if (opToken == "not:") return { kind: "not", kid: parseQuery(rest) };
    if (opToken == "or:") return { kind: "or", kids: rest.split(" ").map(parseQuery) };
    const def = opToken ? OPERATORS_BY_TOKEN.get(opToken) : BARE;
    if (!def) fail(bad(opToken.replace(/:$/, ""), "is not an operator"));
    const query = opToken ? rest : token;
    const extra = def.parse?.(query);
    if (extra?.err) fail(extra);
    return { kind: "term", term: { def, op: def.name, orig: query, value: query.toLowerCase(), ...extra } };
}

// QUERIES
// a query is parsed into a tree of
//   { kind: "term", term }   one operator and its argument (see OPERATORS)
//   { kind: "and", kids }    everything has to match (this is what spaces mean)
//   { kind: "or", kids }     any of them: a | b
//   { kind: "not", kid }     none of it: !a
// grammar, loosest binding first:
//   or    := and ("|" and)*
//   and   := unary*
//   unary := "!" unary | "(" or ")" | term
// quotes group characters, so note:"see also" works, and a term that *starts* with a quote
// is plain text: "/2" looks for the text /2 instead of being the arity operator

// text → [{ type: "(" | ")" | "!" | "|" } | { type: "term", text, literal }]
// ( ! | only mean something at the start of a term, and ) only at the end of one while a group
// is open, so regexes and bodies with parens in them e.g. =a(b, body:(x) mostly just work
function lex(q) {
    const items = [];
    let current = "", started = false, literal = false, quote = false;
    let open = 0; // unclosed ( inside the current term
    let depth = 0; // groups we're inside
    const flush = () => {
        if (current) items.push({ type: "term", text: current, literal });
        current = "";
        started = literal = false;
        open = 0;
    };
    const chars = [...q];
    chars.forEach((c, i) => {
        const next = chars[i + 1];
        const atStart = !started && !quote;
        if (c == '"') {
            if (atStart) literal = true;
            started = true;
            quote = !quote;
        } else if (quote) current += c;
        else if (c == " ") flush();
        else if (atStart && c == "(") {
            items.push({ type: "(" });
            depth++;
        } else if (atStart && (c == "!" || c == "|")) items.push({ type: c });
        else if (c == ")" && depth > 0 && open == 0 && (next === undefined || next == " " || next == ")")) {
            flush();
            items.push({ type: ")" });
            depth--;
        } else {
            if (c == "(") open++;
            else if (c == ")" && open > 0) open--;
            current += c;
            started = true;
        }
    });
    if (quote) fail({ err: `unclosed quote` });
    flush();
    return items;
}

const isEmpty = (node) => node.kind == "and" && !node.kids.length;

function parseQuery(q) {
    const items = lex(q);
    let pos = 0;
    const peek = () => items[pos]?.type;
    
    function parseOr() {
        const alts = [parseAnd()];
        while (peek() == "|") {
            pos++;
            alts.push(parseAnd());
        }
        if (alts.length > 1 && alts.some(isEmpty))
            fail({ err: `<code>|</code> needs a query on both sides` });
        return alts.length == 1 ? alts[0] : { kind: "or", kids: alts };
    }
    function parseAnd() {
        const kids = [];
        while (peek() && peek() != "|" && peek() != ")") kids.push(parseUnary());
        return kids.length == 1 ? kids[0] : { kind: "and", kids };
    }
    function parseUnary() {
        const item = items[pos++];
        if (item.type == "(") {
            const inner = parseOr();
            if (peek() != ")") fail({ err: `unclosed <code>(</code>` });
            pos++;
            if (isEmpty(inner)) fail({ err: `empty <code>()</code>` });
            return inner;
        }
        if (item.type == "!") {
            if (!peek() || peek() == "|" || peek() == ")")
                fail({ err: `<code>!</code> needs something after it` });
            return { kind: "not", kid: parseUnary() };
        }
        return parseTerm(item.text, item.literal);
    }
    
    const root = parseOr();
    if (pos < items.length) fail({ err: `unexpected <code>)</code>` });
    return root;
}

// order: and count: change the results instead of filtering them, so they only make sense
// on the outside of a query. this takes them off the top level of the tree.
const isModifier = (node) => node.kind == "term" && node.term.def.modifier;
const childrenOf = (node) => node.kids ?? (node.kid ? [node.kid] : []);
const hasModifier = (node) => isModifier(node) || childrenOf(node).some(hasModifier);

function splitModifiers(root) {
    const top = root.kind == "and" ? root.kids : [root];
    const modifiers = top.filter(isModifier).map((node) => node.term);
    const rest = top.filter((node) => !isModifier(node));
    if (rest.some(hasModifier))
        fail({
            err: `<code>order:</code> and <code>count:</code> have to go outside any <code>()</code>, <code>|</code> or <code>!</code>`,
        });
    // `order:alpha` on its own means "everything, in that order"
    if (modifiers.length && !rest.length) rest.push(parseTerm("~"));
    return { filter: { kind: "and", kids: rest }, modifiers };
}

// the score of an entry against a tree, or 0 for no match.
// the empty query matches everything, with a score of -Infinity (which sorts as "don't care")
function evaluate(node, entry) {
    switch (node.kind) {
    case "term":
        return node.term.def.match(entry, node.term);
    case "and": {
        let best = -Infinity;
        for (const kid of node.kids) {
            const score = evaluate(kid, entry);
            if (!score) return 0;
            best = Math.max(best, score);
        }
        return best;
    }
    case "or":
        return Math.max(...node.kids.map((kid) => evaluate(kid, entry)));
    case "not":
        return evaluate(node.kid, entry) ? 0 : SCORE.filter;
    }
}

// official entries first, old ones and the example/country dumps last
function bonus(entry) {
    const user =
          entry.user == "official"
          ? 0.3
          : entry.user == "oldofficial" || /^(old)?(countries|examples)$/.test(entry.user)
          ? -0.3
          : 0;
    return user + entry.score / 20;
}

function search(q) {
    try {
        const { filter, modifiers } = splitModifiers(parseQuery(q));
        const named = (name) => modifiers.filter((m) => m.op == name);
        if (named("order").length > 1) fail({ err: `it is not possible to have multiple orders` });
        if (named("count").length > 1) fail({ err: `it is not possible to have multiple counts` });
        
        let res = [];
        for (const entry of dict) {
            const score = evaluate(filter, entry);
            if (score) res.push([entry, score + bonus(entry)]);
        }
        const order = named("order")[0]?.value ?? "default";
        res = order == "random" ? shuffle(res) : res.sort(ORDERS[order]);
        const limit = named("count")[0];
        return limit ? res.slice(0, +limit.value) : res;
    } catch (e) {
        if (typeof e?.err == "string") return e;
        throw e;
    }
}

onmessage = (e) => {
    postMessage(search(e.data.q));
};
