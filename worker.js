importScripts("data/toakue.js");
function search(q) {
    const terms = q.split(" ");
    // var res = [];
    var res = dict.map(e => [e, 1]);
    return res;
}
function sort(a) {
    return a.sort((a, b) => b[1] - a[1]);
}
onmessage = function(e) {
    var q = e.data.q;
    var res = search(q);
    postMessage(res);
}