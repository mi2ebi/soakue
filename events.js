var worker = {"postMessage": function(a) {}};
var page, q = "", res = [];
window.addEventListener("scroll", function(e) {
    if (window.innerHeight + window.scrollY >= document.body.scrollHeight - 100) {
        page++;
        load(res, page);
        checkLength();
    }
});
function checkLength() {
    if (res && (page + 1) * 100 - 1 >= res.length) {
        $`bottom`.innerHTML = res.length ? "ꝡofào ka" : "";
    }
}
function clearRes() {
    [`res`, `len`, `bottom`].forEach(x => $(x).innerHTML = "");
}
function redirect() {
    var v = "?";
    if (q) v += "&q=" + encodeURIComponent(q);
    v = v.replace(/\?&/g, "?").replace(/[?&]+$/, "");
    window.history.pushState(null, null, window.location.href.split("?")[0] + v);
}
var timer;
$`search`.addEventListener("input", function() {
    clearTimeout(timer);
    q = $`search`.value.trim();
    res = null;
    clearRes();
    redirect();
    $`bottom`.innerHTML = "chum lao jí pó jóaıse"
    timer = setTimeout(function() {
        if (q.length) {
            worker.postMessage({"q": q})
        } else {
            res = null;
            clearRes();
            page = 0;
        }
    }, 100);
});
$`clear`.addEventListener("click", function() {
    $`search`.value = "";
    $`search`.focus();
    dispatchSearch();
});
$`english`.addEventListener("click", function() {
    $`search`.value =
    $`search`.value.split(" ")
    .filter(t => !/^([!-]|not:)*scope:/.test(t))
    .concat(["scope:en"]).join(" ").trim();
    $`search`.focus();
    dispatchSearch();
});
function dispatchSearch() {
    $`search`.dispatchEvent(new Event("input", {"bubbles": true}));
}
