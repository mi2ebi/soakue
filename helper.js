const $ = x => document.getElementById(x);

function ipa(head) {
    head = head.toLowerCase().normalize("NFD")
    .replace(/[.,?!]/g, "")
    .replace(/\u0323([\u0301\u0308\u0302])/g, "$1\u0323")
    .replace(/ı/g, "i")
    .replace(/([ptkc])/g, "$1ʰ")
    .replace(/c/g, "ts")
    .replace(/s(ʰ?)h/g, "ɕ$1")
    .replace(/z/g, "dz")
    .replace(/j/g, "dʑ")
    .replace(/nh/g, "ɲ")
    .replace(/q/g, "ŋ")
    .replace(/e/g, "ɛ")
    .replace(/'|(^| )([aeiou])/g, "$1ʔ$2")
    .replace(/a([\u0301\u0308\u0302]?\u0323?)o/, "a$1w")
    .replace(/([aɛo][\u0301\u0308\u0302]?\u0323?)i/, "$1j")
    .replace(/([ɛij][\u0301\u0308\u0302]?\u0323? ?)ꝡ/g, "$1w")
    .replace(/([ouw][\u0301\u0308\u0302]?\u0323? ?)ꝡ/g, "$1j")
    .replace(/ɛ([aou])/g, "e$1")
    .replace(/i([\u0301\u0308\u0302]?\u0323?)ŋ/g, "ɪ$1ŋ")
    .replace(/o([\u0301\u0308\u0302]?\u0323?)ŋ/g, "ɔ$1ŋ")
    .replace(/u([\u0301\u0308\u0302]?\u0323?)ŋ/g, "ʊ$1ŋ")
    ;
    return "/" + head + "/";
}

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
            json.warn ? mkel("span", {}, "⚠\ufe0f ") : null,
            mkel("a", {
                "className": "toa",
                "href": "?q=" + encodeURIComponent(json.head)
            }, [json.head]),
            " ",
            !json.warn ? mkel("span", {"className": "scope"}, [ipa(json.head)]) : null,
            " • ",
            mkel("a", {
                "className": "scope",
                "href": "?q=" + encodeURIComponent("scope:" + json.scope)
            }, [json.scope]),
            " ",
            mkel("a", {"href": "?q=" + encodeURIComponent("@" + json.user)}, [json.user]),
            " ",
            mkel("span", {"className": "score"}, [
                ("" + json.score).replace(/^0$/, "±").replace(/^(\d)/, "+$1")
            ]),
            " • ",
            mkel("a", {"href": "?q=" + encodeURIComponent("#" + json.id)}, [json.date.slice(0, 10)]),
            " ",
            mkel("a", {"href": "https://toadua.uakci.space/#" + encodeURIComponent("#" + json.id)}, ["↗"]),
        ]),
        mkel("dd", {}, replaceLinks(json.body)),
        mkel("div", {"className": "notes indent"}, json.notes.map(note => [
            mkel("span", {"className": "score"}, [
                mkel("a", {"href": "?q=" + encodeURIComponent("@" + note.user)}, [note.user]),
                ": "
            ]),
            mkel("span", {}, replaceLinks(note.content)),
            " ",
            mkel("span", {"className": "scope"}, [/^\d/.test(note.date) ? note.date.slice(0, 10) : new Date(note.date).toISOString().slice(0, 10)]),
            mkel("br", {}, [])
        ]).flat(Infinity))
    ]);
    return entry;
}
function replaceLinks(str) {
    // ugh why isn't /u a default regex flag
    var bits = str
    .replace(/\*\*/g, "📦")
    .replace(/https?:\/\/([a-z0-9./#%?=&_:()'-]+)/giu, "🌐$1🌐")
    .replace(/(?<!🌐[^ ]*)#([a-z0-9_-]{9,})(?=[^a-z0-9_-]|$)/giu, "🆔$1🆔")
    .replace(/<((?![/ ])[^>]+(?<! ))>(?!.+<\/\1>)/giu, "📎$1📎")
    .split(/(?=[📦🆔🌐📎])/u);
    for (var i = 0; i < bits.length; i++) {
        if (i == 0) continue;
        if ([...bits[i]][0] === [...bits[i - 1]][0] && "📦🆔🌐📎".includes([...bits[i]][0])) {
            bits[i] = bits[i].replace(/^[📦🆔🌐📎]/u, "");
            var hrefprefix = bits[i - 1].startsWith("📦") ? "?q=%3D" : bits[i - 1].startsWith("🆔") ? "?q=%23" : bits[i - 1].startsWith("📎") ? "?q=" : "https://";
            var textprefix = bits[i - 1].startsWith("📦") || bits[i - 1].startsWith("📎") ? "" : bits[i - 1].startsWith("🆔") ? "#" : "https://";
            if (i >= 2 && bits[i - 1].startsWith("🌐") && bits[i - 1].endsWith(")") && bits[i - 2].endsWith("(")) {
                bits[i - 1] = bits[i - 1].replace(/\)$/, "");
                bits[i] = ")" + bits[i];
            }
            var href = bits[i - 1].replace(/^[📦🆔🌐📎]/u, "");
            if (bits[i - 1].startsWith("📦")) {
                href = href.replace(/ /g, "|");
            }
            bits[i - 1] = mkel("a", {
                "href": hrefprefix + (hrefprefix != "https://" ? encodeURIComponent : (x) => x)(href)
            }, [bits[i - 1].replace(/^[📦🆔🌐📎]/u, textprefix)])
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