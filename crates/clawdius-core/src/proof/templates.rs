//! Proof templates for common verification patterns

use super::types::ProofTemplate;

/// Template for termination proofs
///
/// Proves that a function terminates for all inputs
#[must_use]
pub fn termination_proof_template() -> ProofTemplate {
    ProofTemplate::new(
        "termination",
        r"/-- Termination proof for {function_name} -/
theorem {name}_terminates : ∀ (input : {input_type}), 
    ∃ (output : {output_type}), {function_name} input = output := by
  {proof_body}",
    )
    .with_description("Proves that a function terminates for all inputs")
}

/// Template for correctness proofs
///
/// Proves that a function satisfies its specification
#[must_use]
pub fn correctness_proof_template() -> ProofTemplate {
    ProofTemplate::new(
        "correctness",
        r"/-- Correctness proof for {function_name} -/
theorem {name}_correct : ∀ (input : {input_type}),
    {precondition} input →
    {postcondition} ({function_name} input) := by
  {proof_body}",
    )
    .with_description("Proves that a function satisfies its specification")
}

/// Template for safety proofs
///
/// Proves that a function never enters an unsafe state
#[must_use]
pub fn safety_proof_template() -> ProofTemplate {
    ProofTemplate::new(
        "safety",
        r"/-- Safety proof for {function_name} -/
theorem {name}_safe : ∀ (input : {input_type}),
    {invariant} input →
    {invariant} ({function_name} input) ∧
    {safety_property} ({function_name} input) := by
  {proof_body}",
    )
    .with_description("Proves that a function maintains safety invariants")
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
    fn test_safety_template() {
        let template = safety_proof_template();
        assert_eq!(template.name, "safety");
    }
}
