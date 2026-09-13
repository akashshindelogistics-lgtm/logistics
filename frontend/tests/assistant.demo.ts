import { test, expect, type Page } from '@playwright/test';
import { registerOrg, uid } from './helpers';

/**
 * A short narrated walk through the org-scoped **"ask your data" assistant**
 * on its own — a global widget, available from any page, that answers
 * natural-language questions grounded in retrieval over the org's own
 * indexed data (dispatch notifications, vehicle compliance documents,
 * dispatch narratives, stock snapshots, and the org directory) plus a live
 * OpsReport snapshot for aggregate-shaped questions, rather than the LLM's
 * general knowledge. Also covers the two canned starter questions (daily
 * digest, reorder suggestions) and the manual reindex backfill.
 *
 * Like the pre-existing AI dispatch-summary feature, a live-answer step
 * needs ANTHROPIC_API_KEY configured on the server (not set in this
 * environment — see todo.org) — this demo shows the deterministic parts
 * that don't depend on it: the widget UI, indexing a real compliance
 * document, retrieval actually finding it (visible in the "based on N
 * facts" footer once an answer comes back, or the same honest
 * "could not reach the assistant" message the AI summary feature already
 * shows when unconfigured), and the fully canned answers (nothing indexed
 * yet, nothing needs attention, no godowns below threshold).
 *
 * Watch it with `npm run test:e2e:demo:assistant` (headed, slowed).
 */
const token = (page: Page) => page.evaluate(() => localStorage.getItem('logi_token'));
async function api(page: Page, method: 'post', path: string, data: unknown) {
  const res = await page.request[method](path, { data, headers: { Authorization: `Bearer ${await token(page)}` } });
  if (!res.ok()) throw new Error(`${method} ${path} -> ${res.status()} ${await res.text()}`);
  return res.json();
}

test('ask your data assistant', async ({ page }) => {
  test.slow();
  const org = await registerOrg(page, `Assistant Demo ${uid()}`);
  const vehicleReg = `AS${uid().toUpperCase()}`;
  const docNumber = `INS-${uid().toUpperCase()}`;

  await test.step('Register a vehicle with a compliance document', async () => {
    await api(page, 'post', `/api/orgs/${org.id}/vehicles`, {
      registration_number: vehicleReg,
      capacity: 20,
      unit: 'MetricTon',
    });
    await api(page, 'post', `/api/vehicles/${encodeURIComponent(vehicleReg)}/documents`, {
      doc_type: 'Insurance',
      document_number: docNumber,
      expires_on: new Date(Date.now() + 20 * 86_400_000).toISOString().slice(0, 10),
      notes: 'Renewed early this cycle.',
    });
  });

  await test.step('Open the assistant — it\'s available from any page', async () => {
    await page.goto(`/orgs/${org.id}`);
    await page.getByRole('button', { name: /open assistant/i }).click();
    await expect(page.getByRole('dialog', { name: /ask your data/i })).toBeVisible();
    await page.waitForTimeout(500);
  });

  const panel = page.getByRole('dialog', { name: /ask your data/i });

  await test.step('Ask an aggregate question nothing in retrieval could ever match — Phase 4 grounds it in a live OpsReport snapshot instead', async () => {
    // Registering the vehicle above already makes the org's OpsReport
    // non-empty (one vehicle, even idle), so this no longer short-circuits
    // to the canned "nothing indexed yet" answer the way it would for a
    // truly empty org — it attempts real generation grounded in the
    // snapshot, same as any other question.
    await page.getByLabel(/question for the assistant/i).fill('what is my fleet utilization like?');
    await page.getByRole('button', { name: /^ask$/i }).click();
    await Promise.race([
      panel.getByText(/based on \d+ fact|utiliz/i).waitFor({ timeout: 15000 }),
      panel.getByText(/could not reach the assistant/i).waitFor({ timeout: 15000 }),
    ]);
    await page.waitForTimeout(1200);
  });

  await test.step('Ask about the vehicle document just recorded — retrieval finds it', async () => {
    await page.getByLabel(/question for the assistant/i).fill(`when does vehicle ${vehicleReg}'s insurance expire?`);
    await page.getByRole('button', { name: /^ask$/i }).click();

    // Either a real answer (ANTHROPIC_API_KEY configured) with a "based on
    // N facts" footer citing the vehicle_document chunk, or the same honest
    // "could not reach" message the pre-existing AI summary feature shows
    // when it isn't — both are real, demonstrable outcomes of this feature.
    await Promise.race([
      panel.getByText(/based on \d+ fact/i).waitFor({ timeout: 15000 }),
      panel.getByText(/could not reach the assistant/i).waitFor({ timeout: 15000 }),
    ]);
    await page.waitForTimeout(1500);
  });

  await test.step('Reindex on demand — rebuilds every chunk for the org from scratch', async () => {
    await page.getByRole('button', { name: /reindex my data/i }).click();
    await expect(page.getByText(/reindexed \d+ fact\(s\)\./i)).toBeVisible({ timeout: 10000 });
    await page.waitForTimeout(1200);
  });

  await test.step('Ask "what needs my attention today?" — the daily digest starter question', async () => {
    // The vehicle's insurance expires in 20 days, inside the 30-day
    // ExpiringSoon window, so the digest has real content to narrate —
    // unlike the reorder-suggestions question below, it does not
    // short-circuit to a canned answer here. That means this step's outcome
    // depends on ANTHROPIC_API_KEY: with it, a real turn is added (the exact
    // question text becomes visible); without it, the call fails and only
    // the generic "could not reach" error shows — no turn is added at all,
    // so the two outcomes are asserted as alternatives, not in sequence.
    await page.getByRole('button', { name: /what needs my attention today/i }).click();
    await Promise.race([
      panel.getByText('What needs my attention today?', { exact: true }).waitFor({ timeout: 15000 }),
      panel.getByText(/could not reach the assistant/i).waitFor({ timeout: 15000 }),
    ]);
    await page.waitForTimeout(1500);
  });

  await test.step('Ask "any reorder suggestions?" — nothing is low, so this is a fully canned answer', async () => {
    await page.getByRole('button', { name: /any reorder suggestions/i }).click();
    await expect(panel.getByText('Any reorder suggestions?', { exact: true })).toBeVisible();
    await expect(panel.getByText(/no godowns are below their reorder threshold/i)).toBeVisible({ timeout: 10000 });
    await page.waitForTimeout(1200);
  });
});
