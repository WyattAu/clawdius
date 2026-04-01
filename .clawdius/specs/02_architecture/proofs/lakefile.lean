import Lake

open Lake DSL

package clawdius_proofs where
  leanOptions := #[⟨`autoImplicit, false⟩]

require Std from git
  "https://github.com/leanprover-community/Std4" @ "v4.29.0"
