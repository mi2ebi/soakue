const $ = x => document.getElementById(x);
function mkel(tag, props, children) {
    const element = document.createElement(tag);
    Object.assign(element, props);
    for (const child of children) {
        if (child) {
            element.append(child);
        }
    }
    return element;
}
function htmlify(json) {
    const entry =
    mkel("div", {"className": "entry"}, [
        mkel("dt", {}, [
            mkel("b", {"className": "toa"}, [json.head]),
            " • ",
            mkel("span", {"className": "scope"}, [json.scope]),
            " " + json.user + " ",
            mkel("span", {"className": "score"}, [
                ("" + json.score).replace("-", "−").replace(/^0$/, "±").replace(/^(\d)/, "+$1")
            ]),
            " • " + json.date.slice(0, 10)
        ]),
        mkel("dd", {}, replaceLinks(json.body)),
        mkel("div", {"className": "notes indent"}, json.notes.map(note => [
            mkel("span", {"className": "score"}, [note.user + ": "]),
            mkel("span", {}, replaceLinks(note.content)),
            mkel("span", {"className": "scope"}, [" " + note.date.slice(0, 10)]),
            mkel("br", {}, [])
        ]).flat(Infinity))
    ]);
    return entry;
}
function replaceLinks(str) {
    // ugh why isn't /u a default regex flag
    var bits = str
    .replace(/\*\*/g, "📦")
    .replace(/https:\/\/([a-z0-9./#%?=&_:()'-]+)/giu, "🌐$1🌐")
    .replace(/(?<!🌐[^ ]*)#(?=[a-z0-9_-]{9,}([^a-z0-9_-]|$))|(?<=(?<!🌐[^ ]*)#[a-z0-9_-]{9,})(?=[^a-z0-9_-]|$)/giu, "🆔")
    .split(/(?=[📦🆔🌐])/u);
    for (var i = 0; i < bits.length; i++) {
        if (i == 0) continue;
        if ([...bits[i]][0] === [...bits[i-1]][0] && "📦🆔🌐".includes([...bits[i]][0])) {
            bits[i] = bits[i].replace(/^[📦🆔🌐]/u, "");
            var hrefprefix = bits[i - 1].startsWith("📦") ? "?q=%3D" : bits[i - 1].startsWith("🆔") ? "?q=%23" : "https://";
            var textprefix = bits[i - 1].startsWith("📦") ? ""       : bits[i - 1].startsWith("🆔") ? "#"      : "https://";
            if (i >= 2 && bits[i - 1].startsWith("🌐") && bits[i - 1].endsWith(")") && bits[i - 2].endsWith("(")) {
                bits[i - 1] = bits[i - 1].replace(/\)$/, "");
                bits[i] = ")" + bits[i];
            }
            var href = bits[i - 1].replace(/^[📦🆔🌐]/u, "");
            if (bits[i - 1].startsWith("📦")) {
                href = href.replace(/ /g, "|");
            }
            bits[i - 1] = mkel("a", {
                "href": hrefprefix + (hrefprefix != "https://" ? encodeURIComponent : (x) => x)(href)
            }, [bits[i - 1].replace(/^[📦🆔🌐]/u, textprefix)])
        }
    }
    return bits;
}

function load(res, page) {
    if (!res) return;
    const start = page * 100;
    const end = (page + 1) * 100;
    var nodes = [];
    for (var i = start; i < end; i++) {
        if (res[i]) {
            nodes.push(htmlify(res[i][0]));
        }
    }
    $`res`.append(...nodes);
}