# Meaning checks and research basis

## Synthetic editing cases

These are original examples, not quotations or factual recommendations. Each case
states what an editor must preserve. A valid response need not match one exact
wording. Use them to review edits, not as automatic word-replacement rules.

| Case | Source or context | Expected editing decision |
| --- | --- | --- |
| Direct verb | The finance team will carry out a review of the invoices on Friday. | The finance team will review the invoices on Friday. Keep actor, action, date and future commitment. |
| Noun stack | Context: a quarterly meeting to review customer access. Heading: Quarterly customer access review meeting. | Quarterly meeting to review customer access. Keep the meeting as well as its purpose; without that context, ask. |
| Known to new | The app sends events to a queue. This queue stores the events until the worker is ready. | The app sends events to a queue, which stores them until the worker is ready. Preserve the storage condition; two sentences are also valid. |
| Unknown actor | The contract was approved yesterday. | Keep unchanged. Do not invent a board, manager or `we` to make it active. |
| Articles and scope | A reviewer may approve a request. | Keep unchanged. `The reviewer must approve all requests` changes identity, permission and scope. |
| Parallel instructions | Procedure bullets: Review the invoice; Archiving the record; Notify the owner. | Review the invoice; Archive the record; Notify the owner. Keep all three actions and their order. |
| Modal ambiguity | Staff may not access the archive. No other context. | Ask, do not guess: is access prohibited, or might access be unavailable? Do not silently choose `must not` or `might not`. |
| Quantifier scope | Not all requests require approval. | Keep unchanged. `No requests require approval` and `All requests can skip approval` are not equivalents of `not all`. |
| Only | Only pilot users can edit records. | Keep unchanged. Moving `only` to `can only edit records` changes what is restricted. |
| Tense and aspect | We have been testing the patch since Monday. | Keep the ongoing activity; do not write that testing is complete or that the patch passed. |
| No causal upgrade | After the rollout, the median wait fell from 12 minutes to 9. The cause is unconfirmed. | The median wait fell from 12 to 9 minutes after the rollout; the cause is unconfirmed. Do not say the rollout cut waits. |
| Statistical caveat | The difference was not statistically significant. | Keep unchanged. This does not establish that there was no difference or that the options are equivalent. |
| Real contrast | The service stores IDs, not passwords. | Keep unchanged. Removing `not passwords` discards a meaningful data-retention boundary. |
| Empty staging | It is worth noting that the review is scheduled for Thursday. | The review is scheduled for Thursday. Do not change scheduled to completed. |
| Date ambiguity | Deadline: 03/04/2027. Locale unspecified. | Ask, do not guess. US and UK readings can differ. Preserve the date until clarified. |
| Do not fabricate voice | I was relieved when the review ended. | Keep the writer's emotion. Do not add a joke, struggle narrative or stronger feeling. |
| Protected material | Citation, quoted policy text, command or API identifier. | Keep unchanged unless the exact change is authorized; edit surrounding prose instead. |

## Review an edited passage

1. Compare what is claimed and who does what. Identify every added actor, example,
   cause, source, date or calculation and require source support or authorization.
2. Compare negatives, quantifiers, modal verbs, articles, time and comparisons.
   A shorter sentence is not better if its logical scope has moved.
3. Confirm that moving clauses or splitting a sentence kept conditions attached
   to the right action. Confirm pronouns still point to one recoverable referent.
4. Check tone and medium. Preserve an unknown actor's passive sentence, deliberate
   term repetition, a real contrast or a useful list even if a heuristic flags it.
5. For slides, review the headline, visual labels, visible caveats and notes as
   one unit. Ask what a reader seeing only the slide could incorrectly conclude.
6. Return unresolved ambiguities outside the edited text. Do not report a
   readability score or absence of flagged phrases as proof of comprehension.

The repository's contract tests check discovery, reference paths and the presence
of these safeguards. They do not run an LLM or establish editing quality. For a
model comparison, use the same source passages and audience for both conditions,
judge meaning retention separately from readability, include unchanged and
ambiguous cases, and report actual model/configuration and reviewer limitations.

## Sources consulted

Consulted on 2026-10-04. These sources inform editorial choices, not universal
linguistic laws or guaranteed model behavior. The workflow and examples here are
independently written; no upstream skill, script or template is bundled.

| Source | Used for | Deliberate adaptation |
| --- | --- | --- |
| [NARA: Top 10 principles for plain language](https://www.archives.gov/open/plain-writing/10-principles.html) | Audience, main point first, everyday words, subject/verb proximity, useful headings and lists | Active voice and short sentences are preferences, not bans on clear passive clauses or useful longer explanations. |
| [Google developer style: Sentence structure](https://developers.google.com/style/sentence-structure) | Applicability conditions before instructions | Do not move a condition just to make every sentence start with a verb; retain logical scope. This technical guide is not the default voice for all documents. |
| [Assertion-evidence approach](https://www.assertion-evidence.com/) | Message-led presentation headlines supported by visual evidence | Useful for explanatory slides; covers, agendas and procedural lists need not become assertions. Do not remove every bullet or invent a stronger claim. |
| [Humanizer skill](https://github.com/blader/humanizer) | Comparison with an existing English editing workflow: staging, inflated claims and formulaic rhythm | Do not copy its word lists or formatting bans, automatically remove real contrasts, or add a reaction to simulate a human voice. Reader understanding, not detector avoidance, is the goal. |

Humanizer's repository publishes an MIT license. Google identifies its prose as
CC BY 4.0. Links acknowledge ideas consulted; no source text, example corpus or
code is redistributed here. No license is selected for AISlide by this work.

Local `article-writing` focuses on long-form drafting, and `brand-voice` on deriving
a voice profile. Neither supplies this bounded English-language editing and
presentation contract. Use an approved voice profile as input; do not install
or enable competing editors automatically. Readings that could not be retrieved
reliably were not used as authority for these instructions.