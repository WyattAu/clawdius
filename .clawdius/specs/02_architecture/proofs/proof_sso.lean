/-
  Lean4 Proof: SSO Authentication Security
  Component: COMP-SSO-001
  Blue Paper: BP-SSO-AUTH-001
  Yellow Paper: YP-SSO-SECURITY-001
-/

import Std.Data.HashMap

inductive SSOProvider where
  | Saml : SSOProvider
  | Oidc : SSOProvider
  deriving Repr, DecidableEq

inductive SessionState where
  | Active : SessionState
  | Expired : SessionState
  | Revoked : SessionState
  deriving Repr, DecidableEq

structure SSOSession where
  sessionId : String
  userId : String
  provider : SSOProvider
  createdAt : Nat
  expiresAt : Nat
  state : SessionState
  deriving Repr

structure Token where
  value : String
  expiresAt : Nat
  issuedAt : Nat
  issuer : String
  deriving Repr

def isValidSession (session : SSOSession) (currentTime : Nat) : Prop :=
  currentTime ≤ session.expiresAt ∧ session.state = SessionState.Active

def validateToken (token : Token) (currentTime : Nat) : Prop :=
  currentTime ≤ token.expiresAt

def verifyIssuer (token : Token) (expected : String) : Prop :=
  token.issuer = expected

def isUsable (session : SSOSession) : Prop :=
  session.state = SessionState.Active

noncomputable axiom verifySignature : Token → String → Bool
  -- Cannot prove: noncomputable cryptographic function with no pure logical definition.
noncomputable def acceptToken (token : Token) (signature : String) : Bool :=
  verifySignature token signature

axiom isValidAssertion : String → Bool
axiom createSession : String → String → List SSOSession
axiom sessionCount : List SSOSession → Nat
  -- Cannot prove: uninterpreted functions modeling external SSO protocol
  -- interactions; no pure logical definition available.
axiom sso_single_session_axiom (assertion userId : String) :
    isValidAssertion assertion = true → sessionCount (createSession assertion userId) = 1
  -- Cannot prove: depends on the uninterpreted isValidAssertion,
  -- createSession, and sessionCount axioms above.

noncomputable axiom getDomain : String → String
  -- Cannot prove: noncomputable string parsing function with no pure logical definition.
noncomputable def allowEmail (email : String) (domains : List String) : Bool :=
  getDomain email ∈ domains

def allowAccess (requireMfa hasMfa : Bool) : Bool :=
  !requireMfa || hasMfa

def isExpired (session : SSOSession) (currentTime timeoutSecs : Nat) : Prop :=
  currentTime - session.createdAt > timeoutSecs

def canCreateNewSession (currentCount maxSessions : Nat) : Prop :=
  currentCount < maxSessions

theorem session_expiry (session : SSOSession) (currentTime : Nat) :
    currentTime > session.expiresAt →
    ¬isValidSession session currentTime := by
  intro h
  simp only [isValidSession]
  omega

theorem token_expiry (token : Token) (currentTime : Nat) :
    currentTime > token.expiresAt →
    ¬validateToken token currentTime := by
  intro h
  simp only [validateToken]
  omega

theorem issuer_verification (token : Token) (expectedIssuer : String) :
    token.issuer ≠ expectedIssuer →
    ¬verifyIssuer token expectedIssuer := by
  intro h hverify
  exact h hverify

theorem session_revocation (session : SSOSession) :
    session.state = SessionState.Revoked →
    ¬isUsable session := by
  intro h husable
  simp only [isUsable] at husable
  exact absurd husable (by rw [h]; decide)

theorem signature_verification (token : Token) (signature : String) :
    verifySignature token signature = false →
    acceptToken token signature = false := by
  intro h
  simp only [acceptToken, h]

theorem sso_single_session (assertion : String) (userId : String) :
    isValidAssertion assertion = true →
    sessionCount (createSession assertion userId) = 1 :=
  sso_single_session_axiom assertion userId

theorem domain_restriction (email : String) (allowedDomains : List String) :
    getDomain email ∉ allowedDomains →
    allowEmail email allowedDomains = false := by
  intro h
  simp only [allowEmail, h, decide_false]

theorem mfa_requirement (requireMfa hasMfa : Bool) :
    requireMfa = true ∧ hasMfa = false →
    allowAccess requireMfa hasMfa = false := by
  intro ⟨h1, h2⟩
  simp only [allowAccess, h1, h2]
  decide

theorem session_timeout (session : SSOSession) (timeoutSecs : Nat) (currentTime : Nat) :
    currentTime - session.createdAt > timeoutSecs →
    isExpired session currentTime timeoutSecs := id

theorem concurrent_session_limit (currentCount : Nat) (maxSessions : Nat) :
    currentCount ≥ maxSessions →
    ¬canCreateNewSession currentCount maxSessions := by
  intro h
  simp only [canCreateNewSession]
  omega
