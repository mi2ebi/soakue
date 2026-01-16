var worker = { postMessage() {} };
var page,
  res = [];
window.addEventListener("scroll", function (e) {
  if (window.innerHeight + window.scrollY >= document.body.scrollHeight - 100) {
    page++;
    load(res, page);
    checkLength();
  }
});
function checkLength() {
  if (res && (page + 1) * 100 - 1 >= res.length) {
    $`bottom`.innerHTML = res.length ? "ꝡọfao ka" : "";
  }
}
function clearRes() {
  res = null;
  [`res`, `len`, `bottom`].forEach((x) => ($(x).innerHTML = ""));
}

let URLfromQuery = (q) =>
  window.location.href.split("?")[0] + (q ? "?q=" + encodeURIComponent(q) : "");

function navigate(q, push_state = true, is_search = false) {
  clearRes();
  if (!is_search) $`search`.value = q;
  let newLink = URLfromQuery(q);
  if (push_state) {
    window.history.pushState("", "", newLink);
  } else {
    window.history.replaceState("", "", newLink);
  }
  if (q == "") {
    page = 0;
    return;
  }
  $`bottom`.innerHTML = "chum lao jí pó jóaıse";
  worker.postMessage({ q });
}
let timer;
$`search`.addEventListener("input", function () {
  clearTimeout(timer);
  clearRes();
  $`bottom`.innerHTML = "chum lao jí pó jóaıse";
  timer = setTimeout(() => {
    navigate(this.value.trim(), false, true);
  }, 200);
});
$`clear`.addEventListener("click", function () {
  $`search`.focus();
  navigate("", false);
});
$`english`.addEventListener("click", function () {
  let newQuery = $`search`.value
    .split(" ")
    .filter((t) => !/^([!-]|not:)*(\$|scope:)/.test(t))
    .concat(["$en"])
    .join(" ")
    .trim();
  $`search`.focus();
  navigate(newQuery, false);
});
