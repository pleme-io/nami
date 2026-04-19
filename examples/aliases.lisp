;; Starter framework-aliased selectors for nami.
;;
;; Copy to `$XDG_CONFIG_HOME/nami/aliases.lisp`. When transforms or
;; scrapes use a `@foo` selector, nami detects which framework(s)
;; rendered the page and resolves `@foo` to the right raw selector.
;;
;; Resolution rule: iterate detected frameworks in confidence order;
;; first one with an override wins; if nothing matches, use :fallback.
;;
;; Supported fields: :shadcn :mui :tailwind :bootstrap :react :nextjs
;; :remix :gatsby :vue :nuxt :svelte :sveltekit :angular :astro :solid
;; :htmx :alpine :wordpress :shopify :fallback (required).

;; ── layout primitives ──────────────────────────────────────────────
(defframework-alias :name "@card"
                    :shadcn "[data-slot=\"card\"]"
                    :mui "div.MuiCard-root"
                    :bootstrap "div.card"
                    :fallback "div.card"
                    :description "card component — shadcn data-slot, MUI class, bootstrap class")

(defframework-alias :name "@nav"
                    :shadcn "[data-slot=\"navigation-menu\"]"
                    :mui "header.MuiAppBar-root"
                    :bootstrap "nav.navbar"
                    :fallback "nav")

(defframework-alias :name "@button"
                    :shadcn "[data-slot=\"button\"]"
                    :mui "button.MuiButton-root"
                    :bootstrap "button.btn"
                    :fallback "button")

;; ── actionable signals ─────────────────────────────────────────────
(defframework-alias :name "@interactive"
                    :htmx "[hx-get], [hx-post], [hx-put], [hx-delete]"
                    :alpine "[x-on\\:click], [x-data]"
                    :fallback "button, a, input, select, textarea")

;; ── content landmarks ──────────────────────────────────────────────
(defframework-alias :name "@hero"
                    :shadcn "[data-slot=\"hero\"]"
                    :fallback "section.hero, .hero-section, header > h1")

(defframework-alias :name "@footer-links"
                    :fallback "footer a[href]")
