# Technical Explanation

## Outcome And Intake

Enable the reader to explain a mechanism, assess a tradeoff, or operate a system correctly. Ask for reader expertise, the system/version/environment, exact learning or engineering decision, authoritative evidence, scope boundaries, failure modes, time budget, and output format. Unknown topology, throughput, SLA or behavior stays `xx` or explicitly unverified. Do not invent deployed infrastructure from a proposed design.

## Claim

Answer one technical question per page in a complete sentence. Distinguish a current observation, a design constraint, a hypothesis and a proposed change. A component list is not an explanation. State the causal relationship or tradeoff, with a source path, experiment, trace or specification that supports it. Define acronyms on first use and keep terms consistent.

## Logic

Build the ledger in this order: problem and constraints, system boundary, mechanism, failure behavior, alternatives, validation, and next action. The governing message explains the mechanism or engineering recommendation. Each transition must identify a dependency, sequence or cause. Do not present implementation details before the reader knows the boundary they belong to.

## Purpose Patterns

| Intent | Required inputs | Native starting point | Acceptance check |
| --- | --- | --- | --- |
| Explain the boundary | Actors, owned system, external dependencies | diagram/custom | Ownership and trust boundaries are labeled; no inferred deployment |
| Trace a request | Initiator, ordered operations, sync/async handoffs | flow/balanced | Arrow direction, data names, and reply path are explicit |
| Explain a dependency | Parent-child relation and dependency meaning | tree/focus | Children belong to the right parent; hierarchy is not confused with time |
| Compare choices | Common criteria, constraints, unknowns | matrix/labeled | Criteria and granularity match; assessment is attributed |
| Show failure handling | Trigger, detection, containment, recovery | vertical-flow/balanced | Failure, retry and human intervention remain distinguishable |
| Explain a control loop | Observed state, decision, action, feedback | cycle/focus | Feedback is real; diagram does not invent automation |
| Present a benchmark | Hardware, workload, versions, repeated measurements | horizontal-bar-graph/labeled | Units, baseline, distribution and limitations are stated |
| Plan a migration | Phases, owners, rollback and acceptance gates | gantt-chart/labeled | Dates and progress are supplied, rollback is not implied by an arrow |

These are starting components, not automatic sequence-diagram, trust-boundary or physical-timing certification. Add native annotations when a specialized composition is required.

## Draw

Use actor/ownership lanes for topology and a separate time axis for sequence. Do not put latency, topology, and organizational ownership on one unlabeled axis. Name the payload on a data edge and the condition on a decision branch. Use filled blocks for responsibilities and restrained connecting arrows for relationships. Keep text inside the safe rectangular region of nonrectangular nodes.

Keep diagrams abstract enough to read but specific enough to falsify. A logical diagram is not a deployment map. A protocol guarantee is not a benchmark result. Use paired before/after layouts without moving unchanged components. A code excerpt should be short, exact, versioned and essential; never execute code imported as slide content.

## Evidence And Unknowns

Every numerical claim needs an experiment or source with scope, sample size and units. Identify simulated versus measured values, throughput versus latency, percentile versus mean, and cold versus warm runs. Do not infer a causal improvement from one noisy measurement. Unknown capacity stays `xx`; never draw a proportional bar from it.

## Measure And Review

Trace one normal request and one failure through the diagram. Verify every named component against supplied evidence. Check that arrows cannot be mistaken for network traffic when they mean ownership. Validate labels, units, connector intrusion, font availability and actual PPTX rendering. Human review checks technical correctness and omitted failure modes; automation checks declared evidence, inputs and geometry.

## Delivery And Refinements

Deliver editable native elements with notes containing versions, evidence and limitations. Offer focused refinements such as a failure trace, measured tradeoff comparison, or an operator runbook view. Do not add speculative infrastructure to make the picture look complete.