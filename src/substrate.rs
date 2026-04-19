//! Loaded Lisp substrate — the bundle of every `def*` type we
//! support in one place, loaded from `$XDG_CONFIG_HOME/nami/extensions.lisp`.
//!
//! tatara-lisp's `compile_typed::<T>` filters by keyword, so one
//! Lisp document can contain any mix of:
//!
//!   (defstate …)      (defeffect …)     (defpredicate …)
//!   (defplan …)        (defagent …)
//!
//! and each loader picks up only its own keyword. Users author a
//! single file; nami reads all of it.
//!
//! The [`Substrate`] struct holds compiled registries ready to use
//! during navigate. [`Browser::run_page_load`](crate::browser) is
//! the consumer.

use nami_core::agent::{AgentRegistry, AgentSpec};
use nami_core::component::{ComponentRegistry, ComponentSpec};
use nami_core::derived::{DerivedRegistry, DerivedSpec};
use nami_core::effect::{EffectRegistry, EffectSpec};
use nami_core::plan::{PlanRegistry, PlanSpec};
use nami_core::predicate::{PredicateRegistry, PredicateSpec};
use nami_core::query::{QueryRegistry, QuerySpec};
use nami_core::route::{RouteRegistry, RouteSpec};
use nami_core::store::{StateSpec, StateStore};
use std::path::Path;

/// Bundle of everything we compile out of `extensions.lisp`.
#[derive(Debug, Clone, Default)]
pub struct Substrate {
    pub states: Vec<StateSpec>,
    pub effects: EffectRegistry,
    pub predicates: PredicateRegistry,
    pub plans: PlanRegistry,
    pub agents: AgentRegistry,
    pub routes: RouteRegistry,
    pub queries: QueryRegistry,
    pub derived: DerivedRegistry,
    pub components: ComponentRegistry,
}

impl Substrate {
    /// Load from a file. Absent file = empty substrate, not an error.
    pub fn load(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let src = std::fs::read_to_string(path).map_err(|e| format!("read {path:?}: {e}"))?;
        Self::from_str(&src)
    }

    /// Compile the whole bundle from one Lisp source string. Each
    /// compile pass filters by keyword, so forms of other types in
    /// the same source are silently skipped by that pass.
    pub fn from_str(src: &str) -> Result<Self, String> {
        let states: Vec<StateSpec> = nami_core::store::compile(src)?;

        let effect_specs: Vec<EffectSpec> = nami_core::effect::compile(src)?;
        let mut effects = EffectRegistry::new();
        effects.extend(effect_specs);

        let pred_specs: Vec<PredicateSpec> = nami_core::predicate::compile(src)?;
        let mut predicates = PredicateRegistry::new();
        predicates.extend(pred_specs);

        let plan_specs: Vec<PlanSpec> = nami_core::plan::compile(src)?;
        let mut plans = PlanRegistry::new();
        plans.extend(plan_specs);

        let agent_specs: Vec<AgentSpec> = nami_core::agent::compile(src)?;
        let mut agents = AgentRegistry::new();
        agents.extend(agent_specs);

        let route_specs: Vec<RouteSpec> = nami_core::route::compile(src)?;
        let mut routes = RouteRegistry::new();
        routes.extend(route_specs);

        let query_specs: Vec<QuerySpec> = nami_core::query::compile(src)?;
        let mut queries = QueryRegistry::new();
        queries.extend(query_specs);

        let derived_specs: Vec<DerivedSpec> = nami_core::derived::compile(src)?;
        let mut derived = DerivedRegistry::new();
        derived.extend(derived_specs);

        let component_specs: Vec<ComponentSpec> = nami_core::component::compile(src)?;
        let mut components = ComponentRegistry::new();
        components.extend(component_specs);

        Ok(Self {
            states,
            effects,
            predicates,
            plans,
            agents,
            routes,
            queries,
            derived,
            components,
        })
    }

    /// Non-empty summary for logging at startup.
    #[must_use]
    pub fn summary(&self) -> String {
        format!(
            "{} state · {} effect · {} predicate · {} plan · {} agent · {} route · {} query · {} derived · {} component",
            self.states.len(),
            self.effects.len(),
            self.predicates.len(),
            self.plans.len(),
            self.agents.len(),
            self.routes.len(),
            self.queries.len(),
            self.derived.len(),
            self.components.len(),
        )
    }

    /// True if every registry is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
            && self.effects.is_empty()
            && self.predicates.is_empty()
            && self.plans.is_empty()
            && self.agents.is_empty()
            && self.routes.is_empty()
            && self.queries.is_empty()
            && self.derived.is_empty()
            && self.components.is_empty()
    }

    /// Produce a fresh [`StateStore`] seeded with this substrate's
    /// declared state specs. The store is owned by the Browser for
    /// the lifetime of the session.
    #[must_use]
    pub fn build_state_store(&self) -> StateStore {
        StateStore::from_specs(&self.states)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_source_is_empty_substrate() {
        let s = Substrate::from_str("").unwrap();
        assert!(s.is_empty());
    }

    #[test]
    fn missing_file_is_empty_substrate() {
        let s = Substrate::load(Path::new("/definitely/does/not/exist.lisp")).unwrap();
        assert!(s.is_empty());
    }

    #[test]
    fn mixed_source_loads_each_keyword() {
        let src = r#"
            (defstate :name "counter" :initial 0)
            (defeffect :name "bump"
                       :on "page-load"
                       :run "(set-state \"counter\" (+ counter 1))")
            (defpredicate :name "has-article" :selector "article" :min 1)
            (defplan :name "reader-mode"
                     :apply ("hide-ads" "flag-images"))
            (defagent :name "auto-reader"
                      :on "page-load"
                      :when "has-article"
                      :apply "reader-mode")
            (defroute :pattern "/users/:id"
                      :bind (("user-id" "id"))
                      :on-match ("load-user"))
            (defquery :name "load-user"
                      :endpoint "https://api.example/users/:id"
                      :method "GET"
                      :into "user")
            (defderived :name "visits-squared"
                        :inputs ("counter")
                        :compute "(* counter counter)")
            (defcomponent :name "Banner"
                          :props ("title")
                          :template "(div :class \"banner\" (h2 (@ title)))")
        "#;
        let s = Substrate::from_str(src).unwrap();
        assert_eq!(s.states.len(), 1);
        assert_eq!(s.effects.len(), 1);
        assert_eq!(s.predicates.len(), 1);
        assert_eq!(s.plans.len(), 1);
        assert_eq!(s.agents.len(), 1);
        assert_eq!(s.routes.len(), 1);
        assert_eq!(s.queries.len(), 1);
        assert_eq!(s.derived.len(), 1);
        assert_eq!(s.components.len(), 1);
        assert!(!s.is_empty());
    }

    #[test]
    fn state_store_seeds_from_specs() {
        let src = r#"
            (defstate :name "x" :initial 42)
            (defstate :name "flag" :initial #t)
        "#;
        let s = Substrate::from_str(src).unwrap();
        let store = s.build_state_store();
        assert_eq!(store.get("x"), Some(serde_json::json!(42)));
        assert_eq!(store.get("flag"), Some(serde_json::json!(true)));
    }
}
