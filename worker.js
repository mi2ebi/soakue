importScripts("data/toakue.js");
function search(q) {
    var terms = q.split(" ");
    var res = [];
    terms = terms.map(t => {
        var op = t.match(/^(==|[=~@#/$!-]|[a-z]*:)(.*)/);
        return op ? {"op": (op[1]).replace(/:$/, ""), "orig": op[2], "v": op[2].toLowerCase(), "colon": /:$/.test(op[1])} : {"op": "", "orig": t, "v": t.toLowerCase()};
    });
    console.log(terms);
    var excl = Array(terms.length);
    for (var i = 0; i < terms.length; i++) {
        excl[i] = ["!", "-", "not"].includes(terms[i].op);
        if (excl[i]) {
            const no = search(terms[i].orig);
            if (no.err) {
                return no;
            }
            excl[i] = no.map(e => {console.log(e[0].id); return e[0].id})
        } else {excl[i] = [];}
    }
    excl = new Set(excl.flat());
    for (const entry of dict) {
        var bonus = (entry.user == "official") ? 0.3 : (entry.user == "oldofficial" || /^(old)?(countries|examples)$/.test(entry.user)) ? -0.3 : 0;
        bonus += entry.score / 20;
        var pass = Array(terms.length).fill(false);
        var score = 0;
        for (var i = 0; i < terms.length; i++) {
            const t = terms[i];
            if (t.colon && !["head", "body", "user", "score", "id", "scope", "arity", "not"].includes(t.op)) {
                return {"err": "bu jıq mıjóaıchase «<code>" + t.op + "</code>»"};
            }
            if (["!", "-", "not"].includes(t.op)) {
                pass[i] = true;
                score = Math.max(score, 0.1);
                continue;
            }
            // 6: id
            if (["#", "id"].includes(t.op)) {
                if (entry.id == t.orig) {
                    pass[i] = true;
                    score = Math.max(score, 6);
                    continue;
                }
            }
            // 5: head
            if (["=", "head", ""].includes(t.op)) {
                pass[i] = true;
                if (normalize(entry.head) == normalize(t.v)) {
                    score = Math.max(score, 5.2);
                } else if (!t.op && compareish(normalizeToneless(t.v), normalizeToneless(entry.head))) {
                    score = Math.max(score, 5.1);
                } else if (t.op && compareish(t.v, entry.head)) {
                    score = Math.max(score, 5);
                } else {
                    pass[i] = false;
                }
                if (pass[i]) {continue;}
            }
            // 4: other
            if (
                ["@", "user"].includes(t.op) && entry.user.toLowerCase() == t.v.toLowerCase()
                || ["scope"].includes(t.op) && entry.scope.toLowerCase() == t.v.toLowerCase()
                || ["/", "arity"].includes(t.op) && t.v == entry.body.split("▯").length - 1
            ) {
                pass[i] = true;
                score = Math.max(score, 4);
                continue;
            }
            // 3: body
            if (["body", ""].includes(t.op)) {
                pass[i] = true;
                const v = normalize(t.v).replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
                if (RegExp(`▯ ?(is|are)?( an?)? ([^ /▯]+/)*${v}`, "iu").test(normalize(entry.body))) {
                    score = Math.max(score, 3.2);
                } else if (RegExp(`([^'’]\\b|(?!['’])\\W|^)${v}`, "iu").test(normalize(entry.body))) {
                    score = Math.max(score, 3.1);
                } else if (normalize(entry.body).includes(normalize(t.v))) {
                    score = Math.max(score, 3);
                } else {
                    pass[i] = false;
                }
                if (pass[i]) {continue;}
            }
            // 1-2: no op
            if (!t.op) {
                pass[i] = true;
                if (entry.notes.some(n => normalize(n.content).includes(normalize(t.v)))) {
                    score = Math.max(score, 2);
                } else if (normalize(entry.head).startsWith(normalize(t.v))) {
                    score = Math.max(score, 1.1);
                } else if (normalizeToneless(entry.head).includes(normalizeToneless(t.v))) {
                    score = Math.max(score, 1);
                } else {
                    pass[i] = false;
                }
                if (pass[i]) {continue;}
            }
        }
        if (pass.reduce((a, b) => a && b) && score && !excl.has(entry.id)) res.push([entry, score + bonus]);
    }
    return res.sort((a, b) => b[1] - a[1]);
}
function isTone(c) {
    return /^[\u0300\u0301\u0308\u0302\u0323]$/.test(c);
}
function normalizeToneless(w) {
    return [...normalize(w)].filter(c => !isTone(c)).join("");
}
function normalize(w) {
    return w.normalize("NFD")
    .toLowerCase()
    .replace(/i/g, "ı")
    .replace(/[vwy]/g, "ꝡ")
    .replace(/[x‘’]/g, "'")
    .replace(/\u0323([\u0301\u0308\u0302])/, "$1\u0323")
    ;
}
// todo: make a = nạbie match b = nạ́bıe
function compareish(a, b) {
    a = normalize(a);
    b = normalize(b);
    for (var i = 0, j = 0; i < (a.length >= b.length ? a : b).length; i++, j++) {
        if (i == a.length && b[j] == "-") {
            continue;
        }
        if (!isTone(a[i]) && isTone(b[j]) && a[i - 1] == b[j - 1]) {
            if (j + 1 < b.length && isTone(b[j + 1])) {
                j++;
            }
            i--; 
            continue;
        }
        if (a[i] != b[j] && isTone(a[i]) == isTone(b[j])) {
            return false;
        }
        if (isTone(a[i]) && !isTone(b[j]) && a[i - 1] == b[j - 1]) {
            return false;
        }
    }
    return true;
}
function sort(a) {
    return a.sort((a, b) => b[1] - a[1]);
}
onmessage = function(e) {
    var q = e.data.q;
    var res = search(q);
    postMessage(res);
}
