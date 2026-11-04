# Chat inbox

Status: **built, not yet run against the real model** (2026-11-04). A
ChatGPT-style page where people run the business by asking, instead of going
from tab to tab: add a customer, raise a dispatch, track it through its steps,
plan a trip, move stock, bill, ask questions.

The dashboard side is in logistics-frontend (`/chat`, where signing in now lands).
The other pages stay, because some things are still easier to see there.

## How it works

```
 Person ──message──▶ Claude ──asks for a tool──┐
                       ▲                       ├─ read: run it now, as this person
                       │                       ├─ not in the catalog: refuse
                       └──── tool result ──────┤
                                               └─ write: store a card. Nothing runs.
 Person ──Confirm──▶ run the card, as this person ──▶ Claude carries on with the result
```

**It acts as the signed-in person, through the real API.** Each request goes
through the actual route handlers, in-process, with that person's own token. Roles,
org scope, validation, the dispatch state machine and notifications all work the
same as clicking through the dashboard, with no second copy of any rule.

**It can reach nearly everything, because the catalog comes from the OpenAPI
spec.** 75 of the 105 operations are available. The model sees a one-line list and
fetches an operation's schema with `describe_operation` when it needs it. Parameters
are limited to the ones the spec declares and are percent-encoded, so a request can't
reach an undocumented route.

**How this differs from agents** ([agentic-workflow.md](agentic-workflow.md)): an
agent works alone under the org's autonomy mode, and has few, narrow tools. Chat has
a person present who confirms every change, so it can offer far more, and it doesn't
depend on the autonomy mode.

## Controls

Customer names and notes reach the model, so a prompt injection has to be assumed
possible. These don't depend on the model behaving:

| Control | Where | Effect |
| --- | --- | --- |
| Confirmation | `runtime::call_operation` | A write only stores a card. Only `POST /api/chat/proposals/{id}/confirm` runs it |
| The card is the server's | `runtime::card_fields` | Fields come from the actual request, with names looked up for ids. The model's reason is labelled as its own |
| Catalog | `catalog::EXPOSED` | Anything else is refused. A test fails until every new route is listed as exposed or excluded |
| Credentials hidden | `catalog::redact` | Fields such as `tracker_key` are removed before a result reaches the model or the database. A test fails on a new field that looks like one |
| Own token | `Caller` | Each call is checked as this person, with the role in the token sent when they confirm |
| Once only | `store::claim_pending` | A double click can't run a write twice |
| Private | `store::Owner` | Conversations and cards belong to one person. Others get a 404 |
| Limits | `runtime.rs` | 12 model calls and 10 cards per message, 8,000 characters per message, about 100,000 tokens of history per conversation, a daily token limit per org |
| No markup | `MessageText.tsx` | Assistant text becomes React elements, never HTML, and links aren't made clickable |

### What chat can't do

Listed with reasons in `catalog::EXCLUDED`. People use the page instead for:

- signing in, creating an org, and adding team members, since those take a password;
  a password typed in a chat would go to the model and be stored;
- phone pairing and GPS tracker keys, which are credentials;
- agent controls (mode, run, approve), so a model can't change what agents may do;
- deleting the organization.

Deletes and money (`pay_invoice`, `record_vendor_payment`, `update_invoice`) are
available but get a warning card. Unlike agents, which have no such tools, a person
is making that decision here.

## Decisions

- **Every write needs a click, even an easy one.** It's the safe starting point.
  If the extra click turns out to be annoying, the natural next step is letting
  people pre-approve simple additions.
- **Tokens aren't streamed.** The request waits until the model has finished, and the
  page says "Working on it…". Streaming is a follow-up.
- **History is never trimmed.** Thinking blocks stop being valid if earlier turns are
  edited, and the history is what's cached. A conversation that gets too long asks
  the person to start a new one.
- **Results over 30,000 characters are cut**, and the model is told, so it can say so.
  List routes aren't paginated, so this will affect orgs with a lot of data.
- **Model:** `claude-opus-5-5` at `low` effort, since someone is waiting. Not measured
  against real conversations yet.

## Settings

| Variable | Default | |
| --- | --- | --- |
| `ANTHROPIC_API_KEY` | | Without it, sending a message answers 503 and the page says so |
| `CHAT_MODEL` | `AGENT_MODEL`, then `claude-opus-5-5` | |
| `CHAT_EFFORT` | `low` | |
| `CHAT_DAILY_TOKEN_LIMIT` | `2000000` | Per org, counting cached tokens as used. A rough guard, not a budget |
| `AGENT_MODEL_URL` | | For tests only: a stand-in model. The real key is never sent to it |

## Routes

| Method & path | |
| --- | --- |
| `GET, POST /api/chat/conversations` | This person's conversations, or start one |
| `GET, DELETE /api/chat/conversations/{id}` | Read one, ready to draw, or delete it. What it did stays done |
| `POST /api/chat/conversations/{id}/messages` | Send a message and wait for the answer |
| `POST /api/chat/proposals/{id}/confirm` | Run a card as this person |
| `POST /api/chat/proposals/{id}/cancel` | Refuse a card. Nothing runs |

Tables: `ChatConversations`, `ChatMessages`, `ChatProposals`, removed with the org.

## Before this goes live

- **Run it on the real model.** Tests script the model's words, so nothing yet shows
  how well it understands people. The API key has had no credits since 2026-11-03.
  `cargo test live_chat_run -- --ignored --nocapture` is the first check.
- **Clean the live database first** (`docs/database-cleanup.md`). Chat reads whatever
  is there, including test rows.
- **Data goes to Anthropic.** Read results, including customer names and phone numbers,
  are sent to the API. The older assistant sends some of this too, but this is much more.
- **Roles have gaps that chat inherits.** Per `docs/roles.md`, customer, vehicle,
  document and maintenance writes aren't role-gated on the server. Chat applies the same
  rules as the dashboard, so warehouse staff can add a customer from either.

## Tests

`chat/tests.rs` runs the real loop, handlers and database against a scripted fake of
the API: writes change nothing until confirmed; confirming twice runs once; cancelling;
excluded operations; role and org limits at confirm time; the card shows the real
record; credentials stay hidden; a dispatch raised, tracked, moved on and refused an
illegal move; history replayed unedited; the model failing without losing work.
`routes/tests/chat.rs` covers privacy over HTTP.
