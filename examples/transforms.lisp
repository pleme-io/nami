;; Starter tatara-lisp transforms for nami.
;;
;; Copy to `$XDG_CONFIG_HOME/nami/transforms.lisp` (typically
;; `~/.config/nami/transforms.lisp`) and nami will apply them to every
;; page post-parse, pre-layout. Each transform is one
;; `(defdom-transform …)` form.
;;
;; V2 selectors supported:
;;
;;   tag        "div"             (case-insensitive)
;;   class      ".ad"
;;   id         "#hero"
;;   universal  "*"
;;   compound   "div.card#hero"
;;   descendant "article p"        (space)
;;   child      "ul > li"          (greater-than)
;;
;; Actions:
;;
;;   remove        — delete matches from the tree
;;   unwrap        — replace matches with their children
;;   add-class     — :arg = class name
;;   remove-class  — :arg = class name
;;   set-attr      — :arg = "name=value"
;;   remove-attr   — :arg = attribute name
;;   set-text      — :arg = new inner text

;; ── sanity: strip inert JS noise (nami has no JS engine yet; the WASM/ ──
;; ── WASI sandbox arc is on the roadmap).                              ──
(defdom-transform :name "strip-script"
                  :selector "script"
                  :action remove
                  :description "no-JS browser: script tags are inert noise")

(defdom-transform :name "surface-noscript"
                  :selector "noscript"
                  :action unwrap
                  :description "reveal content the site hides behind JS")

;; ── ads / trackers by obvious class ─────────────────────────────────
(defdom-transform :name "hide-ads"
                  :selector ".ad"
                  :action remove)

(defdom-transform :name "hide-sponsored"
                  :selector ".sponsored"
                  :action remove)

;; ── combinator-aware: iframes that happen to sit inside an ad div, ──
;; ── whose class we wouldn't otherwise target.                       ──
(defdom-transform :name "strip-iframes-in-ads"
                  :selector ".ad > iframe"
                  :action remove)

;; ── accessibility scoring ───────────────────────────────────────────
;; Tag images sitting inside a <figure> as editorial (different from
;; inline images). Descendant combinator picks up <img> at any depth
;; under <figure>.
(defdom-transform :name "flag-editorial-images"
                  :selector "figure img"
                  :action add-class
                  :arg "nami-editorial")

;; Universal-selector fallback for everything else.
(defdom-transform :name "flag-all-images"
                  :selector "img"
                  :action add-class
                  :arg "nami-needs-alt-review")

;; ── reader-mode hint: paragraphs that are immediate children of ──
;; ── <article> get a class so downstream styling can pick them up.──
(defdom-transform :name "reader-article-paragraphs"
                  :selector "article > p"
                  :action add-class
                  :arg "reader-p")
