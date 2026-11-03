# Agentic workflow

Status: **Phase 1 backend built, off by default** (2026-11-03). How every
operation in the logistics system can be run by agents with as little human
involvement as possible, what's built, and the phases to get there.

## Summary

The aim is that people handle exceptions, not routine work. An agent does the
routine, and each decision is either carried out, or put in front of a person
with the agent's reasoning, according to rules enforced in code.

**Not everything should become an agent.** Of the 91 API operations today, 37
are reads and 54 are writes. Agents use the reads and the routine writes. The
rest stay as they are:

- **Rules stay in code.** The dispatch state machine, vehicle eligibility and
  invoice arithmetic are already correct and deterministic. An agent calls them;
  it doesn't replace them.
- **Some operations get no agent at all:** deleting data, user management,
  credentials and tokens. An agent can't call a tool that doesn't exist.
- **Some facts need a sensor or a person.** Software can't know a truck was
  loaded or that goods were handed over. Those come from the driver's phone,
  GPS and warehouse staff. Agents act on those signals; they don't assume them.

**What's blocking this now:** the Anthropic account has no credits (see
[Prerequisites](#prerequisites)), and several agents need things that don't
exist yet, mainly a way for orders and replies to come in other than the
dashboard.

## How it works

```
 Events and timers          Agent loop (agent/runtime.rs)
 ─────────────────          ───────────────────────────────
 hourly scheduler  ───────▶ Claude ──asks for a tool──▶ Gate
 "Run now" button                ▲                       │
 (Phase 2: domain events)        │                       ├─ read: answer it
                                 │                       ├─ unknown tool: refuse
                                 │                       ├─ validate against the
                                 │                       │  org and domain rules
                                 │                       └─ autonomy policy:
                                 │                          record / ask a person
                                 └──── tool result ─────────── / carry it out
                                                             │
                 Same model code as the dashboard ◀──────────┘
                 AgentRuns + AgentActions (full audit trail)
```

Safety doesn't depend on the prompt. Customer names and notes reach the model,
so a prompt injection has to be assumed possible. What limits the damage is
enforced outside the model:

| Control | Where | Effect |
| --- | --- | --- |
| Tool catalog | `agent/tools.rs` | No tool exists to delete, pay, or manage users or credentials |
| Org scope | `org_id` comes from the run, never the model | Another org's records read as not found |
| Autonomy policy | `store::decide` | Sensitive actions always wait for a person |
| Domain rules | Same `transition_to` as the dashboard | Illegal moves fail for agents too |
| Hard rules in tools | `tools::validate` | An expired vehicle document blocks confirming, whatever the model says |
| Recheck on approval | `tools::execute` | An approval that has gone stale fails instead of applying |
| Limits | `runtime.rs` | 12 turns and 20 actions per run, a daily token limit per org |
| Kill switch | Mode `OFF` | A running agent stops at its next turn |
| Audit trail | `AgentRuns`, `AgentActions` | Every request, its reason and who decided |

### Autonomy modes

Each org picks a mode (Admin only). New orgs start at `OFF`. Turn it up in order.

| | Internal (raise an alert) | Operational (confirm a dispatch) | Sensitive (cancel a dispatch) |
| --- | --- | --- | --- |
| `OFF` | Agents don't run | | |
| `SHADOW` | Recorded only | Recorded only | Recorded only |
| `SUPERVISED` | Happens | Waits for a person | Waits for a person |
| `AUTONOMOUS` | Happens | Happens | Waits for a person |

`SHADOW` is how an org finds out whether the agent's judgement can be trusted,
at no risk: compare what it would have done with what staff did.

### Why a router and not a supervising agent

One LLM "manager" agent handing work to others adds cost and a new way to fail,
and gains nothing here. The routing is known in advance: a delivered dispatch
goes to billing. So events route to agents in plain code. Each agent is a prompt
and a list of tools, with the loop, gate and audit trail shared.

The loop runs in this backend, not on a hosted agent platform, because its tools
are this backend's own database and domain code.

## Operations by agent

| Agent | Takes over | Autonomous | Waits for a person | Needs first |
| --- | --- | --- | --- | --- |
| **Dispatch desk** (built) | Reviewing open dispatches | Confirm, raise alerts | Cancel | Nothing |
| **Order intake** | Dispatch creation, new customers | Create when customer and stock are unambiguous | Anything ambiguous, unusual quantities | Inbound email or WhatsApp |
| **Trip planner** | Grouping stops, own or hired vehicle | Group stops on own vehicles | Hiring at a cost | Rate cards |
| **Delivery monitor** | Loaded, in transit, delivered, returned; delay alerts | Advance status from driver and GPS signals | Returns, disputed deliveries | Driver app status actions; [proof-of-delivery check](../todo.org) |
| **Fleet and compliance** | Document and maintenance tracking | Reminders, take a non-compliant vehicle out of planning | Booking paid work | Vehicle availability flag |
| **Warehouse** | Stock levels, transfers, reorder | Suggest reorders, propose transfers | Stock adjustments, purchases | Purchase order concept |
| **Vendor and hire** | Hiring market trucks | Contact vendors, assign an offered truck within budget | Payments, above-budget hires | Vendor messaging, rate cards |
| **Billing** | Invoices and collections | Raise on delivery, send reminders | Amounts, recording payment | Payment gateway to confirm payment |
| **Customer comms** | Notifications, questions | Status answers | Complaints, refunds | Inbound channel |
| **Driver assistant** | Driver questions | Answer about their own trips | Nothing: read-only | Android app work ([RAG Phase 5](rag-integration.md)) |
| **Ops analyst** | Reports, digest | Daily summary to the owner | Nothing: read-only | Scheduled delivery |
| **Data quality** | Onboarding, bad records | Flag problems | All changes | Nothing |

### What stays with people

- **Physical work:** loading, driving, handing over goods.
- **Money:** agents can raise invoices and chase them, but recording a payment
  or paying a vendor stays human until a gateway confirms it independently.
- **Decisions about relationships and liability:** complaints, disputes,
  cancelling on a customer.
- **Administration, with no agent at all:** users and roles, deleting anything,
  org settings, credentials, device and tracker tokens.

"Minimized" realistically means staff work from an exception inbox rather than
running each order by hand. It does not mean nobody is involved.

## Prerequisites

| Blocker | Detail |
| --- | --- |
| **Anthropic credits** | Tried on 2026-11-03: the API key is rejected with "credit balance is too low". No agent can run, and the existing AI features can't reach Claude either. |
| Live check | Because of that, no real agent run has happened. `cargo test live_agent_run -- --ignored` checks it once there are credits. Tests so far use a fake of the API. |
| Email and SMS | Agents need to reach people: PRs #107 and #108 aren't merged or live. |
| Data quality | The live data has placeholder phone numbers and many test orgs. Agents acting on that data would act badly. Decide what's real before going past `SHADOW`. |

## Cost

**These are estimates, not measurements:** assuming about 30,000 input and 2,000
output tokens per run, hourly, for one org, before any prompt-caching saving.

| Model | Per run | Per month |
| --- | --- | --- |
| Claude Opus 5.5 ($4 / $20 per million tokens) | about $0.16 | about $115 |
| Claude Haiku 5.5 ($0.10 / $0.50 per million tokens) | about $0.004 | about $3 |

The default is Opus 5.5. `AGENT_MODEL` changes it, and the choice should come
from comparing both in `SHADOW` on real work. Running less often, or only when
something changes (Phase 2), cuts this further.

## Phases

Each agent goes through `SHADOW` before anything else. Move an agent up only
when its record justifies it: as a starting bar, staff agreed with at least 95%
of 200 or more shadow decisions, with no serious mistake.

**Phase 1: foundation and first agent.** Backend built: loop, gate, audit, modes,
routes, dispatch desk. Still to do: the dashboard inbox (logistics-frontend),
with unit, Playwright and standalone demo tests, and the first live run.

**Phase 2: events.** Replace polling with an outbox of domain events (dispatch
created, delivered, document expiring), so agents run when something happens.
Move notification sending out of the route handlers and into the model layer, so
agent actions notify people exactly as dashboard actions do.

**Phase 3: agents needing no new channels.** Fleet and compliance, warehouse,
ops analyst, data quality.

**Phase 4: driver signals.** Status actions in the Android app, then the delivery
monitor and trip planner.

**Phase 5: billing.** Raise on delivery and send reminders. Payment recording
stays human.

**Phase 6: inbound channels.** Email or WhatsApp in, then order intake, customer
comms, vendor and hire. This is the biggest reduction in human work, and it's the
riskiest, since it's the first time outsiders' text directly drives agents.

**Phase 7: payment gateway.** Only then consider automatic payment recording.

## What's built (Phase 1)

- `src/logistics/agent/`: `model.rs` (API call), `tools.rs` (catalog, validation),
  `store.rs` (policy, audit tables), `runtime.rs` (loop, approvals, scheduler).
- **Tools:** `list_open_dispatches`, `list_compliance_issues`, `list_low_stock`,
  `confirm_dispatch`, `cancel_dispatch`, `raise_alert`. Read tools don't include
  customer names or contact details, so those aren't sent to the model.
- **Tables:** `AgentSettings`, `AgentRuns`, `AgentActions`, removed with the org.
- **Settings:** `AGENTS_ENABLED=true` starts the timer, and the org's mode must
  also be on. `AGENT_INTERVAL_MINUTES` (60), `AGENT_MODEL` (`claude-opus-5-5`),
  `AGENT_EFFORT` (`medium`), `AGENT_DAILY_TOKEN_LIMIT` (500000).
- **Request:** uses the API's server-side refusal fallback, so a safety classifier
  refusal can be retried on a suitable model.

### Routes

| Method & path | Who | |
| --- | --- | --- |
| `GET /api/orgs/{id}/agents/settings` | Any member | The autonomy mode |
| `PUT /api/orgs/{id}/agents/settings` | Admin | Change it |
| `POST /api/orgs/{id}/agents/run` | Admin, Dispatcher | Run now and wait for the result |
| `GET /api/orgs/{id}/agents/runs` | Any member | Recent runs |
| `GET /api/orgs/{id}/agents/actions?status=` | Any member | Recent actions, such as `PENDING_APPROVAL` |
| `POST /api/agent-actions/{id}/approve` | Admin, Dispatcher | Recheck, then carry out |
| `POST /api/agent-actions/{id}/reject` | Admin, Dispatcher | Refuse. The agent won't ask again |

### Known limits

- `raise_alert` only records the alert. Nothing tells staff yet except the
  actions list, until the inbox and email are live.
- Repeat detection is by subject, so a second, different alert about the same
  subject is treated as a repeat, and a rejected action stays rejected for good.
- Two simultaneous runs for an org are unlikely but not impossible. Approvals
  can't double-apply, but one run could record a repeat.
- The model's reasoning and tool transcript aren't stored, only requested actions,
  their reasons and the final summary.

## Tests

`agent/tests.rs` runs the real loop, database and policy against a scripted fake
of the API: the policy table for every mode and risk; off by default; shadow
changes nothing; approval carries out once and rechecks stale state; rejection
sticks; another org's dispatch and actions are out of reach; an expired document
blocks confirming; unknown tools are refused; endless runs stop; refusals and the
token limit. `routes/tests/agents.rs` covers org scoping and role gates over HTTP.
