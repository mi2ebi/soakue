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

let makeLink = (query, text = query, props = {}) => mkel("a", {
    onclick(event) {
        event.preventDefault();
        navigate(query)
    },
    href: URLfromQuery(query),
    ...props
}, [text])

function htmlify(json) {
    return mkel("div", {"className": "entry"}, [
        mkel("dt", {}, [
            json.warn ? mkel("span", {}, "⚠\ufe0f ") : null,
            makeLink(json.head, json.head, {className: "toa"}),
            " • ",
            makeLink("scope:" + json.scope, json.scope, {className: "scope"}),
            " ",
            makeLink("@" + json.user, json.user),
            " ",
            mkel("span", {"className": "score"}, [
                ("" + json.score).replace(/^0$/, "±").replace(/^(\d)/, "+$1")
            ]),
            " • ",
            makeLink("#" + json.id, json.date.slice(0, 10)),
            " ",
            mkel("a", {"href": "https://toadua.uakci.space/#" + encodeURIComponent("#" + json.id)}, ["↗"]),
        ]),
        mkel("dd", {}, replaceLinks(json.body)),
        mkel("div", {"className": "notes indent"}, json.notes.flatMap(note => [
            mkel("span", {"className": "score"}, [
                makeLink("@" + note.user, note.user),
                ": "
            ]),
            mkel("span", {}, replaceLinks(note.content)),
            " ",
            mkel("span", {"className": "scope"}, [/^\d/.test(note.date)
                ? note.date.slice(0, 10)
                : new Date(note.date).toISOString().slice(0, 10)]),
            mkel("br", {}, [])
        ]))
    ]);
}
function replaceLinks(str) {
    // ugh why isn't /u a default regex flag
    let parts = str
        .replace(/\*\*/g, "📦")
        .replace(/(https?:\/\/[a-z0-9./#%?=&_:'-]+)/giu, "🌐$1🌐")
        .replace(/(?<!🌐[^ ]*)(#[a-z0-9_-]{9,})(?=[^a-z0-9_-]|$)/giu, "🆔$1🆔")
        .replace(/<((?![/ ])[^>]+(?<! ))>(?!.+<\/\1>)/giu, "📎$1📎")
        .match(/([📦🆔🌐📎])[^📦🆔🌐📎]*?\1|[^📦🆔🌐📎]+/ug);
    return parts.map(part => {
        part = [...part];
        let head = part[0], body = part.slice(1, -1).join("")
        if (!"📦🆔🌐📎".includes(head)) return part.join("")
        if (head === "🌐") {
            return mkel("a", { href: body }, [body.replace(/^https?:\/\//, "")]);
        }
        let search = head === '📦' ? '=' + body.replace(/ /g, '|') : body;
        return makeLink(search, body);
    })
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
