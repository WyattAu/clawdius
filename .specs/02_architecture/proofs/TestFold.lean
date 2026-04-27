import Init

namespace Test

def step (b : Nat) (s : Nat) (t : Nat) : Nat := s + t

example (b x : Nat) (xs : List Nat) :
    (List.foldl (step b) 0 (x :: xs)) = List.foldl (step b) (step b 0 x) xs := by
  simp only [List.foldl]

end Test
