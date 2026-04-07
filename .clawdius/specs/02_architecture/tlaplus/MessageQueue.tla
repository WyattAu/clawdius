---- MODULE MessageQueue ----
EXTENDS Naturals, Sequences, FiniteSets
CONSTANT Tasks, MaxRetries, MaxQueueSize

(* Corresponds to: crates/clawdius-core/src/messaging/retry_queue.rs
   - RetryQueue: HashMap<String, RetryTask> (pending) + HashMap (dead_letter)
   - RetryTask: id, platform, chat_id, message, attempt, max_retries, next_retry_at
   - RetryConfig: max_retries, initial_delay_ms, max_delay_ms, exponential_base
   - enqueue(): creates task with attempt=0, checks max_queue_size
   - mark_failed(): increments attempt, exponential backoff, moves to dead_letter
   - mark_success(): removes task from pending
   - At-least-once: task stays in queue until explicitly marked success
   - Dead letter: tasks exceeding max_retries moved to dead_letter queue *)

VARIABLES pending, dead_letter, attempt, delivered, next_seq

TaskState == {"pending", "delivered", "dead"}

TypeInvariant ==
    /\ pending \subseteq Tasks
    /\ dead_letter \subseteq Tasks
    /\ attempt \in [Tasks -> 0..MaxRetries]
    /\ delivered \subseteq Tasks
    /\ pending \intersect dead_letter = {}
    /\ delivered \intersect pending = {}
    /\ delivered \intersect dead_letter = {}
    /\ Cardinality(pending) + Cardinality(dead_letter) <= MaxQueueSize
    /\ next_seq \in Nat

Init ==
    /\ pending = {}
    /\ dead_letter = {}
    /\ attempt = [t \in Tasks |-> 0]
    /\ delivered = {}
    /\ next_seq = 0

Enqueue(t) ==
    /\ t \notin pending
    /\ t \notin dead_letter
    /\ t \notin delivered
    /\ Cardinality(pending) + Cardinality(dead_letter) < MaxQueueSize
    /\ pending' = pending \union {t}
    /\ attempt' = [attempt EXCEPT ![t] = 0]
    /\ dead_letter' = dead_letter
    /\ delivered' = delivered
    /\ next_seq' = next_seq

MarkSuccess(t) ==
    /\ t \in pending
    /\ pending' = pending \ {t}
    /\ delivered' = delivered \union {t}
    /\ attempt' = attempt
    /\ dead_letter' = dead_letter
    /\ next_seq' = next_seq

MarkFailed(t) ==
    /\ t \in pending
    /\ attempt[t] < MaxRetries
    /\ attempt' = [attempt EXCEPT ![t] = attempt[t] + 1]
    /\ pending' = pending
    /\ dead_letter' = dead_letter
    /\ delivered' = delivered
    /\ next_seq' = next_seq

MoveToDeadLetter(t) ==
    /\ t \in pending
    /\ attempt[t] = MaxRetries
    /\ pending' = pending \ {t}
    /\ dead_letter' = dead_letter \union {t}
    /\ attempt' = attempt
    /\ delivered' = delivered
    /\ next_seq' = next_seq

Next == \E t \in Tasks :
    \/ Enqueue(t)
    \/ MarkSuccess(t)
    \/ MarkFailed(t)
    \/ MoveToDeadLetter(t)

Spec == Init /\ [][Next]_<<pending, dead_letter, attempt, delivered, next_seq>>

PendingAndDeadLetterDisjoint ==
    [](pending \intersect dead_letter = {})

TaskEventuallyResolved ==
    [](t \in pending ~> <>(t \in delivered \/ t \in dead_letter))

MaxRetriesThenDeadLetter ==
    [](t \in dead_letter => attempt[t] = MaxRetries)

DeliveredNeverRetried ==
    [](\A t \in delivered : t \notin pending /\ t \notin dead_letter)

AtLeastOnceDelivery ==
    [](t \in pending => ~\E s \in Nat : <<delivered>> = [delivered EXCEPT !{t} = {}])

QueueSizeBound ==
    [](Cardinality(pending) + Cardinality(dead_letter) <= MaxQueueSize)

=============================================================================
(* TLC configuration:
   --SPECIFICATION Spec
   --INVARIANT TypeInvariant
   --INVARIANT PendingAndDeadLetterDisjoint
   --INVARIANT DeliveredNeverRetried
   --INVARIANT QueueSizeBound
   --INVARIANT MaxRetriesThenDeadLetter
   --PROPERTY TaskEventuallyResolved
   --CONSTANT Tasks = {t1, t2, t3}
   --CONSTANT MaxRetries = 3
   --CONSTANT MaxQueueSize = 5
*)
====
