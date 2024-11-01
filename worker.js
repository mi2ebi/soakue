importScripts("data/toakue.js");
function search(q) {
    var terms = q.split(" ");
    var res = [];
    terms = terms.map(t => {
        var op = t.match(/^(==|[=~@#/$!-]|[a-z]*:)(.*)/);
        return op ? {"op": (op[1]).replace(/:$/, ""), "orig": op[2], "v": op[2].toLowerCase(), "colon": /:$/.test(op[1])} : {"op": "", "orig": t, "v": t.toLowerCase()};
    });
    for (const entry of dict) {
        var bonus = (entry.user == "official") ? 0.3 : (entry.user == "oldofficial" || /^(old)?(countries|examples)$/.test(entry.user)) ? -0.3 : 0;
        bonus += entry.score / 20;
        var pass = Array(terms.length).fill(false);
        var score = 0;
        for (var i = 0; i < terms.length; i++) {
            const t = terms[i];
            if (t.colon && !["head", "body", "user", "score", "id", "scope"].includes(t.op)) {
                return {"err": "bu jıq mıjóaıchase «<code>" + t.op + "</code>»"};
            }
            if (["#", "id"].includes(t.op)) {
                if (entry.id == t.orig) {
                    pass[i] = true;
                    score = 6;
                    continue;
                }
            }
            if (["=", "head", ""].includes(t.op)) {
                pass[i] = true;
                if (normalize(entry.head) == normalize(t.v)) {
                    score = 5.2;
                } else if (!t.op && compareish(normalizeToneless(t.v), normalizeToneless(entry.head))) {
                    score = 5.1;
                } else if (t.op && compareish(t.v, entry.head)) {
                    score = 5;
                } else {
                    pass[i] = false;
                }
                if (pass[i]) {continue;}
            }
            if (["@", "user"].includes(t.op)) {
                if (entry.user.toLowerCase() == t.v.toLowerCase()) {
                    pass[i] = true;
                    score = 4;
                    continue;
                }
            }
            if (["scope"].includes(t.op)) {
                if (entry.scope.toLowerCase() == t.v.toLowerCase()) {
                    pass[i] = true;
                    score = 4;
                    continue;
                }
            }
            if (["body", ""].includes(t.op)) {
                pass[i] = true;
                const v = normalize(t.v).replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
                if (RegExp(`▯ ?(is|are)?( an?)? ([^ /▯]+/)*${v}`, "iu").test(normalize(entry.body))) {
                    score = 3.2;
                } else if (RegExp(`([^'’]\\b|(?!['’])\\W|^)${v}`, "iu").test(normalize(entry.body))) {
                    score = 3.1;
                } else if (normalize(entry.body).includes(normalize(t.v))) {
                    score = 3;
                } else {
                    pass[i] = false;
                }
                if (pass[i]) {continue;}
            }
            if (!t.op) {
                pass[i] = true;
                if (entry.notes.some(n => normalize(n.content).includes(normalize(t.v)))) {
                    score = 2;
                } else if (normalize(entry.head).startsWith(normalize(t.v))) {
                    score = 1.1;
                } else if (normalizeToneless(entry.head).includes(normalizeToneless(t.v))) {
                    score = 1;
                } else {
                    pass[i] = false;
                }
                if (pass[i]) {continue;}
            }
        }
        if (pass.reduce((a, b) => a && b) && score) res.push([entry, score + bonus]);
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
    return w.normalize("NFD").toLowerCase().replace(/i/g, "ı").replace(/[vwy]/g, "ꝡ");
}
// todo: make a = è match b = è and b = e without any tone (currently only matches b = è)
// todo: make a = naXbıe NOT match b = nạ́bıe (currently it matches regardless of what character X is)
// todo: make a = nabie match b = nạ́bıe
function compareish(a, b) {
    a = normalize(a);
    b = normalize(b);
    for (var i = 0, j = 0; i < (a.length >= b.length ? a : b).length; i++, j++) {
        if (i == a.length && b[j] == "-") {
            continue;
        }
        if (!isTone(a[i]) && isTone(b[j]) && a[i - 1] == b[j - 1]) {
            i--; continue;
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