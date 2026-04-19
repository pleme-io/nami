;; Starter tatara-lisp scrapes for nami.
;;
;; Copy to `$XDG_CONFIG_HOME/nami/scrapes.lisp` or pass with
;; `nami scrape <url> -c <path>`. Each form is one `(defscrape …)`.
;;
;; Extract kinds:
;;
;;   text   — concatenated text content of the match
;;   attr   — the value of a single named attribute (:attr "href")
;;   tag    — the element's tag name
;;   attrs  — every attribute as (key . value) pairs
;;
;; Selectors: tag / .class / #id / compound / descendant (space) /
;; child (>) / universal (*). Same grammar as (defdom-transform :selector …).

;; ── page skeleton ────────────────────────────────────────────────
(defscrape :name "title"
           :selector "title"
           :extract text
           :description "page <title>")

(defscrape :name "h1"
           :selector "h1"
           :extract text)

;; ── navigation + content structure ──────────────────────────────
(defscrape :name "all-headings"
           :selector "h1, h2, h3"          ; note: selector lists not V2;
           :extract text                   ; this specific form will skip
           :description "placeholder — selector lists graduate in V3")

(defscrape :name "article-h2"
           :selector "article h2"
           :extract text
           :description "headings inside <article>, with descendant combinator")

;; ── hyperlinks + images ────────────────────────────────────────
(defscrape :name "links"
           :selector "a"
           :extract attr
           :attr "href")

(defscrape :name "images"
           :selector "img"
           :extract attrs
           :description "every <img> with all its attributes")

;; ── htmx detection (attribute selectors are V3; tag-only for now) ──
(defscrape :name "any-button"
           :selector "button"
           :extract attrs
           :description "returns hx-* attrs automatically when present")
