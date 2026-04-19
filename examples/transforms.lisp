;; Starter tatara-lisp transforms for nami.
;;
;; Copy to ~/.config/nami/transforms.lisp (or set `transforms_file`
;; in nami.yaml) and nami will apply them to every page post-parse,
;; pre-layout. Each transform is one `(defdom-transform …)` form.
;;
;; Selectors are simple for V1: tag name ("a"), class (".ad"),
;; or id ("#main"). Actions:
;;
;;   remove        — delete matches from the tree
;;   unwrap        — replace matches with their children (strip wrapper)
;;   add-class     — :arg = class name
;;   remove-class  — :arg = class name
;;   set-attr      — :arg = "name=value"
;;   remove-attr   — :arg = attribute name
;;   set-text      — :arg = new inner text

;; ── sanity: strip script and style tags we can't run. ─────────────
;; nami has no JS engine (yet — wasm/wasi sandboxing is the roadmap),
;; so <script> content only adds parse noise. Same for the heavy
;; inline <style> blocks we do a cascade on anyway.
(defdom-transform :name "strip-script"
                  :selector "script"
                  :action remove
                  :description "no-JS browser: script tags are inert noise")

(defdom-transform :name "strip-noscript"
                  :selector "noscript"
                  :action unwrap
                  :description "surface <noscript> content that's hidden behind JS")

;; ── ads / trackers by obvious class ──────────────────────────────
(defdom-transform :name "hide-ads-by-class"
                  :selector ".ad"
                  :action remove)

(defdom-transform :name "hide-sponsored"
                  :selector ".sponsored"
                  :action remove)

;; ── accessibility scoring ────────────────────────────────────────
;; Tag un-captioned images so downstream reader-mode can filter or
;; annotate them.
(defdom-transform :name "flag-images"
                  :selector "img"
                  :action add-class
                  :arg "nami-needs-alt-review")
