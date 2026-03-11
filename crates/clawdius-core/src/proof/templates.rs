//! Proof templates for common verification patterns

use super::types::ProofTemplate;

/// Template for termination proofs
///
/// Proves that a function terminates for all inputs
pub fn termination_proof_template() -> ProofTemplate {
    ProofTemplate::new(
        "termination",
        r#"/-- Termination proof for {function_name} -/
theorem {name}_terminates : ∀ (input : {input_type}), 
    ∃ (output : {output_type}), {function_name} input = output := by
  {proof_body}"#,
    )
    .with_description("Proves that a function terminates for all inputs")
}

/// Template for correctness proofs
///
/// Proves that a function satisfies its specification
pub fn correctness_proof_template() -> ProofTemplate {
    ProofTemplate::new(
        "correctness",
        r#"/-- Correctness proof for {function_name} -/
theorem {name}_correct : ∀ (input : {input_type}),
    {precondition} input →
    {postcondition} ({function_name} input) := by
  {proof_body}"#,
    )
    .with_description("Proves that a function satisfies its specification")
}

/// Template for safety proofs
///
/// Proves that a function never enters an unsafe state
pub fn safety_proof_template() -> ProofTemplate {
    ProofTemplate::new(
        "safety",
        r#"/-- Safety proof for {function_name} -/
theorem {name}_safe : ∀ (input : {input_type}),
    {invariant} input →
    {invariant} ({function_name} input) ∧
    {safety_property} ({function_name} input) := by
  {proof_body}"#,
    )
    .with_description("Proves that a function maintains safety invariants")
}

/// Template for bisimulation proofs
#[allow(dead_code)]
pub fn bisimulation_proof_template() -> ProofTemplate {
    ProofTemplate::new(
        "bisimulation",
        r#"/-- Bisimulation proof between {impl_a} and {impl_b} -/
theorem {name}_bisim : ∀ (s : State) (a : Action),
    {relation} s →
    {relation} ({step_a} s a) ({step_b} s a) := by
  {proof_body}"#,
    )
    .with_description("Proves bisimulation between two implementations")
}

/// Template for memory safety proofs
#[allow(dead_code)]
pub fn memory_safety_proof_template() -> ProofTemplate {
    ProofTemplate::new(
        "memory_safety",
        r#"/-- Memory safety proof for {function_name} -/
theorem {name}_memory_safe : ∀ (ptr : {pointer_type}) (len : Nat),
    {valid_pointer} ptr len →
    {access_in_bounds} ({function_name} ptr) len := by
  {proof_body}"#,
    )
    .with_description("Proves memory safety for pointer operations")
}

/// Template for cryptographic security proofs
#[allow(dead_code)]
pub fn crypto_security_proof_template() -> ProofTemplate {
    ProofTemplate::new(
        "crypto_security",
        r#"/-- Security proof for {primitive} -/
theorem {name}_secure : ∀ (adv : Adversary) (msg : Message),
    {advantage} adv {primitive} msg ≤ {bound} := by
  {proof_body}"#,
    )
    .with_description("Proves cryptographic security bounds")
}

/// Template for concurrency safety proofs
#[allow(dead_code)]
pub fn concurrency_safety_proof_template() -> ProofTemplate {
    ProofTemplate::new(
        "concurrency_safety",
        r#"/-- Concurrency safety proof -/
theorem {name}_concurrent_safe : ∀ (t1 t2 : Thread) (s : SharedState),
    {race_free} t1 t2 s →
    {linearizable} ({op1} t1 s) ({op2} t2 s) := by
  {proof_body}"#,
    )
    .with_description("Proves thread safety and race freedom")
}

/// Get all built-in templates
#[allow(dead_code)]
pub fn all_templates() -> Vec<ProofTemplate> {
    vec![
        termination_proof_template(),
        correctness_proof_template(),
        safety_proof_template(),
        bisimulation_proof_template(),
        memory_safety_proof_template(),
        crypto_security_proof_template(),
        concurrency_safety_proof_template(),
    ]
}

/// Find a template by name
#[allow(dead_code)]
pub fn find_template(name: &str) -> Option<ProofTemplate> {
    all_templates().into_iter().find(|t| t.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_termination_template() {
        let template = termination_proof_template();
        assert_eq!(template.name, "termination");
        assert!(template.placeholders.contains(&"name".to_string()));
        assert!(template.placeholders.contains(&"function_name".to_string()));
    }

    #[test]
    fn test_correctness_template_render() {
        let template = correctness_proof_template();

        let mut values = HashMap::new();
        values.insert("name".to_string(), "sort".to_string());
        values.insert("function_name".to_string(), "quicksort".to_string());
        values.insert("input_type".to_string(), "List Nat".to_string());
        values.insert("precondition".to_string(), "fun _ => True".to_string());
        values.insert("postcondition".to_string(), "IsSorted".to_string());
        values.insert("proof_body".to_string(), "sorry".to_string());

        let result = template.render(&values).unwrap();
        assert!(result.contains("theorem sort_correct"));
        assert!(result.contains("quicksort"));
    }

    #[test]
    fn test_find_template() {
        let template = find_template("safety");
        assert!(template.is_some());
        assert_eq!(template.unwrap().name, "safety");
    }

    #[test]
    fn test_all_templates_count() {
        let templates = all_templates();
        assert_eq!(templates.len(), 7);
    }
}
