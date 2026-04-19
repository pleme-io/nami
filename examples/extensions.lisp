;; Unified extensions file for nami. Contains the full Lisp substrate:
;;
;;   (defstate …)     — runtime state cells (visit counts, preferences,
;;                       anything that should persist across navigations)
;;   (defeffect …)    — expressions that run on triggers, mutate state
;;                       via (set-state NAME VALUE)
;;   (defpredicate …) — named boolean checks over DOM + frameworks + state
;;   (defplan …)      — named bundles of transform names
;;   (defagent …)     — on trigger, if predicate, apply a plan/transform
;;
;; Copy to `$XDG_CONFIG_HOME/nami/extensions.lisp`. The existing
;; `transforms.lisp` still holds the `(defdom-transform …)` forms that
;; plans + agents reference.
;;
;; tatara-lisp's compile filters by keyword, so the order of forms
;; here doesn't matter — each loader picks up only its own type.

;; ── state cells ────────────────────────────────────────────────
(defstate :name "visit-count"   :initial 0)
(defstate :name "last-url"      :initial "")
(defstate :name "reader-mode-on" :initial #f)

;; ── effects: bump the visit counter on every page load ─────────
(defeffect :name "bump-visit-count"
           :on "page-load"
           :run "(set-state \"visit-count\" (+ visit-count 1))")

;; ── predicates: reusable page-shape checks ─────────────────────
(defpredicate :name "has-article"
              :selector "article"
              :min 1)

(defpredicate :name "prose-heavy"
              :selector "p"
              :min 3)

(defpredicate :name "likely-article"
              :all ("has-article" "prose-heavy"))

(defpredicate :name "has-any-ad"
              :selector ".ad"
              :min 1)

;; ── plans: named bundles of transforms (declared in transforms.lisp) ──
(defplan :name "reader-mode"
         :apply ("strip-script" "hide-ads"))

(defplan :name "full-cleanup"
         :apply ("reader-mode" "flag-editorial-images"))

;; ── agents: predicate-gated reactive transforms ────────────────
(defagent :name "auto-reader-mode"
          :on "page-load"
          :when "likely-article"
          :apply "reader-mode")

(defagent :name "ad-stripper"
          :on "page-load"
          :when "has-any-ad"
          :apply "hide-ads")
