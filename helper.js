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
            mkel("a", {
                "className": "toa",
                "href": "?q=" + encodeURIComponent(json.head)
            }, [json.head]),
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
            mkel("span", {}, json.warn ? "⚠\ufe0f " : ""),
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