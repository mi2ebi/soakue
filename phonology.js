
const phonology = (() => {
    const SINGLE_INITIALS = [..."mpbfntdczsrljꝡkg'h"];
    const DIGRAPH_INITIALS = ["ch", "sh", "nh"];
    const INITIALS = [...SINGLE_INITIALS, ...DIGRAPH_INITIALS];
    const VOWELS = "aeıou";
    const DIPHTHONGS = ["aı", "eı", "oı", "ao"];
    const FINALS = "mq";
    
    const GRAVE = "\u0300";
    const UNDERDOT = "\u0323";
    const TONE_MARKS = { [GRAVE]: "fall", "\u0301": "rise", "\u0308": "glottal", "\u0302": "hiatus" };
    const TONE_CHARS = Object.keys(TONE_MARKS).join("");
    
    const tones = TONE_CHARS;
    const char_match = `(?:.[${tones}]?${UNDERDOT}?)`;
    const vowel_match = `(?:[${VOWELS}][${tones}]?${UNDERDOT}?)`;
    const init_consonants = `(?:[${SINGLE_INITIALS.join("")}]|${DIGRAPH_INITIALS.join("|")})`;
    // q is the one final that isn't also an initial
    const letter = `(?:${vowel_match}|${init_consonants}|q)`;
    const finals = `[${FINALS}]`;
    const diphthongs = `(?:${DIPHTHONGS.join("|")})`;
    const raku = `(?:(?:(?<= |^)|${init_consonants})${vowel_match}?(?:${diphthongs}|${vowel_match}${finals}?))`;
    
    const substitutions = {
        X: letter,
        C: init_consonants,
        V: vowel_match,
        F: diphthongs,
        Q: finals,
        R: raku,
    };
    // If a tone is present in the query, it's required in the word; if not present any tone(s) are allowed.
    // Underdots are dealt with separately, so query nạbie matches word nạ́bıe
    for (const vowel of VOWELS) {
        substitutions[vowel] = `${vowel}[${tones}]?${UNDERDOT}?`;
        substitutions[vowel + UNDERDOT] = `${vowel}[${tones}]?${UNDERDOT}'?`;
        for (const tone of tones) {
            substitutions[vowel + tone] = `${vowel}${tone}${UNDERDOT}?`;
            substitutions[vowel + tone + UNDERDOT] = `${vowel}${tone}${UNDERDOT}'?`;
        }
    }
    
    const word_diacritic_regex = new RegExp(`(${letter}+)([1234])`, "iug");
    const vowel_regex = new RegExp(vowel_match, "iu");
    const underdot_regex = new RegExp(`(${raku})([:-])`, "iug");
    // NFD puts the underdot before the tone; the patterns above want it after
    const underdot_first_regex = new RegExp(`${UNDERDOT}([${tones}])`, "g");
    const isDiacritic = (c) => c === UNDERDOT || tones.includes(c);
    
    // attach a cache to a function, so that it doesn't recalculate the same values
    const memoize = (fn) => {
        const cache = new Map();
        return (...args) => {
            const hash = args.join("\x00");
            if (cache.has(hash)) return cache.get(hash);
            const res = fn(...args);
            cache.set(hash, res);
            return res;
        };
    };
    
    // for regex search purposes, we don't want to convert to lowercase since C/F/Q/R/V exist
    const normalize = memoize((w, lowercase = true) =>
        (lowercase ? w.toLowerCase() : w)
            .normalize("NFD")
            .replace(/i/g, "ı")
            .replace(/[vwy]/g, "ꝡ")
            .replace(/[x‘’`]/g, "'")
            .replace(underdot_first_regex, `$1${UNDERDOT}`),
    );
    
    const normalizeToneless = memoize((w) =>
        [...normalize(w)].filter((c) => !isDiacritic(c)).join(""),
    );
    
    // queries also have underdot and number replacements, which can be dealt with separately (and are somewhat
    // expensive)
    const normalize_query = memoize((w, lowercase = true) =>
        normalize(w, lowercase)
            .replace(word_diacritic_regex, (_, word, number) =>
                word.replace(vowel_regex, (c) => c + TONE_CHARS[number - 1]),
            )
            .replace(underdot_regex, (_, word) =>
                word.replace(vowel_regex, (c) => c + UNDERDOT),
            ),
    );
    
    // handle prefix hyphens
    const compareish = (query, word) =>
          query == word ||
          query == word.replace(/-$/, "") ||
          (word.endsWith("-") && query.replace(/\u0323$/, "") == word.replace(/-$/, ""));
    
    const char_regex = new RegExp(char_match, "iug");
    const char_brackets_regex = new RegExp(`\\[${char_match}*?\\]`, "iug");
    
    // a toaq-aware regex from a query, or null if it isn't one
    const queryToRegex = memoize((query, anchored = true) => {
        if (query.includes("[^")) return null;
        // due to [...] not being true character classes, we can't directly substitute them
        // and instead have to turn [abc] into (a|b|c)
        const compiled = query
              .replace(
                  char_brackets_regex,
                  (c) => `(?:${c.slice(1, -1).match(char_regex)?.join("|") ?? ""})`,
              )
              .replace(char_regex, (c) => substitutions[c] ?? c);
        // Rather than attempting to deal with invalid regexes manually,
        // just let javascript barf if something goes wrong.
        // -? is added to the end to allow for prefix hyphens
        try {
            return new RegExp(anchored ? `^(?:${compiled})-?$` : `(?:${compiled})-?`, "ui");
        } catch {
            return null;
        }
    });
    
    return {
        SINGLE_INITIALS,
        DIGRAPH_INITIALS,
        INITIALS,
        VOWELS,
        DIPHTHONGS,
        FINALS,
        GRAVE,
        UNDERDOT,
        TONE_MARKS,
        TONE_CHARS,
        normalize,
        normalize_query,
        normalizeToneless,
        compareish,
        queryToRegex,
    };
})();
